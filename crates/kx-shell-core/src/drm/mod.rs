// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! DRM/KMS hardware backend for `KynexComp`.

pub mod device;
pub mod egl;
pub mod gpu;
pub mod input;
pub mod layout;
pub mod output;
pub mod session;

use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::{Duration, Instant};

use calloop::generic::Generic;
use calloop::signals::{Signal, Signals};
use calloop::{EventLoop, Interest, LoopHandle, Mode, PostAction};
use smithay::backend::drm::compositor::{FrameFlags, PrimaryPlaneElement};
use smithay::backend::drm::{DrmDeviceNotifier, DrmEvent};
use smithay::backend::libinput::LibinputInputBackend;
use smithay::backend::renderer::Color32F;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::session::libseat::LibSeatSessionNotifier;
use smithay::output::Output;
use smithay::reexports::wayland_server::Display;
use smithay::wayland::socket::ListeningSocketSource;
use tracing::info;

pub use device::{ConnectorInfo, DrmDeviceInfo, DrmDiagnosticReport, ModeInfo};
pub use gpu::DrmGpuResources;
pub use layout::{ConnectorCandidate, OutputAssignment, allocate_outputs, migrate_window_position};
pub use output::DrmOutputPipeline;
pub use session::DrmSession;

use crate::client::ClientState;
use crate::error::ShellError;
use crate::frame_clock::FrameClock;
use crate::state::KynexState;

/// Options for configuring DRM backend execution.
#[derive(Debug, Clone, Default)]
pub struct DrmOptions {
    /// Optional exit timer duration.
    pub exit_after: Option<Duration>,
    /// Optional frame count limit (useful for automated testing).
    pub max_frames: Option<u64>,
    /// When true, only query and log hardware info, then exit immediately.
    pub diagnostic_only: bool,
}

struct DrmLoopState {
    running: bool,
    gpu: DrmGpuResources,
    pipeline: DrmOutputPipeline,
    clock: FrameClock,
    frame_count: u64,
    max_frames: Option<u64>,
    exit_after: Option<Duration>,
    start_time: Instant,
    bg_color: Color32F,
    last_error: Option<ShellError>,
    state: KynexState,
    display: Display<KynexState>,
}

impl DrmLoopState {
    fn render_clear_frame(&mut self) -> Result<(), ShellError> {
        let now = Instant::now();
        self.clock.schedule_frame();
        if let Err(err) = self.clock.on_render_begin(now) {
            tracing::warn!(%err, "Frame clock render begin warning");
        }

        let render_elements: [WaylandSurfaceRenderElement<GlesRenderer>; 0] = [];
        let frame_mode = FrameFlags::DEFAULT;

        let render_result = self
            .pipeline
            .output
            .render_frame(
                &mut self.gpu.renderer,
                &render_elements,
                self.bg_color,
                frame_mode,
            )
            .map_err(|err| ShellError::Output(format!("DRM render_frame error: {err:?}")))?;

        if render_result.is_empty {
            tracing::trace!("Render frame has no damage; skipping queue_frame");
            self.clock.on_render_skipped();
            return Ok(());
        }

        if render_result.needs_sync() {
            if let PrimaryPlaneElement::Swapchain(ref element) = render_result.primary_element {
                if let Err(err) = element.sync.wait() {
                    tracing::warn!(%err, "Failed to wait on swapchain sync fence");
                }
            }
        }

        self.pipeline
            .output
            .queue_frame(())
            .map_err(|err| ShellError::Output(format!("DRM queue_frame error: {err:?}")))?;

        let submitted_now = Instant::now();
        if let Err(err) = self.clock.on_render_submitted(submitted_now) {
            tracing::warn!(%err, "Frame clock render submitted warning");
        }

        Ok(())
    }
}

struct DrmEventSources {
    session_notifier: LibSeatSessionNotifier,
    drm_notifier: Option<DrmDeviceNotifier>,
    libinput: LibinputInputBackend,
    listening_socket: ListeningSocketSource,
    poll_fd: OwnedFd,
}

fn register_event_sources(
    loop_handle: &LoopHandle<'_, DrmLoopState>,
    sources: DrmEventSources,
) -> Result<(), ShellError> {
    let signals = Signals::new(&[Signal::SIGINT, Signal::SIGTERM])?;
    loop_handle
        .insert_source(signals, |signal_event, (), loop_state| {
            tracing::info!(signal = ?signal_event.signal(), "Shutdown signal received");
            loop_state.running = false;
        })
        .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;

    loop_handle
        .insert_source(sources.session_notifier, |event, (), loop_state| {
            handle_session_event(event, loop_state);
        })
        .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;

    if let Some(notifier) = sources.drm_notifier {
        loop_handle
            .insert_source(notifier, |event, _meta, loop_state| {
                handle_drm_event(event, loop_state);
            })
            .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;
    }

    loop_handle
        .insert_source(sources.libinput, |event, (), loop_state| {
            if input::process_input_event(event, &mut loop_state.state) {
                loop_state.running = false;
            }
        })
        .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;

    register_wayland_sources(loop_handle, sources.listening_socket, sources.poll_fd)?;

    Ok(())
}

fn handle_session_event(event: smithay::backend::session::Event, loop_state: &mut DrmLoopState) {
    match event {
        smithay::backend::session::Event::PauseSession => {
            tracing::info!("Session paused (VT switch away)");
            loop_state.pipeline.manager.pause();
        }
        smithay::backend::session::Event::ActivateSession => {
            tracing::info!("Session activated (VT switch back)");
            if let Err(err) = loop_state.pipeline.manager.device_mut().activate(false) {
                tracing::warn!(%err, "Failed to activate DRM device on session resume");
            }
            if let Err(err) = loop_state.render_clear_frame() {
                tracing::error!(%err, "Failed to re-render frame after session activation");
                loop_state.last_error = Some(err);
                loop_state.running = false;
            }
        }
    }
}

fn handle_drm_event(event: DrmEvent, loop_state: &mut DrmLoopState) {
    match event {
        DrmEvent::VBlank(crtc) => {
            if crtc == loop_state.pipeline.crtc {
                let _ = loop_state.pipeline.output.frame_submitted();
                let now = Instant::now();
                let _ = loop_state.clock.on_vblank(now);
                loop_state.frame_count = loop_state.frame_count.saturating_add(1);

                if let Some(limit) = loop_state.max_frames {
                    if loop_state.frame_count >= limit {
                        tracing::info!(
                            count = loop_state.frame_count,
                            "Reached maximum frame count, shutting down"
                        );
                        loop_state.running = false;
                        return;
                    }
                }

                if let Err(err) = loop_state.render_clear_frame() {
                    tracing::error!(%err, "Failed to render frame after vblank");
                    loop_state.last_error = Some(err);
                    loop_state.running = false;
                }
            }
        }
        DrmEvent::Error(err) => {
            tracing::error!(%err, "DRM device notifier error");
        }
    }
}

fn register_wayland_sources(
    loop_handle: &LoopHandle<'_, DrmLoopState>,
    listening_socket: ListeningSocketSource,
    poll_fd: OwnedFd,
) -> Result<(), ShellError> {
    loop_handle
        .insert_source(listening_socket, |stream, (), loop_state| {
            let client_data = Arc::new(ClientState::default());
            match loop_state
                .display
                .handle()
                .insert_client(stream, client_data)
            {
                Ok(client) => tracing::info!(?client, "Accepted new Wayland client connection"),
                Err(err) => tracing::error!(%err, "Failed to register accepted Wayland client"),
            }
        })
        .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;

    loop_handle
        .insert_source(
            Generic::new(poll_fd, Interest::READ, Mode::Level),
            |_readiness, _file, loop_state| {
                if let Err(err) = loop_state.display.dispatch_clients(&mut loop_state.state) {
                    tracing::error!(%err, "Error dispatching Wayland clients on poll event");
                }
                Ok(PostAction::Continue)
            },
        )
        .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;

    Ok(())
}

fn create_display_and_state(
    output: Output,
) -> Result<
    (
        Display<KynexState>,
        ListeningSocketSource,
        KynexState,
        OwnedFd,
    ),
    ShellError,
> {
    let mut display: Display<KynexState> = Display::new()?;
    let listening_socket = ListeningSocketSource::new_auto()?;
    let socket_name = listening_socket
        .socket_name()
        .to_string_lossy()
        .into_owned();

    tracing::info!(socket = %socket_name, "Wayland listening socket bound");

    let state = KynexState::new_with_output(&display.handle(), socket_name, output)?;
    let poll_fd = display.backend().poll_fd().try_clone_to_owned()?;
    Ok((display, listening_socket, state, poll_fd))
}

fn register_exit_timer(
    handle: &LoopHandle<'_, DrmLoopState>,
    duration: Duration,
) -> Result<(), ShellError> {
    let timer = calloop::timer::Timer::from_duration(duration);
    handle
        .insert_source(timer, |_deadline, (), loop_state| {
            tracing::info!("Exit-after timeout reached, shutting down DRM backend");
            loop_state.running = false;
            calloop::timer::TimeoutAction::Drop
        })
        .map_err(|e| {
            ShellError::SourceInsertion(format!("Failed to insert exit-after timer: {e}"))
        })?;
    Ok(())
}

fn run_event_loop(
    event_loop: &mut EventLoop<'_, DrmLoopState>,
    loop_state: &mut DrmLoopState,
) -> Result<(), ShellError> {
    while loop_state.running {
        if let Some(limit) = loop_state.exit_after {
            if loop_state.start_time.elapsed() >= limit {
                tracing::info!("Exit-after duration elapsed, shutting down");
                loop_state.running = false;
                break;
            }
        }

        event_loop.dispatch(Some(Duration::from_millis(16)), loop_state)?;

        if let Err(err) = loop_state.display.flush_clients() {
            tracing::error!(%err, "Failed to flush Wayland clients");
        }

        if let Some(err) = loop_state.last_error.take() {
            return Err(err);
        }
    }

    Ok(())
}

/// Entry point to initialize and run the DRM backend.
///
/// # Errors
///
/// Returns `ShellError` if session, GPU, output pipeline, or event loop initialization fails.
pub fn run_drm(options: &DrmOptions) -> Result<(), ShellError> {
    info!("Initializing DRM backend...");

    let (mut session, session_notifier) = DrmSession::open()?;

    let report = device::inspect_drm(&mut session)?;
    report.log_report();

    if options.diagnostic_only {
        info!("Diagnostic scan complete, exiting as requested by --diagnostic");
        return Ok(());
    }

    let primary_card = report
        .primary_gpu
        .or_else(|| report.devices.first().map(|d| d.path.clone()))
        .ok_or_else(|| ShellError::Drm("no DRM GPU device found on seat".to_string()))?;

    let mut gpu = DrmGpuResources::init(&mut session, &primary_card)?;
    let drm_notifier = gpu.drm_notifier.take();
    let pipeline = DrmOutputPipeline::init(&mut gpu)?;
    let libinput = input::init_libinput(session.libseat_session(), &session.seat())?;

    let (display, listening_socket, state, poll_fd) =
        create_display_and_state(pipeline.kynex_output.clone())?;

    let refresh_rate_mhz = pipeline.mode.vrefresh().saturating_mul(1000);
    let clock = FrameClock::new(refresh_rate_mhz);

    let [r, g, b, a] = crate::nested::DEFAULT_BG_COLOR;
    let bg_color = Color32F::new(r, g, b, a);

    let mut event_loop: EventLoop<'_, DrmLoopState> = EventLoop::try_new()?;

    if let Some(duration) = options.exit_after {
        register_exit_timer(&event_loop.handle(), duration)?;
    }

    let sources = DrmEventSources {
        session_notifier,
        drm_notifier,
        libinput,
        listening_socket,
        poll_fd,
    };
    register_event_sources(&event_loop.handle(), sources)?;

    let mut loop_state = DrmLoopState {
        running: true,
        gpu,
        pipeline,
        clock,
        frame_count: 0,
        max_frames: options.max_frames,
        exit_after: options.exit_after,
        start_time: Instant::now(),
        bg_color,
        last_error: None,
        state,
        display,
    };

    loop_state.render_clear_frame()?;

    if let Err(err) = kx_common::sd_notify(kx_common::notify_state::READY) {
        tracing::warn!(%err, "Failed to send systemd READY notification");
    }

    info!("DRM hardware compositor running, entering event loop");
    let result = run_event_loop(&mut event_loop, &mut loop_state);

    if let Err(err) = kx_common::sd_notify(kx_common::notify_state::STOPPING) {
        tracing::warn!(%err, "Failed to send systemd STOPPING notification");
    }

    info!("DRM backend shut down cleanly");
    result
}

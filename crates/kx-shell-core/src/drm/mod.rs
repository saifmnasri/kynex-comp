// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! DRM/KMS hardware backend for `KynexComp`.

pub mod device;
pub mod egl;
pub mod gpu;
pub mod hotplug;
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
use smithay::backend::drm::{DrmDeviceNotifier, DrmEvent};
use smithay::backend::libinput::LibinputInputBackend;
use smithay::backend::renderer::Color32F;
use smithay::backend::session::libseat::LibSeatSessionNotifier;
use smithay::backend::udev::{UdevBackend, UdevEvent};
use smithay::output::Output;
use smithay::reexports::wayland_server::Display;
use smithay::utils::{Logical, Point};
use smithay::wayland::socket::ListeningSocketSource;
use tracing::info;

pub use device::{ConnectorInfo, DrmDeviceInfo, DrmDiagnosticReport, ModeInfo};
pub use gpu::DrmGpuResources;
pub use layout::{ConnectorCandidate, OutputAssignment, allocate_outputs, migrate_window_position};
pub use output::DrmOutputPipeline;
pub use session::DrmSession;

use crate::client::ClientState;
use crate::error::ShellError;
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
    frame_count: u64,
    max_frames: Option<u64>,
    exit_after: Option<Duration>,
    start_time: Instant,
    bg_color: Color32F,
    last_error: Option<ShellError>,
    pointer_location: Point<f64, Logical>,
    state: KynexState,
    display: Display<KynexState>,
}

impl DrmLoopState {
    fn new(
        gpu: DrmGpuResources,
        pipeline: DrmOutputPipeline,
        mut state: KynexState,
        display: Display<KynexState>,
        options: &DrmOptions,
    ) -> Self {
        if state.drm_syncobj_state.is_none() {
            state.drm_syncobj_state = crate::syncobj::init_drm_syncobj_global(
                &display.handle(),
                Some(gpu.drm_fd.clone()),
            );
        }
        let [r, g, b, a] = crate::nested::DEFAULT_BG_COLOR;
        Self {
            running: true,
            gpu,
            pipeline,
            frame_count: 0,
            max_frames: options.max_frames,
            exit_after: options.exit_after,
            start_time: Instant::now(),
            bg_color: Color32F::new(r, g, b, a),
            last_error: None,
            pointer_location: Point::from((0.0, 0.0)),
            state,
            display,
        }
    }

    fn render_clear_frame(&mut self) -> Result<(), ShellError> {
        self.pipeline
            .render_all_clear_frames(&mut self.gpu, self.bg_color)
    }
}

struct DrmEventSources {
    session_notifier: LibSeatSessionNotifier,
    drm_notifier: Option<DrmDeviceNotifier>,
    udev: UdevBackend,
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
            let bounds = loop_state.pipeline.global_bounds();
            if input::process_input_event(
                event,
                &mut loop_state.state,
                &mut loop_state.pointer_location,
                bounds,
            ) {
                loop_state.running = false;
            }
        })
        .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;

    loop_handle
        .insert_source(sources.udev, |event, (), loop_state| {
            handle_udev_event(event, loop_state);
        })
        .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;

    register_wayland_sources(loop_handle, sources.listening_socket, sources.poll_fd)?;

    Ok(())
}

fn handle_udev_event(event: UdevEvent, loop_state: &mut DrmLoopState) {
    match event {
        UdevEvent::Changed { device_id } => {
            tracing::info!(?device_id, "Udev device change event detected");
            let dh = loop_state.display.handle();
            if let Err(err) =
                loop_state
                    .pipeline
                    .handle_hotplug(&mut loop_state.gpu, &mut loop_state.state, &dh)
            {
                tracing::error!(%err, "Failed to handle DRM monitor hotplug");
            }
        }
        UdevEvent::Added { device_id, path } => {
            tracing::debug!(?device_id, ?path, "Udev device added");
        }
        UdevEvent::Removed { device_id } => {
            tracing::debug!(?device_id, "Udev device removed");
        }
    }
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
            if loop_state.pipeline.on_vblank(crtc).is_some() {
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

                if let Err(err) = loop_state.pipeline.render_output_frame(
                    crtc,
                    &mut loop_state.gpu,
                    loop_state.bg_color,
                ) {
                    tracing::error!(%err, ?crtc, "Failed to render frame after vblank");
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
                .insert_client(stream, client_data.clone())
            {
                Ok(client) => {
                    if let Ok(creds) = client.get_credentials(&loop_state.display.handle()) {
                        client_data.inspect_and_cache_privilege(
                            &creds,
                            &loop_state.state.config.privilege.layer_shell_allow,
                        );
                    }
                    tracing::info!(?client, "Accepted new Wayland client connection");
                }
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

fn init_subsystems(
    session: &mut DrmSession,
    primary_card: &std::path::Path,
) -> Result<
    (
        DrmGpuResources,
        Option<DrmDeviceNotifier>,
        DrmOutputPipeline,
        UdevBackend,
        LibinputInputBackend,
    ),
    ShellError,
> {
    let mut gpu = DrmGpuResources::init(session, primary_card)?;
    let drm_notifier = gpu.drm_notifier.take();
    let pipeline = DrmOutputPipeline::init(&mut gpu)?;
    let udev = UdevBackend::new(session.seat())?;
    let libinput = input::init_libinput(session.libseat_session(), &session.seat())?;
    Ok((gpu, drm_notifier, pipeline, udev, libinput))
}

fn setup_drm_session(
    options: &DrmOptions,
) -> Result<Option<(DrmSession, LibSeatSessionNotifier, std::path::PathBuf)>, ShellError> {
    let (mut session, session_notifier) = DrmSession::open()?;
    let report = device::inspect_drm(&mut session)?;
    report.log_report();

    if options.diagnostic_only {
        info!("Diagnostic scan complete, exiting as requested by --diagnostic");
        return Ok(None);
    }

    let primary_card = report
        .primary_gpu
        .or_else(|| report.devices.first().map(|d| d.path.clone()))
        .ok_or_else(|| ShellError::Drm("no DRM GPU device found on seat".to_string()))?;

    Ok(Some((session, session_notifier, primary_card)))
}

/// Entry point to initialize and run the DRM backend.
///
/// # Errors
///
/// Returns `ShellError` if session, GPU, output pipeline, or event loop initialization fails.
pub fn run_drm(options: &DrmOptions) -> Result<(), ShellError> {
    info!("Initializing DRM backend...");

    let Some((mut session, session_notifier, primary_card)) = setup_drm_session(options)? else {
        return Ok(());
    };

    let (gpu, drm_notifier, pipeline, udev, libinput) =
        init_subsystems(&mut session, &primary_card)?;

    let primary_output = pipeline
        .primary_output()
        .map(|entry| entry.kynex_output.clone())
        .ok_or_else(|| {
            ShellError::Output("no active output configured in DRM pipeline".to_string())
        })?;

    let (display, listening_socket, mut state, poll_fd) = create_display_and_state(primary_output)?;

    let primary_crtc = pipeline.primary_crtc();
    for (crtc, entry) in &pipeline.outputs {
        if Some(*crtc) != primary_crtc {
            state.map_output(&display.handle(), &entry.kynex_output, entry.position);
        }
    }

    let mut event_loop: EventLoop<'_, DrmLoopState> = EventLoop::try_new()?;
    if let Some(duration) = options.exit_after {
        register_exit_timer(&event_loop.handle(), duration)?;
    }

    let sources = DrmEventSources {
        session_notifier,
        drm_notifier,
        udev,
        libinput,
        listening_socket,
        poll_fd,
    };
    register_event_sources(&event_loop.handle(), sources)?;

    let mut loop_state = DrmLoopState::new(gpu, pipeline, state, display, options);
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

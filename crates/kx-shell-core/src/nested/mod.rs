// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Nested compositor runner using the Smithay winit backend and Calloop event loop.

mod event;

use std::os::fd::OwnedFd;
use std::time::{Duration, Instant};

use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle};
use smithay::backend::renderer::Color32F;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::winit::{self, WinitGraphicsBackend};
use smithay::reexports::wayland_server::Display;
use smithay::wayland::socket::ListeningSocketSource;

use self::event::{NestedLoopState, register_event_sources};
use crate::error::ShellError;
use crate::state::KynexState;

/// Default background color (#1a1a24 in sRGB floating-point).
pub const DEFAULT_BG_COLOR: [f32; 4] = [
    26.0 / 255.0, // 0.101960785
    26.0 / 255.0, // 0.101960785
    36.0 / 255.0, // 0.14117647
    1.0,
];

/// Configuration for the nested winit window.
#[derive(Debug, Clone)]
pub struct NestedConfig {
    /// Window title.
    pub title: String,
    /// Background clear color [R, G, B, A] normalized to [0.0, 1.0].
    pub background_color: [f32; 4],
}

impl Default for NestedConfig {
    fn default() -> Self {
        Self {
            title: "KynexComp (nested)".to_string(),
            background_color: DEFAULT_BG_COLOR,
        }
    }
}

/// Execution options for the nested compositor loop.
#[derive(Debug, Clone, Default)]
pub struct NestedOptions {
    /// Nested window configuration.
    pub config: NestedConfig,
    /// Optional limit on the number of rendered frames before exiting (useful for testing).
    pub max_frames: Option<u64>,
    /// Optional time limit before the compositor cleanly terminates.
    pub exit_after: Option<Duration>,
}

fn create_display_and_state(
    backend: &WinitGraphicsBackend<GlesRenderer>,
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

    let state = KynexState::new(&display.handle(), socket_name, backend.window_size())?;
    let poll_fd = display.backend().poll_fd().try_clone_to_owned()?;
    Ok((display, listening_socket, state, poll_fd))
}

fn register_exit_timer(
    handle: &LoopHandle<'_, NestedLoopState>,
    duration: Duration,
) -> Result<(), ShellError> {
    let timer = Timer::from_duration(duration);
    handle
        .insert_source(timer, |_deadline, (), loop_state| {
            tracing::info!("Exit-after timeout reached, shutting down");
            loop_state.running = false;
            TimeoutAction::Drop
        })
        .map_err(|e| {
            ShellError::SourceInsertion(format!("Failed to insert exit-after timer: {e}"))
        })?;
    Ok(())
}

fn run_event_loop(
    event_loop: &mut EventLoop<'_, NestedLoopState>,
    loop_state: &mut NestedLoopState,
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
    }
    Ok(())
}

fn setup_nested_environment(
    options: &NestedOptions,
) -> Result<(EventLoop<'static, NestedLoopState>, NestedLoopState), ShellError> {
    let (backend, winit_event_loop) = winit::init::<GlesRenderer>()?;
    backend.window().set_title(&options.config.title);

    let [r, g, b, a] = options.config.background_color;
    let bg_color = Color32F::new(r, g, b, a);

    let (display, listening_socket, state, poll_fd) = create_display_and_state(&backend)?;
    let damage_tracker = OutputDamageTracker::from_output(&state.output);
    let start_time = Instant::now();

    #[cfg(feature = "fault-injection")]
    let fault_controller = crate::fault::FaultController::from_env()
        .map_err(|e| ShellError::Render(format!("Invalid fault spec: {e}")))?;

    let event_loop: EventLoop<'static, NestedLoopState> = EventLoop::try_new()?;
    register_event_sources(
        &event_loop.handle(),
        listening_socket,
        poll_fd,
        winit_event_loop,
    )?;

    if let Some(duration) = options.exit_after {
        register_exit_timer(&event_loop.handle(), duration)?;
    }

    let loop_state = NestedLoopState::new(
        backend,
        damage_tracker,
        options,
        bg_color,
        start_time,
        state,
        display,
        #[cfg(feature = "fault-injection")]
        fault_controller,
    );

    Ok((event_loop, loop_state))
}

/// Runs the nested compositor with the winit backend.
///
/// # Errors
///
/// Returns a [`ShellError`] if initialization fails or an unrecoverable event loop error occurs.
pub fn run_nested(options: &NestedOptions) -> Result<(), ShellError> {
    tracing::info!("Initializing nested winit backend");

    let (mut event_loop, mut loop_state) = setup_nested_environment(options)?;

    loop_state.backend.window().request_redraw();
    if let Err(err) = kx_common::sd_notify(kx_common::notify_state::READY) {
        tracing::warn!(%err, "Failed to send systemd READY notification");
    }
    tracing::info!("Entering nested compositor main event loop");

    run_event_loop(&mut event_loop, &mut loop_state)?;

    if let Err(err) = kx_common::sd_notify(kx_common::notify_state::STOPPING) {
        tracing::warn!(%err, "Failed to send systemd STOPPING notification");
    }

    tracing::info!(
        total_frames = loop_state.frame_count,
        "Exited nested compositor main event loop cleanly"
    );

    if let Some(err) = loop_state.last_error {
        return Err(err);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_nested_config() {
        let config = NestedConfig::default();
        assert_eq!(config.title, "KynexComp (nested)");
        assert!((config.background_color[0] - DEFAULT_BG_COLOR[0]).abs() < f32::EPSILON);
        assert!((config.background_color[1] - DEFAULT_BG_COLOR[1]).abs() < f32::EPSILON);
        assert!((config.background_color[2] - DEFAULT_BG_COLOR[2]).abs() < f32::EPSILON);
        assert!((config.background_color[3] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_color_rgb_channels() {
        // #1a1a24: 0x1a = 26, 0x24 = 36
        assert_eq!(DEFAULT_BG_COLOR[0], 26.0 / 255.0);
        assert_eq!(DEFAULT_BG_COLOR[1], 26.0 / 255.0);
        assert_eq!(DEFAULT_BG_COLOR[2], 36.0 / 255.0);
        assert_eq!(DEFAULT_BG_COLOR[3], 1.0);
    }
}

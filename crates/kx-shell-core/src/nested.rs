// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Nested compositor runner using the Smithay winit backend and Calloop event loop.

use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::{Duration, Instant};

use calloop::generic::Generic;
use calloop::signals::{Signal, Signals};
use calloop::{EventLoop, Interest, LoopHandle, Mode, PostAction};
use smithay::backend::input::{InputEvent, KeyState, KeyboardKeyEvent};
use smithay::backend::renderer::Color32F;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::winit::{self, WinitEvent, WinitEventLoop, WinitGraphicsBackend, WinitInput};
use smithay::desktop::space::render_output;
use smithay::reexports::wayland_server::Display;
use smithay::wayland::socket::ListeningSocketSource;

use crate::client::ClientState;
use crate::error::ShellError;
use crate::state::KynexState;

/// Linux input event codes for emergency exit key combinations.
const KEY_ESC: u32 = 1;
const KEY_BACKSPACE: u32 = 14;
const KEY_LEFTCTRL: u32 = 29;
const KEY_RIGHTCTRL: u32 = 97;
const KEY_LEFTSHIFT: u32 = 42;
const KEY_RIGHTSHIFT: u32 = 54;
const KEY_LEFTALT: u32 = 56;
const KEY_RIGHTALT: u32 = 100;

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
}

/// Tracks emergency modifier keys.
#[derive(Debug, Default, Clone, Copy)]
struct EmergencyModifiers {
    ctrl: bool,
    shift: bool,
    alt: bool,
}

/// Internal loop state passed to event source callbacks.
struct NestedLoopState {
    running: bool,
    backend: WinitGraphicsBackend<GlesRenderer>,
    damage_tracker: OutputDamageTracker,
    modifiers: EmergencyModifiers,
    frame_count: u64,
    max_frames: Option<u64>,
    bg_color: Color32F,
    start_time: Instant,
    last_error: Option<ShellError>,
    state: KynexState,
    display: Display<KynexState>,
}

impl NestedLoopState {
    fn handle_keyboard_event(&mut self, event: &impl KeyboardKeyEvent<WinitInput>) {
        let key = event.key_code().raw();
        let is_pressed = event.state() == KeyState::Pressed;

        match key {
            KEY_LEFTCTRL | KEY_RIGHTCTRL => self.modifiers.ctrl = is_pressed,
            KEY_LEFTSHIFT | KEY_RIGHTSHIFT => self.modifiers.shift = is_pressed,
            KEY_LEFTALT | KEY_RIGHTALT => self.modifiers.alt = is_pressed,
            KEY_ESC if is_pressed && self.modifiers.ctrl && self.modifiers.shift => {
                tracing::info!("Emergency exit key combination triggered (Ctrl+Shift+Escape)");
                self.running = false;
            }
            KEY_BACKSPACE if is_pressed && self.modifiers.ctrl && self.modifiers.alt => {
                tracing::info!("Emergency exit key combination triggered (Ctrl+Alt+Backspace)");
                self.running = false;
            }
            _ => {}
        }
    }

    fn send_frames(&self) {
        let output = &self.state.output;
        let elapsed = self.start_time.elapsed();
        for window in self.state.space.elements() {
            window.send_frame(output, elapsed, Some(Duration::ZERO), |_, _| {
                Some(output.clone())
            });
        }
    }

    fn render_frame(&mut self) -> Result<(), ShellError> {
        // Dispatch pending client messages before compositing
        self.display
            .dispatch_clients(&mut self.state)
            .map_err(|e| ShellError::ClientDispatch(e.to_string()))?;

        let buffer_age = self.backend.buffer_age().unwrap_or(0);

        let render_res = {
            let (renderer, mut framebuffer) = self.backend.bind()?;
            render_output::<_, WaylandSurfaceRenderElement<GlesRenderer>, _, _>(
                &self.state.output,
                renderer,
                &mut framebuffer,
                1.0,
                buffer_age,
                [&self.state.space],
                &[],
                &mut self.damage_tracker,
                self.bg_color,
            )
            .map_err(|e| ShellError::Render(format!("{e:?}")))?
        };

        if let Some(damage) = render_res.damage {
            self.backend.submit(Some(damage))?;
        }

        self.send_frames();

        // Flush protocol replies to clients after frame submission
        self.display
            .flush_clients()
            .map_err(|e| ShellError::ClientDispatch(e.to_string()))?;

        self.frame_count = self.frame_count.saturating_add(1);
        if let Some(max) = self.max_frames {
            if self.frame_count >= max {
                tracing::info!(
                    frames = self.frame_count,
                    "Reached maximum configured frame count; shutting down"
                );
                self.running = false;
            } else {
                self.backend.window().request_redraw();
            }
        }

        Ok(())
    }
}

fn register_event_sources(
    loop_handle: &LoopHandle<'_, NestedLoopState>,
    listening_socket: ListeningSocketSource,
    poll_fd: OwnedFd,
    winit_event_loop: WinitEventLoop,
) -> Result<(), ShellError> {
    let signals = Signals::new(&[Signal::SIGINT, Signal::SIGTERM])?;
    loop_handle
        .insert_source(signals, |signal_event, (), loop_state| {
            tracing::info!(signal = ?signal_event.signal(), "Shutdown signal received");
            loop_state.running = false;
        })
        .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;

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
                loop_state.backend.window().request_redraw();
                Ok(PostAction::Continue)
            },
        )
        .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;

    loop_handle
        .insert_source(winit_event_loop, |event, (), loop_state| {
            handle_winit_event(&event, loop_state);
        })
        .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;

    Ok(())
}

fn handle_winit_event(event: &WinitEvent, loop_state: &mut NestedLoopState) {
    match event {
        WinitEvent::CloseRequested => {
            tracing::info!("Window close requested; shutting down nested compositor");
            loop_state.running = false;
        }
        WinitEvent::Redraw => {
            if let Err(err) = loop_state.render_frame() {
                tracing::error!(error = %err, "Failed to render frame in nested backend");
                loop_state.last_error = Some(err);
                loop_state.running = false;
            }
        }
        WinitEvent::Resized { size, scale_factor } => {
            tracing::debug!(?size, scale_factor, "Nested window resized");
            loop_state.state.update_output_size(*size);
            loop_state.backend.window().request_redraw();
        }
        WinitEvent::Focus(focused) => {
            tracing::debug!(focused, "Nested window focus changed");
        }
        WinitEvent::Input(InputEvent::Keyboard { event }) => {
            loop_state.handle_keyboard_event(event);
        }
        WinitEvent::Input(_) => {}
    }
}

/// Runs the nested compositor with the winit backend.
///
/// # Errors
///
/// Returns a [`ShellError`] if initialization fails or an unrecoverable event loop error occurs.
pub fn run_nested(options: &NestedOptions) -> Result<(), ShellError> {
    tracing::info!("Initializing nested winit backend");

    let (backend, winit_event_loop) = winit::init::<GlesRenderer>()?;
    backend.window().set_title(&options.config.title);

    let [r, g, b, a] = options.config.background_color;
    let bg_color = Color32F::new(r, g, b, a);

    let mut display: Display<KynexState> = Display::new()?;
    let listening_socket = ListeningSocketSource::new_auto()?;
    let socket_name = listening_socket
        .socket_name()
        .to_string_lossy()
        .into_owned();

    tracing::info!(socket = %socket_name, "Wayland listening socket bound");

    let state = KynexState::new(&display.handle(), socket_name, backend.window_size())?;
    let poll_fd = display.backend().poll_fd().try_clone_to_owned()?;
    let damage_tracker = OutputDamageTracker::from_output(&state.output);
    let start_time = Instant::now();

    let mut event_loop: EventLoop<'_, NestedLoopState> = EventLoop::try_new()?;
    register_event_sources(
        &event_loop.handle(),
        listening_socket,
        poll_fd,
        winit_event_loop,
    )?;

    let mut loop_state = NestedLoopState {
        running: true,
        backend,
        damage_tracker,
        modifiers: EmergencyModifiers::default(),
        frame_count: 0,
        max_frames: options.max_frames,
        bg_color,
        start_time,
        last_error: None,
        state,
        display,
    };

    loop_state.backend.window().request_redraw();
    tracing::info!("Entering nested compositor main event loop");

    while loop_state.running {
        event_loop.dispatch(Some(Duration::from_millis(16)), &mut loop_state)?;
        if let Err(err) = loop_state.display.flush_clients() {
            tracing::error!(%err, "Failed to flush Wayland clients");
        }
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

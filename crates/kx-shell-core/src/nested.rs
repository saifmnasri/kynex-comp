// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Nested compositor runner using the Smithay winit backend.

use std::time::Duration;

use calloop::EventLoop;
use calloop::signals::{Signal, Signals};
use smithay::backend::input::{InputEvent, KeyState, KeyboardKeyEvent};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::{Color32F, Frame, Renderer};
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend, WinitInput};
use smithay::utils::{Rectangle, Transform};

use crate::error::ShellError;

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
    modifiers: EmergencyModifiers,
    frame_count: u64,
    max_frames: Option<u64>,
    bg_color: Color32F,
    last_error: Option<ShellError>,
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

    fn render_frame(&mut self) -> Result<(), ShellError> {
        let window_size = self.backend.window_size();
        let damage = [Rectangle::from_size(window_size)];

        {
            let (renderer, mut framebuffer) = self.backend.bind()?;
            let mut frame = renderer
                .render(&mut framebuffer, window_size, Transform::Normal)
                .map_err(|e| ShellError::Render(format!("{e:?}")))?;

            frame
                .clear(self.bg_color, &damage)
                .map_err(|e| ShellError::Render(format!("{e:?}")))?;

            // SyncPoint returned by frame.finish; on nested winit backend buffer swap handles synchronization.
            let _sync_point = frame
                .finish()
                .map_err(|e| ShellError::Render(format!("{e:?}")))?;
        }

        self.backend.submit(Some(&damage))?;

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

    let mut event_loop: EventLoop<'_, NestedLoopState> = EventLoop::try_new()?;
    let loop_handle = event_loop.handle();

    // Register OS signal handler (SIGINT, SIGTERM)
    let signals = Signals::new(&[Signal::SIGINT, Signal::SIGTERM])?;
    loop_handle
        .insert_source(signals, |signal_event, (), state| {
            tracing::info!(signal = ?signal_event.signal(), "Shutdown signal received");
            state.running = false;
        })
        .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;

    // Register Winit event loop
    loop_handle
        .insert_source(winit_event_loop, |event, (), state| {
            match event {
                WinitEvent::CloseRequested => {
                    tracing::info!("Window close requested; shutting down nested compositor");
                    state.running = false;
                }
                WinitEvent::Redraw => {
                    if let Err(err) = state.render_frame() {
                        tracing::error!(error = %err, "Failed to render frame in nested backend");
                        state.last_error = Some(err);
                        state.running = false;
                    }
                }
                WinitEvent::Resized { size, scale_factor } => {
                    tracing::debug!(?size, scale_factor, "Nested window resized");
                    state.backend.window().request_redraw();
                }
                WinitEvent::Focus(focused) => {
                    tracing::debug!(focused, "Nested window focus changed");
                }
                WinitEvent::Input(InputEvent::Keyboard { event }) => {
                    state.handle_keyboard_event(&event);
                }
                WinitEvent::Input(_) => {
                    // Other input events will be routed in Milestone 1E
                }
            }
        })
        .map_err(|e| ShellError::SourceInsertion(e.to_string()))?;

    let mut state = NestedLoopState {
        running: true,
        backend,
        modifiers: EmergencyModifiers::default(),
        frame_count: 0,
        max_frames: options.max_frames,
        bg_color,
        last_error: None,
    };

    // Request initial redraw
    state.backend.window().request_redraw();

    tracing::info!("Entering nested compositor main event loop");

    while state.running {
        event_loop.dispatch(Some(Duration::from_millis(16)), &mut state)?;
    }

    tracing::info!(
        total_frames = state.frame_count,
        "Exited nested compositor main event loop cleanly"
    );

    if let Some(err) = state.last_error {
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

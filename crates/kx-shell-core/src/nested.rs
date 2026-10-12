// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Nested compositor runner using the Smithay winit backend and Calloop event loop.

use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::{Duration, Instant};

use calloop::generic::Generic;
use calloop::signals::{Signal, Signals};
use calloop::{EventLoop, Interest, LoopHandle, Mode, PostAction};
use smithay::backend::input::{
    Axis, ButtonState, InputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent,
    PointerButtonEvent, PointerMotionAbsoluteEvent,
};
use smithay::backend::renderer::Color32F;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::winit::{self, WinitEvent, WinitEventLoop, WinitGraphicsBackend, WinitInput};
use smithay::desktop::WindowSurfaceType;
use smithay::desktop::space::render_output;
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::reexports::wayland_server::Display;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::socket::ListeningSocketSource;

use crate::client::ClientState;
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

/// Tracks emergency modifier keys.
/// Internal loop state passed to event source callbacks.
struct NestedLoopState {
    running: bool,
    backend: WinitGraphicsBackend<GlesRenderer>,
    damage_tracker: OutputDamageTracker,
    pointer_location: Point<f64, Logical>,
    frame_count: u64,
    max_frames: Option<u64>,
    exit_after: Option<Duration>,
    bg_color: Color32F,
    start_time: Instant,
    last_error: Option<ShellError>,
    state: KynexState,
    display: Display<KynexState>,
    #[cfg(feature = "fault-injection")]
    fault_controller: crate::fault::FaultController,
}

impl NestedLoopState {
    fn handle_keyboard_event(&mut self, event: &impl KeyboardKeyEvent<WinitInput>) {
        let serial = SERIAL_COUNTER.next_serial();
        if let Some(keyboard) = self.state.seat.get_keyboard() {
            let action = keyboard.input(
                &mut self.state,
                event.key_code(),
                event.state(),
                serial,
                event.time_msec(),
                |state, modifiers, keysym_handle| {
                    if event.state() == KeyState::Pressed {
                        let mut key_mods = kx_input::KeyModifiers::empty();
                        if modifiers.ctrl {
                            key_mods |= kx_input::KeyModifiers::CTRL;
                        }
                        if modifiers.shift {
                            key_mods |= kx_input::KeyModifiers::SHIFT;
                        }
                        if modifiers.alt {
                            key_mods |= kx_input::KeyModifiers::ALT;
                        }
                        if modifiers.logo {
                            key_mods |= kx_input::KeyModifiers::LOGO;
                        }

                        let sym = kx_input::KeySym(keysym_handle.modified_sym().raw());
                        if let Some(action) = state.binding_matcher.matches(key_mods, sym) {
                            return FilterResult::Intercept(action.clone());
                        }

                        for raw_sym in keysym_handle.raw_syms() {
                            let sym = kx_input::KeySym(raw_sym.raw());
                            if let Some(action) = state.binding_matcher.matches(key_mods, sym) {
                                return FilterResult::Intercept(action.clone());
                            }
                        }
                    }
                    FilterResult::Forward
                },
            );

            if let Some(kx_input::CompositorAction::Quit) = action {
                tracing::info!("Emergency exit key combination triggered; shutting down");
                self.running = false;
            }
        }
    }

    fn handle_pointer_motion_absolute(
        &mut self,
        event: &impl PointerMotionAbsoluteEvent<WinitInput>,
    ) {
        let output_size = self
            .state
            .output
            .current_mode()
            .map(|m| m.size)
            .unwrap_or_default();
        self.pointer_location = event.position_transformed(output_size.to_logical(1));
        let serial = SERIAL_COUNTER.next_serial();

        let under = self
            .state
            .space
            .element_under(self.pointer_location)
            .and_then(|(window, window_loc)| {
                let relative = self.pointer_location - window_loc.to_f64();
                window.surface_under(relative, WindowSurfaceType::ALL).map(
                    |(surface, surface_offset)| (surface, (window_loc + surface_offset).to_f64()),
                )
            });

        if let Some(pointer) = self.state.seat.get_pointer() {
            pointer.motion(
                &mut self.state,
                under,
                &MotionEvent {
                    location: self.pointer_location,
                    serial,
                    time: event.time_msec(),
                },
            );
            pointer.frame(&mut self.state);
        }
    }

    fn handle_pointer_button(&mut self, event: &impl PointerButtonEvent<WinitInput>) {
        let serial = SERIAL_COUNTER.next_serial();
        let button = event.button_code();
        let state = event.state();
        let time = event.time_msec();

        if state == ButtonState::Pressed {
            if let Some((window, _loc)) = self.state.space.element_under(self.pointer_location) {
                let window = window.clone();
                self.state.space.raise_element(&window, true);
                if let Some(surface) = window.toplevel().map(|t| t.wl_surface().clone()) {
                    if let Some(keyboard) = self.state.seat.get_keyboard() {
                        keyboard.set_focus(&mut self.state, Some(surface), serial);
                    }
                }
            }
        }

        if let Some(pointer) = self.state.seat.get_pointer() {
            pointer.button(
                &mut self.state,
                &ButtonEvent {
                    serial,
                    time,
                    button,
                    state,
                },
            );
            pointer.frame(&mut self.state);
        }
    }

    fn handle_pointer_axis(&mut self, event: &impl PointerAxisEvent<WinitInput>) {
        let mut frame = AxisFrame::new(event.time_msec()).source(event.source());

        for axis in [Axis::Horizontal, Axis::Vertical] {
            if let Some(amount) = event.amount(axis) {
                frame = frame.value(axis, amount);
            }
            #[expect(
                clippy::cast_possible_truncation,
                reason = "discrete v120 scroll step count fits within standard i32"
            )]
            if let Some(amount_v120) = event.amount_v120(axis) {
                frame = frame.v120(axis, amount_v120 as i32);
            }
        }

        if let Some(pointer) = self.state.seat.get_pointer() {
            pointer.axis(&mut self.state, frame);
            pointer.frame(&mut self.state);
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
        self.frame_count = self.frame_count.saturating_add(1);

        #[cfg(feature = "fault-injection")]
        {
            self.fault_controller.check_pre_render(self.frame_count);
            if self.fault_controller.should_fail_gpu(self.frame_count) {
                tracing::error!(
                    frame = self.frame_count,
                    "Fault-injection: simulating GPU context loss"
                );
                return Err(ShellError::SwapBuffers(
                    smithay::backend::SwapBuffersError::ContextLost(Box::new(
                        std::io::Error::other("Simulated GPU context loss via fault injection"),
                    )),
                ));
            }
        }

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
        WinitEvent::Input(InputEvent::PointerMotionAbsolute { event }) => {
            loop_state.handle_pointer_motion_absolute(event);
        }
        WinitEvent::Input(InputEvent::PointerButton { event }) => {
            loop_state.handle_pointer_button(event);
        }
        WinitEvent::Input(InputEvent::PointerAxis { event }) => {
            loop_state.handle_pointer_axis(event);
        }
        WinitEvent::Input(_) => {}
    }
}

/// Runs the nested compositor with the winit backend.
///
/// # Errors
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
    let timer = calloop::timer::Timer::from_duration(duration);
    handle
        .insert_source(timer, |_deadline, (), loop_state| {
            tracing::info!("Exit-after timeout reached, shutting down");
            loop_state.running = false;
            calloop::timer::TimeoutAction::Drop
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

    let (display, listening_socket, state, poll_fd) = create_display_and_state(&backend)?;
    let damage_tracker = OutputDamageTracker::from_output(&state.output);
    let start_time = Instant::now();

    #[cfg(feature = "fault-injection")]
    let fault_controller = crate::fault::FaultController::from_env()
        .map_err(|e| ShellError::Render(format!("Invalid fault spec: {e}")))?;

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
        pointer_location: Point::from((0.0, 0.0)),
        frame_count: 0,
        max_frames: options.max_frames,
        exit_after: options.exit_after,
        bg_color,
        start_time,
        last_error: None,
        state,
        display,
        #[cfg(feature = "fault-injection")]
        fault_controller,
    };

    if let Some(duration) = options.exit_after {
        register_exit_timer(&event_loop.handle(), duration)?;
    }

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

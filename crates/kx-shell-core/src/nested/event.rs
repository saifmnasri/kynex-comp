// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Event handling and loop state for the nested compositor backend.

use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::{Duration, Instant};

use calloop::generic::Generic;
use calloop::signals::{Signal, Signals};
use calloop::{Interest, LoopHandle, Mode, PostAction};
use smithay::backend::input::{
    Axis, ButtonState, InputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent,
    PointerButtonEvent, PointerMotionAbsoluteEvent,
};
use smithay::backend::renderer::Color32F;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::winit::{WinitEvent, WinitEventLoop, WinitGraphicsBackend, WinitInput};
use smithay::desktop::WindowSurfaceType;
use smithay::desktop::space::render_output;
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::reexports::wayland_server::Display;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::socket::ListeningSocketSource;

use crate::client::ClientState;
use crate::error::ShellError;
use crate::nested::NestedOptions;
use crate::state::KynexState;

/// Internal loop state passed to event source callbacks.
pub(crate) struct NestedLoopState {
    pub(crate) running: bool,
    pub(crate) backend: WinitGraphicsBackend<GlesRenderer>,
    pub(crate) damage_tracker: OutputDamageTracker,
    pub(crate) pointer_location: Point<f64, Logical>,
    pub(crate) frame_count: u64,
    pub(crate) max_frames: Option<u64>,
    pub(crate) exit_after: Option<Duration>,
    pub(crate) bg_color: Color32F,
    pub(crate) start_time: Instant,
    pub(crate) last_error: Option<ShellError>,
    pub(crate) state: KynexState,
    pub(crate) display: Display<KynexState>,
    #[cfg(feature = "fault-injection")]
    pub(crate) fault_controller: crate::fault::FaultController,
}

impl NestedLoopState {
    #[expect(
        clippy::too_many_arguments,
        reason = "nested loop state constructs monolithic runtime bundle from initial backend handles"
    )]
    pub(crate) fn new(
        backend: WinitGraphicsBackend<GlesRenderer>,
        damage_tracker: OutputDamageTracker,
        options: &NestedOptions,
        bg_color: Color32F,
        start_time: Instant,
        state: KynexState,
        display: Display<KynexState>,
        #[cfg(feature = "fault-injection")] fault_controller: crate::fault::FaultController,
    ) -> Self {
        Self {
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
        }
    }

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

            if let Some(action) = action {
                if crate::action::execute_compositor_action(&mut self.state, &action) {
                    self.running = false;
                }
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

        let under = if self.state.is_session_locked {
            crate::session_lock::find_lock_surface_under(&self.state, self.pointer_location)
        } else {
            self.state
                .space
                .element_under(self.pointer_location)
                .and_then(|(window, window_loc)| {
                    let relative = self.pointer_location - window_loc.to_f64();
                    window.surface_under(relative, WindowSurfaceType::ALL).map(
                        |(surface, surface_offset)| {
                            (surface, (window_loc + surface_offset).to_f64())
                        },
                    )
                })
        };

        let mut grabbed = false;
        if let Some(pointer) = self.state.seat.get_pointer() {
            grabbed = pointer.is_grabbed();
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
            grabbed = grabbed || pointer.is_grabbed();
            if let Some(surface) = pointer.current_focus() {
                crate::pointer_constraints::maybe_activate_pointer_constraint(&surface, &pointer);
            }
        }

        if grabbed {
            self.backend.window().request_redraw();
        } else if self.state.is_session_locked {
            self.backend
                .window()
                .set_cursor(smithay::reexports::winit::window::CursorIcon::Default);
        } else {
            let hit = crate::decoration::find_window_hit_test(&self.state, self.pointer_location);
            let icon = hit
                .as_ref()
                .and_then(|(_, h)| crate::decoration::cursor_icon_for_hit_test(h))
                .unwrap_or(smithay::reexports::winit::window::CursorIcon::Default);
            self.backend.window().set_cursor(icon);
        }
    }

    fn handle_decoration_click(&mut self, button: u32, serial: smithay::utils::Serial) -> bool {
        crate::decoration::handle_decoration_click(
            &mut self.state,
            self.pointer_location,
            button,
            serial,
        )
    }

    fn handle_pointer_button(&mut self, event: &impl PointerButtonEvent<WinitInput>) {
        let serial = SERIAL_COUNTER.next_serial();
        let button = event.button_code();
        let state = event.state();
        let time = event.time_msec();

        if self.state.is_session_locked {
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
            self.backend.window().request_redraw();
            return;
        }

        if state == ButtonState::Pressed {
            if crate::grab::try_start_compositor_grab(
                &mut self.state,
                self.pointer_location,
                button,
                serial,
            ) || self.handle_decoration_click(button, serial)
            {
                self.backend.window().request_redraw();
                return;
            }

            if let Some((window, _loc)) = self.state.space.element_under(self.pointer_location) {
                let window = window.clone();
                self.state.space.raise_element(&window, true);
                if let Some(surface) = window.toplevel().map(|t| t.wl_surface().clone()) {
                    if let Some(&wid) = self.state.surface_to_window_id.get(&surface) {
                        crate::action::activate_and_focus_window(&mut self.state, wid);
                    } else if let Some(keyboard) = self.state.seat.get_keyboard() {
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

        self.backend.window().request_redraw();
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
        crate::presentation::post_presentation_feedback(
            &self.state.space,
            output,
            elapsed,
            self.frame_count,
        );
    }

    fn check_fault_injection(&mut self) -> Result<(), ShellError> {
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
        Ok(())
    }

    fn check_frame_limits(&mut self) {
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
    }

    fn render_frame(&mut self) -> Result<(), ShellError> {
        self.frame_count = self.frame_count.saturating_add(1);
        self.check_fault_injection()?;

        // Dispatch pending client messages before compositing
        self.display
            .dispatch_clients(&mut self.state)
            .map_err(|e| ShellError::ClientDispatch(e.to_string()))?;

        let buffer_age = self.backend.buffer_age().unwrap_or(0);
        let (clear_color, spaces_active) = if self.state.is_session_locked {
            (Color32F::new(0.0, 0.0, 0.0, 1.0), false)
        } else {
            (self.bg_color, true)
        };

        let render_res = {
            let (renderer, mut framebuffer) = self.backend.bind()?;
            let custom_elements =
                super::render::collect_nested_render_elements(&self.state, renderer);
            let spaces = if spaces_active {
                vec![&self.state.space]
            } else {
                Vec::new()
            };

            render_output::<_, super::render::NestedRenderElement<GlesRenderer>, _, _>(
                &self.state.output,
                renderer,
                &mut framebuffer,
                1.0,
                buffer_age,
                spaces,
                &custom_elements,
                &mut self.damage_tracker,
                clear_color,
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

        self.check_frame_limits();

        Ok(())
    }
}

pub(crate) fn register_event_sources(
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
                .insert_client(stream, client_data.clone())
            {
                Ok(client) => {
                    if let Ok(creds) = client.get_credentials(&loop_state.display.handle()) {
                        client_data.inspect_and_cache_privilege(
                            &creds,
                            &loop_state.state.config.privilege,
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
                if let Err(err) = loop_state.state.dispatch_idle_timers(Some(Duration::ZERO)) {
                    tracing::error!(%err, "Error dispatching idle timers on poll event");
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
        WinitEvent::Input(input_event) => {
            loop_state.state.process_input_activity();
            match input_event {
                InputEvent::Keyboard { event } => loop_state.handle_keyboard_event(event),
                InputEvent::PointerMotionAbsolute { event } => {
                    loop_state.handle_pointer_motion_absolute(event);
                }
                InputEvent::PointerButton { event } => loop_state.handle_pointer_button(event),
                InputEvent::PointerAxis { event } => loop_state.handle_pointer_axis(event),
                _ => {}
            }
        }
    }
}

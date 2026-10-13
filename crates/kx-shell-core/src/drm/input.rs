// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Libinput integration and emergency input event processing for DRM backend.

use smithay::backend::input::{
    Axis, ButtonState, InputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent,
    PointerButtonEvent, PointerMotionAbsoluteEvent, PointerMotionEvent,
};
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::desktop::WindowSurfaceType;
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::reexports::input::Libinput;
use smithay::utils::{Logical, Point, SERIAL_COUNTER, Size};
use tracing::{info, warn};

use crate::error::ShellError;
use crate::state::KynexState;

/// Create and initialize a Libinput input backend attached to the given libseat session.
///
/// # Errors
///
/// Returns [`ShellError::Input`] if libinput initialization or seat assignment fails.
pub fn init_libinput(
    session: LibSeatSession,
    seat_name: &str,
) -> Result<LibinputInputBackend, ShellError> {
    info!(seat = %seat_name, "Initializing Libinput for DRM seat");

    let interface = LibinputSessionInterface::from(session);
    let mut libinput = Libinput::new_with_udev(interface);

    libinput.udev_assign_seat(seat_name).map_err(|()| {
        ShellError::Input(format!("failed to assign Libinput to seat '{seat_name}'"))
    })?;

    let backend = LibinputInputBackend::new(libinput);
    info!("Libinput input backend successfully initialized");
    Ok(backend)
}

/// Clamps pointer logical coordinates to the desktop bounding box `[0, bounds.w] x [0, bounds.h]`.
#[must_use]
pub fn clamp_pointer_location(
    loc: Point<f64, Logical>,
    bounds: Size<i32, Logical>,
) -> Point<f64, Logical> {
    let max_x = f64::from(bounds.w.max(0));
    let max_y = f64::from(bounds.h.max(0));
    Point::from((loc.x.clamp(0.0, max_x), loc.y.clamp(0.0, max_y)))
}

/// Process a single libinput event for DRM backend.
///
/// Dispatches keyboard shortcuts (including emergency quit combos), pointer motion (relative and
/// absolute) with desktop bounding box clamping, pointer button clicks with window raising and focus,
/// and pointer scroll axes.
///
/// Returns `true` if an emergency quit action was triggered, indicating the event loop should exit.
pub fn process_input_event(
    event: InputEvent<LibinputInputBackend>,
    state: &mut KynexState,
    pointer_location: &mut Point<f64, Logical>,
    bounds: Size<i32, Logical>,
) -> bool {
    match event {
        InputEvent::Keyboard { event } => handle_keyboard_event(&event, state),
        InputEvent::PointerMotion { event } => {
            handle_pointer_motion(&event, state, pointer_location, bounds);
            false
        }
        InputEvent::PointerMotionAbsolute { event } => {
            handle_pointer_motion_absolute(&event, state, pointer_location, bounds);
            false
        }
        InputEvent::PointerButton { event } => {
            handle_pointer_button(&event, state, *pointer_location);
            false
        }
        InputEvent::PointerAxis { event } => {
            handle_pointer_axis(&event, state);
            false
        }
        InputEvent::DeviceAdded { device } => {
            info!(name = %device.name(), "Input device added");
            false
        }
        InputEvent::DeviceRemoved { device } => {
            info!(name = %device.name(), "Input device removed");
            false
        }
        _ => false,
    }
}

fn handle_pointer_motion(
    event: &impl PointerMotionEvent<LibinputInputBackend>,
    state: &mut KynexState,
    pointer_location: &mut Point<f64, Logical>,
    bounds: Size<i32, Logical>,
) {
    let new_x = pointer_location.x + event.delta_x();
    let new_y = pointer_location.y + event.delta_y();
    *pointer_location = clamp_pointer_location(Point::from((new_x, new_y)), bounds);

    dispatch_pointer_motion(state, *pointer_location, event.time_msec());
}

fn handle_pointer_motion_absolute(
    event: &impl PointerMotionAbsoluteEvent<LibinputInputBackend>,
    state: &mut KynexState,
    pointer_location: &mut Point<f64, Logical>,
    bounds: Size<i32, Logical>,
) {
    let pos = event.position_transformed(bounds);
    *pointer_location = clamp_pointer_location(pos, bounds);

    dispatch_pointer_motion(state, *pointer_location, event.time_msec());
}

fn dispatch_pointer_motion(
    state: &mut KynexState,
    pointer_location: Point<f64, Logical>,
    time: u32,
) {
    let serial = SERIAL_COUNTER.next_serial();
    let under = state
        .space
        .element_under(pointer_location)
        .and_then(|(window, window_loc)| {
            let relative = pointer_location - window_loc.to_f64();
            window
                .surface_under(relative, WindowSurfaceType::ALL)
                .map(|(surface, surface_offset)| (surface, (window_loc + surface_offset).to_f64()))
        });

    if let Some(pointer) = state.seat.get_pointer() {
        pointer.motion(
            state,
            under,
            &MotionEvent {
                location: pointer_location,
                serial,
                time,
            },
        );
        pointer.frame(state);
    }
}

fn handle_pointer_button(
    event: &impl PointerButtonEvent<LibinputInputBackend>,
    state: &mut KynexState,
    pointer_location: Point<f64, Logical>,
) {
    let serial = SERIAL_COUNTER.next_serial();
    let button = event.button_code();
    let button_state = event.state();
    let time = event.time_msec();

    if button_state == ButtonState::Pressed {
        if let Some((window, _loc)) = state.space.element_under(pointer_location) {
            let window = window.clone();
            state.space.raise_element(&window, true);
            if let Some(surface) = window.toplevel().map(|t| t.wl_surface().clone()) {
                if let Some(&wid) = state.surface_to_window_id.get(&surface) {
                    crate::action::activate_and_focus_window(state, wid);
                } else if let Some(keyboard) = state.seat.get_keyboard() {
                    keyboard.set_focus(state, Some(surface), serial);
                }
            }
        }
    }

    if let Some(pointer) = state.seat.get_pointer() {
        pointer.button(
            state,
            &ButtonEvent {
                serial,
                time,
                button,
                state: button_state,
            },
        );
        pointer.frame(state);
    }
}

fn handle_pointer_axis(
    event: &impl PointerAxisEvent<LibinputInputBackend>,
    state: &mut KynexState,
) {
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

    if let Some(pointer) = state.seat.get_pointer() {
        pointer.axis(state, frame);
        pointer.frame(state);
    }
}

fn handle_keyboard_event(
    event: &impl KeyboardKeyEvent<LibinputInputBackend>,
    state: &mut KynexState,
) -> bool {
    let serial = SERIAL_COUNTER.next_serial();
    let Some(keyboard) = state.seat.get_keyboard() else {
        warn!("Received keyboard event but seat has no keyboard initialized");
        return false;
    };

    let action = keyboard.input(
        state,
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
        if crate::action::execute_compositor_action(state, &action) {
            info!("Compositor shutdown requested via Libinput action");
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamp_pointer_location_within_bounds() {
        let loc = Point::from((100.0, 200.0));
        let bounds = Size::from((1920, 1080));
        assert_eq!(
            clamp_pointer_location(loc, bounds),
            Point::from((100.0, 200.0))
        );
    }

    #[test]
    fn test_clamp_pointer_location_negative() {
        let loc = Point::from((-50.0, -10.0));
        let bounds = Size::from((1920, 1080));
        assert_eq!(clamp_pointer_location(loc, bounds), Point::from((0.0, 0.0)));
    }

    #[test]
    fn test_clamp_pointer_location_exceeding_bounds() {
        let loc = Point::from((2500.0, 1500.0));
        let bounds = Size::from((1920, 1080));
        assert_eq!(
            clamp_pointer_location(loc, bounds),
            Point::from((1920.0, 1080.0))
        );
    }

    #[test]
    fn test_clamp_pointer_location_zero_bounds() {
        let loc = Point::from((100.0, 100.0));
        let bounds = Size::from((0, 0));
        assert_eq!(clamp_pointer_location(loc, bounds), Point::from((0.0, 0.0)));
    }

    #[test]
    fn test_clamp_pointer_location_multi_monitor_bounds() {
        let loc = Point::from((3500.0, 1000.0));
        let bounds = Size::from((3840, 1080));
        assert_eq!(
            clamp_pointer_location(loc, bounds),
            Point::from((3500.0, 1000.0))
        );
    }
}

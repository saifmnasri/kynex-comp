// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Libinput integration and emergency input event processing for DRM backend.

use smithay::backend::input::{InputEvent, KeyState, KeyboardKeyEvent};
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::input::keyboard::FilterResult;
use smithay::reexports::input::Libinput;
use smithay::utils::SERIAL_COUNTER;
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

/// Process a single libinput event for DRM backend.
///
/// Returns `true` if an emergency quit action was triggered, indicating the event loop should exit.
pub fn process_input_event(
    event: InputEvent<LibinputInputBackend>,
    state: &mut KynexState,
) -> bool {
    match event {
        InputEvent::Keyboard { event } => handle_keyboard_event(&event, state),
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

    if let Some(kx_input::CompositorAction::Quit) = action {
        info!("Emergency exit key combination triggered via Libinput; shutting down");
        return true;
    }

    false
}

// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Pointer grab implementations for interactive window management.
//!
//! Provides [`MoveSurfaceGrab`] and [`ResizeSurfaceGrab`], intercepting pointer
//! motions and buttons during interactive move and resize operations.

pub mod move_grab;
pub mod resize_grab;
pub mod resize_math;

use smithay::input::keyboard::ModifiersState;
use smithay::utils::{Logical, Point, Serial};

use crate::state::KynexState;

pub use move_grab::{MoveSurfaceGrab, handle_client_move_request, start_compositor_move_grab};
pub use resize_grab::{
    ResizeSurfaceGrab, handle_client_resize_request, start_compositor_resize_grab,
};
pub use resize_math::{calculate_resize_geometry, detect_resize_edge};

/// Linux input button code for mouse left button (`BTN_LEFT`).
pub const BTN_LEFT: u32 = 0x110;

/// Linux input button code for mouse right button (`BTN_RIGHT`).
pub const BTN_RIGHT: u32 = 0x111;

/// Converts Smithay [`ModifiersState`] into [`kx_input::KeyModifiers`].
#[must_use]
pub fn modifiers_to_key_modifiers(mods: ModifiersState) -> kx_input::KeyModifiers {
    let mut key_mods = kx_input::KeyModifiers::empty();
    if mods.ctrl {
        key_mods |= kx_input::KeyModifiers::CTRL;
    }
    if mods.shift {
        key_mods |= kx_input::KeyModifiers::SHIFT;
    }
    if mods.alt {
        key_mods |= kx_input::KeyModifiers::ALT;
    }
    if mods.logo {
        key_mods |= kx_input::KeyModifiers::LOGO;
    }
    key_mods
}

/// Checks whether the primary modifier configured in state is active.
#[must_use]
pub fn is_primary_modifier_active(state: &KynexState) -> bool {
    state
        .seat
        .get_keyboard()
        .map(|kb| kb.modifier_state())
        .is_some_and(|mods| {
            let key_mods = modifiers_to_key_modifiers(mods);
            state.binding_matcher.primary_modifier().is_active(key_mods)
        })
}

/// Attempts to start a compositor move or resize grab if the primary modifier is active.
///
/// Returns `true` if an interactive grab was initiated, intercepting further click handling.
pub fn try_start_compositor_grab(
    state: &mut KynexState,
    location: Point<f64, Logical>,
    button: u32,
    serial: Serial,
) -> bool {
    if !is_primary_modifier_active(state) || (button != BTN_LEFT && button != BTN_RIGHT) {
        return false;
    }

    let Some((window, _loc)) = state.space.element_under(location) else {
        return false;
    };

    let window = window.clone();
    state.space.raise_element(&window, true);

    let Some(surface) = window.toplevel().map(|t| t.wl_surface().clone()) else {
        return false;
    };

    let Some(&window_id) = state.surface_to_window_id.get(&surface) else {
        return false;
    };

    crate::action::activate_and_focus_window(state, window_id);

    if button == BTN_LEFT {
        start_compositor_move_grab(state, window_id, location, button, serial);
        true
    } else if button == BTN_RIGHT {
        let Some(win) = state.window_manager.window(window_id) else {
            return false;
        };
        let edges = detect_resize_edge(location, win.geometry);
        start_compositor_resize_grab(state, window_id, location, edges, button, serial);
        true
    } else {
        false
    }
}

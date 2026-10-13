// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Pointer grab implementations for interactive window management.
//!
//! Provides [`MoveSurfaceGrab`], which intercepts pointer motions
//! and buttons during interactive window move operations.

use kx_layout::{WindowGeometry, WindowId, clamp_to_reachability};
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
    GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
    GestureSwipeUpdateEvent, GrabStartData, MotionEvent, PointerGrab, PointerInnerHandle,
    RelativeMotionEvent,
};
use smithay::reexports::wayland_server::protocol::wl_seat;
use smithay::utils::{Logical, Point, Serial};
use smithay::wayland::shell::xdg::ToplevelSurface;

use crate::state::KynexState;
use crate::window::{primary_output_usable_geometry, sync_layout_to_space};

/// Linux input button code for mouse left button (`BTN_LEFT`).
pub const BTN_LEFT: u32 = 0x110;

/// Linux input button code for mouse right button (`BTN_RIGHT`).
pub const BTN_RIGHT: u32 = 0x111;

/// Converts Smithay [`ModifiersState`](smithay::input::keyboard::ModifiersState) into [`kx_input::KeyModifiers`].
#[must_use]
pub fn modifiers_to_key_modifiers(
    mods: smithay::input::keyboard::ModifiersState,
) -> kx_input::KeyModifiers {
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

/// Pointer grab for interactive window movement.
pub struct MoveSurfaceGrab {
    start_data: GrabStartData<KynexState>,
    window_id: WindowId,
    initial_window_location: Point<i32, Logical>,
}

impl MoveSurfaceGrab {
    /// Construct a new move grab for a window.
    #[must_use]
    pub fn new(
        start_data: GrabStartData<KynexState>,
        window_id: WindowId,
        initial_window_location: Point<i32, Logical>,
    ) -> Self {
        Self {
            start_data,
            window_id,
            initial_window_location,
        }
    }
}

impl PointerGrab<KynexState> for MoveSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut KynexState,
        handle: &mut PointerInnerHandle<'_, KynexState>,
        _focus: Option<(
            <KynexState as smithay::input::SeatHandler>::PointerFocus,
            Point<f64, Logical>,
        )>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);

        let Some(win) = data.window_manager.window(self.window_id) else {
            handle.unset_grab(self, data, event.serial, event.time, true);
            return;
        };

        #[expect(
            clippy::cast_possible_truncation,
            reason = "screen delta in logical pixels fits safely in i32"
        )]
        let dx = (event.location.x - self.start_data.location.x) as i32;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "screen delta in logical pixels fits safely in i32"
        )]
        let dy = (event.location.y - self.start_data.location.y) as i32;

        let target_x = self.initial_window_location.x.saturating_add(dx);
        let target_y = self.initial_window_location.y.saturating_add(dy);

        let target_geo =
            WindowGeometry::new(target_x, target_y, win.geometry.width, win.geometry.height);
        let usable_area = primary_output_usable_geometry(data);
        let clamped = clamp_to_reachability(target_geo, usable_area);

        let _ = data
            .window_manager
            .set_window_geometry(self.window_id, clamped);
        sync_layout_to_space(data);
    }

    fn relative_motion(
        &mut self,
        data: &mut KynexState,
        handle: &mut PointerInnerHandle<'_, KynexState>,
        _focus: Option<(
            <KynexState as smithay::input::SeatHandler>::PointerFocus,
            Point<f64, Logical>,
        )>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, None, event);
    }

    fn button(
        &mut self,
        data: &mut KynexState,
        handle: &mut PointerInnerHandle<'_, KynexState>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);
        if smithay::backend::input::ButtonState::Released == event.state
            && (event.button == self.start_data.button || handle.current_pressed().is_empty())
        {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(
        &mut self,
        data: &mut KynexState,
        handle: &mut PointerInnerHandle<'_, KynexState>,
        details: AxisFrame,
    ) {
        handle.axis(data, details);
    }

    fn frame(&mut self, data: &mut KynexState, handle: &mut PointerInnerHandle<'_, KynexState>) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut KynexState,
        handle: &mut PointerInnerHandle<'_, KynexState>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut KynexState,
        handle: &mut PointerInnerHandle<'_, KynexState>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut KynexState,
        handle: &mut PointerInnerHandle<'_, KynexState>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut KynexState,
        handle: &mut PointerInnerHandle<'_, KynexState>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut KynexState,
        handle: &mut PointerInnerHandle<'_, KynexState>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut KynexState,
        handle: &mut PointerInnerHandle<'_, KynexState>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut KynexState,
        handle: &mut PointerInnerHandle<'_, KynexState>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut KynexState,
        handle: &mut PointerInnerHandle<'_, KynexState>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }

    fn start_data(&self) -> &GrabStartData<KynexState> {
        &self.start_data
    }

    fn unset(&mut self, _data: &mut KynexState) {}
}

/// Handles a client request (`xdg_toplevel.move`) to start an interactive move grab.
pub fn handle_client_move_request(
    state: &mut KynexState,
    surface: &ToplevelSurface,
    _seat: &wl_seat::WlSeat,
    serial: Serial,
) {
    let Some(pointer) = state.seat.get_pointer() else {
        return;
    };

    if !pointer.has_grab(serial) {
        tracing::debug!(?serial, "Rejected move_request with invalid serial");
        return;
    }

    let Some(start_data) = pointer.grab_start_data() else {
        return;
    };

    let Some(&window_id) = state.surface_to_window_id.get(surface.wl_surface()) else {
        return;
    };

    let Some(win) = state.window_manager.window(window_id) else {
        return;
    };

    if win.state != kx_layout::window::WindowState::Normal {
        tracing::debug!(?window_id, ?win.state, "Ignoring move request on non-normal window");
        return;
    }

    let initial_location = Point::from((win.geometry.x, win.geometry.y));
    let grab = MoveSurfaceGrab::new(start_data, window_id, initial_location);
    pointer.set_grab(state, grab, serial, smithay::input::pointer::Focus::Clear);
}

/// Initiates an interactive move grab from a compositor action (e.g. modifier + left mouse drag).
pub fn start_compositor_move_grab(
    state: &mut KynexState,
    window_id: WindowId,
    location: Point<f64, Logical>,
    button: u32,
    serial: Serial,
) {
    let Some(pointer) = state.seat.get_pointer() else {
        return;
    };

    let Some(win) = state.window_manager.window(window_id) else {
        return;
    };

    let initial_location = Point::from((win.geometry.x, win.geometry.y));
    let start_data = GrabStartData {
        focus: None,
        button,
        location,
    };

    let grab = MoveSurfaceGrab::new(start_data, window_id, initial_location);
    pointer.set_grab(state, grab, serial, smithay::input::pointer::Focus::Clear);
}

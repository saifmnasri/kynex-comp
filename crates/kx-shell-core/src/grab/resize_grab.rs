// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Pointer grab for interactive window resize operations.

use kx_layout::{WindowGeometry, WindowId};
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
    GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
    GestureSwipeUpdateEvent, GrabStartData, MotionEvent, PointerGrab, PointerInnerHandle,
    RelativeMotionEvent,
};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_seat;
use smithay::utils::{Logical, Point, Serial};
use smithay::wayland::shell::xdg::ToplevelSurface;

use super::resize_math::calculate_resize_geometry;
use crate::state::KynexState;
use crate::window::{primary_output_usable_geometry, sync_layout_to_space};

/// Pointer grab for interactive window resize.
pub struct ResizeSurfaceGrab {
    start_data: GrabStartData<KynexState>,
    window_id: WindowId,
    edges: xdg_toplevel::ResizeEdge,
    initial_window_geometry: WindowGeometry,
    last_window_geometry: WindowGeometry,
}

impl ResizeSurfaceGrab {
    /// Construct a new resize grab for a window.
    #[must_use]
    pub fn new(
        start_data: GrabStartData<KynexState>,
        window_id: WindowId,
        edges: xdg_toplevel::ResizeEdge,
        initial_window_geometry: WindowGeometry,
    ) -> Self {
        Self {
            start_data,
            window_id,
            edges,
            initial_window_geometry,
            last_window_geometry: initial_window_geometry,
        }
    }
}

impl PointerGrab<KynexState> for ResizeSurfaceGrab {
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

        let usable_area = primary_output_usable_geometry(data);
        let target_geo = calculate_resize_geometry(
            self.initial_window_geometry,
            &win.constraints,
            self.edges,
            dx,
            dy,
            usable_area,
        );

        self.last_window_geometry = target_geo;
        let _ = data
            .window_manager
            .set_window_geometry(self.window_id, target_geo);
        sync_layout_to_space(data);

        if let Some(toplevel) = data
            .window_map
            .get(&self.window_id)
            .and_then(|w| w.toplevel().cloned())
        {
            toplevel.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
                state.size = Some((target_geo.width, target_geo.height).into());
            });
            toplevel.send_configure();
        }
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

    fn unset(&mut self, data: &mut KynexState) {
        if let Some(toplevel) = data
            .window_map
            .get(&self.window_id)
            .and_then(|w| w.toplevel().cloned())
        {
            toplevel.with_pending_state(|state| {
                state.states.unset(xdg_toplevel::State::Resizing);
                state.size = Some(
                    (
                        self.last_window_geometry.width,
                        self.last_window_geometry.height,
                    )
                        .into(),
                );
            });
            toplevel.send_configure();
        }
    }
}

/// Handles a client request (`xdg_toplevel.resize`) to start an interactive resize grab.
pub fn handle_client_resize_request(
    state: &mut KynexState,
    surface: &ToplevelSurface,
    _seat: &wl_seat::WlSeat,
    serial: Serial,
    edges: xdg_toplevel::ResizeEdge,
) {
    let Some(pointer) = state.seat.get_pointer() else {
        return;
    };

    if !pointer.has_grab(serial) {
        tracing::debug!(?serial, "Rejected resize_request with invalid serial");
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
        tracing::debug!(
            ?window_id,
            ?win.state,
            "Ignoring resize request on non-normal window"
        );
        return;
    }

    if edges == xdg_toplevel::ResizeEdge::None {
        return;
    }

    surface.with_pending_state(|toplevel_state| {
        toplevel_state.states.set(xdg_toplevel::State::Resizing);
    });
    surface.send_configure();

    let grab = ResizeSurfaceGrab::new(start_data, window_id, edges, win.geometry);
    pointer.set_grab(state, grab, serial, smithay::input::pointer::Focus::Clear);
}

/// Initiates an interactive resize grab from a compositor action (e.g. modifier + right mouse drag).
pub fn start_compositor_resize_grab(
    state: &mut KynexState,
    window_id: WindowId,
    location: Point<f64, Logical>,
    edges: xdg_toplevel::ResizeEdge,
    button: u32,
    serial: Serial,
) {
    let Some(pointer) = state.seat.get_pointer() else {
        return;
    };

    let Some(win) = state.window_manager.window(window_id) else {
        return;
    };

    if let Some(toplevel) = state
        .window_map
        .get(&window_id)
        .and_then(|w| w.toplevel().cloned())
    {
        toplevel.with_pending_state(|toplevel_state| {
            toplevel_state.states.set(xdg_toplevel::State::Resizing);
        });
        toplevel.send_configure();
    }

    let start_data = GrabStartData {
        focus: None,
        button,
        location,
    };

    let grab = ResizeSurfaceGrab::new(start_data, window_id, edges, win.geometry);
    pointer.set_grab(state, grab, serial, smithay::input::pointer::Focus::Clear);
}

// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Pointer constraints protocol handler (`zwp_pointer_constraints_v1`).
//!
//! Provides support for the `zwp_pointer_constraints_v1` interface, enabling
//! clients to request that the pointer cursor be confined to a specific surface region
//! or locked in place (e.g. for first-person games, CAD viewers, or 3D navigation).
//!
//! # Security and Focus Invariants
//!
//! Per Rule 05 and the Wayland protocol specification:
//! - Pointer lock and confinement constraints are only activated when the surface
//!   currently holds active pointer focus.
//! - When pointer focus leaves a constrained surface, the constraint is automatically
//!   deactivated by Smithay.
//! - When a client commits a cursor position hint on a locked pointer, the hint is
//!   recorded and surfaced for potential warp handling.

use smithay::delegate_pointer_constraints;
use smithay::input::pointer::PointerHandle;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};
pub use smithay::wayland::pointer_constraints::{
    PointerConstraint, PointerConstraintRef, PointerConstraintsHandler, PointerConstraintsState,
    with_pointer_constraint,
};

use crate::state::KynexState;

/// Initializes the `zwp_pointer_constraints_v1` global on the Wayland display.
///
/// Registers version 1 of the global.
#[must_use]
pub fn init_pointer_constraints_global(dh: &DisplayHandle) -> PointerConstraintsState {
    tracing::info!("Initializing zwp_pointer_constraints_v1 global (version 1)");
    PointerConstraintsState::new::<KynexState>(dh)
}

/// Helper to activate any pending pointer constraint on `surface` if the pointer is focused on it.
pub fn maybe_activate_pointer_constraint(surface: &WlSurface, pointer: &PointerHandle<KynexState>) {
    if pointer.current_focus().as_ref() != Some(surface) {
        return;
    }
    with_pointer_constraint(surface, pointer, |constraint| {
        if let Some(constraint) = constraint {
            if !constraint.is_active() {
                constraint.activate();
            }
        }
    });
}

impl PointerConstraintsHandler for KynexState {
    fn new_constraint(&mut self, surface: &WlSurface, pointer: &PointerHandle<Self>) {
        tracing::debug!(
            surface = ?surface.id(),
            "New pointer constraint request received"
        );

        maybe_activate_pointer_constraint(surface, pointer);
    }

    fn cursor_position_hint(
        &mut self,
        surface: &WlSurface,
        _pointer: &PointerHandle<Self>,
        location: Point<f64, Logical>,
    ) {
        tracing::trace!(
            surface = ?surface.id(),
            ?location,
            "Client committed cursor position hint"
        );
    }
}

delegate_pointer_constraints!(KynexState);

#[cfg(test)]
mod tests {
    use smithay::reexports::wayland_server::Display;

    use super::*;

    #[test]
    fn test_init_pointer_constraints_global_success() {
        let display = Display::<KynexState>::new().expect("failed to create test wayland display");
        let _state = init_pointer_constraints_global(&display.handle());
    }
}

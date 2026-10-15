// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Cursor shape protocol handler (`wp_cursor_shape_v1`).
//!
//! Provides support for the `wp_cursor_shape_manager_v1` interface, enabling
//! clients to request server-side named cursor shapes (e.g. `default`, `pointer`,
//! `text`, `crosshair`, `wait`) rather than uploading pixel buffers.
//!
//! When a client sets a cursor shape on a pointer device, the shape is converted
//! to a standard [`smithay::input::pointer::CursorIcon`] and forwarded to
//! [`smithay::input::SeatHandler::cursor_image`] on [`KynexState`].

use smithay::delegate_cursor_shape;
use smithay::reexports::wayland_server::DisplayHandle;
pub use smithay::wayland::cursor_shape::CursorShapeManagerState;
use smithay::wayland::tablet_manager::TabletSeatHandler;

use crate::state::KynexState;

/// Initializes the `wp_cursor_shape_manager_v1` global on the Wayland display.
///
/// Registers version 2 of the global, providing server-side cursor shapes
/// for both pointer and tablet-tool devices.
#[must_use]
pub fn init_cursor_shape_global(dh: &DisplayHandle) -> CursorShapeManagerState {
    tracing::info!("Initializing wp_cursor_shape_manager_v1 global (version 2)");
    CursorShapeManagerState::new::<KynexState>(dh)
}

impl TabletSeatHandler for KynexState {}

delegate_cursor_shape!(KynexState);

#[cfg(test)]
mod tests {
    use smithay::reexports::wayland_server::Display;

    use super::*;

    #[test]
    fn test_init_cursor_shape_global_success() {
        let display = Display::<KynexState>::new().expect("failed to create test wayland display");
        let _state = init_cursor_shape_global(&display.handle());
    }
}

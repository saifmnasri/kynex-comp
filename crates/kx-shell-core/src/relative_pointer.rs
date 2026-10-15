// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Wayland Relative Pointer protocol handler (`zwp_relative_pointer_v1`).
//!
//! Provides relative (unaccelerated and accelerated) pointer motion events
//! to clients that require raw pointer deltas (such as 3D games, camera controls,
//! and FPS input handling).

use smithay::delegate_relative_pointer;
use smithay::reexports::wayland_server::DisplayHandle;
pub use smithay::wayland::relative_pointer::RelativePointerManagerState;

use crate::state::KynexState;

/// Initializes and registers the `zwp_relative_pointer_manager_v1` global.
///
/// Registers version 1 of the global on the Wayland display.
#[must_use]
pub fn init_relative_pointer_global(dh: &DisplayHandle) -> RelativePointerManagerState {
    tracing::info!("Initializing zwp_relative_pointer_manager_v1 global (version 1)");
    RelativePointerManagerState::new::<KynexState>(dh)
}

delegate_relative_pointer!(KynexState);

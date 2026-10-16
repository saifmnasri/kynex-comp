// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Wayland primary selection protocol delegate (`zwp_primary_selection_device_manager_v1`).

use smithay::delegate_primary_selection;
use smithay::wayland::selection::primary_selection::{
    PrimarySelectionHandler, PrimarySelectionState,
};
use tracing::info;

use crate::state::KynexState;

impl PrimarySelectionHandler for KynexState {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}

delegate_primary_selection!(KynexState);

/// Initializes the primary selection global (`zwp_primary_selection_device_manager_v1`).
pub fn init_primary_selection_global(
    dh: &smithay::reexports::wayland_server::DisplayHandle,
) -> PrimarySelectionState {
    info!("Initializing zwp_primary_selection_device_manager_v1 global (version 1)");
    PrimarySelectionState::new::<KynexState>(dh)
}

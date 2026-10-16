// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Wayland alpha modifier protocol delegate (`wp_alpha_modifier_v1`).

use smithay::delegate_alpha_modifier;
use smithay::wayland::alpha_modifier::AlphaModifierState;
use tracing::info;

use crate::state::KynexState;

delegate_alpha_modifier!(KynexState);

/// Initializes the alpha modifier global (`wp_alpha_modifier_v1`).
pub fn init_alpha_modifier_global(
    dh: &smithay::reexports::wayland_server::DisplayHandle,
) -> AlphaModifierState {
    info!("Initializing wp_alpha_modifier_v1 global (version 1)");
    AlphaModifierState::new::<KynexState>(dh)
}

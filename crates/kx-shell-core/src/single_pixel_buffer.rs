// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Wayland single-pixel buffer protocol delegate (`wp_single_pixel_buffer_manager_v1`).

use smithay::delegate_single_pixel_buffer;
use smithay::wayland::single_pixel_buffer::SinglePixelBufferState;
use tracing::info;

use crate::state::KynexState;

delegate_single_pixel_buffer!(KynexState);

/// Initializes the single-pixel buffer global (`wp_single_pixel_buffer_manager_v1`).
pub fn init_single_pixel_buffer_global(
    dh: &smithay::reexports::wayland_server::DisplayHandle,
) -> SinglePixelBufferState {
    info!("Initializing wp_single_pixel_buffer_manager_v1 global (version 1)");
    SinglePixelBufferState::new::<KynexState>(dh)
}

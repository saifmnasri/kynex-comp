// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Content-type protocol handler (`wp_content_type_v1`).

use smithay::delegate_content_type;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::content_type::ContentTypeState;
use tracing::info;

use crate::state::KynexState;

delegate_content_type!(KynexState);

/// Initializes the `wp_content_type_manager_v1` global on the Wayland display.
pub fn init_content_type_global(dh: &DisplayHandle) -> ContentTypeState {
    info!("Initializing wp_content_type_manager_v1 global (version 1)");
    ContentTypeState::new::<KynexState>(dh)
}

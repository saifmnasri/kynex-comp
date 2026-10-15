// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Viewport cropping and scaling protocol handler (`wp_viewporter`).
//!
//! Provides support for the `wp_viewporter` interface, enabling clients to specify
//! source buffer cropping rectangles and destination logical scaling sizes for
//! video playback, games, and UI element scaling.

use smithay::delegate_viewporter;
use smithay::reexports::wayland_server::DisplayHandle;
pub use smithay::wayland::viewporter::{
    ViewportCachedState, ViewportState, ViewporterState, ensure_viewport_valid,
};

use crate::state::KynexState;

/// Initializes the `wp_viewporter` global on the Wayland display.
///
/// Registers the `wp_viewporter` global (version 1) and enables surface
/// cropping and scaling for all connected clients.
#[must_use]
pub fn init_viewporter_global(dh: &DisplayHandle) -> ViewporterState {
    tracing::info!("Initializing wp_viewporter global (version 1)");
    ViewporterState::new::<KynexState>(dh)
}

delegate_viewporter!(KynexState);

#[cfg(test)]
mod tests {
    use smithay::reexports::wayland_server::Display;

    use super::*;

    #[test]
    fn test_init_viewporter_global_success() {
        let display = Display::<KynexState>::new().expect("failed to create test wayland display");
        let dh = display.handle();
        let _state = init_viewporter_global(&dh);
    }
}

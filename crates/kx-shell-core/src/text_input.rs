// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Wayland Text Input protocol handler (`zwp_text_input_v3`).
//!
//! Provides text input capabilities, composing text, surrounding text context,
//! and cursor rectangles to Wayland clients (such as GTK, Qt, and browser text fields).

use smithay::delegate_text_input_manager;
use smithay::reexports::wayland_server::DisplayHandle;
pub use smithay::wayland::text_input::{TextInputHandle, TextInputManagerState, TextInputSeat};

use crate::state::KynexState;

/// Initializes and registers the `zwp_text_input_manager_v3` global.
///
/// Registers version 1 of the global on the Wayland display.
#[must_use]
pub fn init_text_input_global(dh: &DisplayHandle) -> TextInputManagerState {
    tracing::info!("Initializing zwp_text_input_manager_v3 global (version 1)");
    TextInputManagerState::new::<KynexState>(dh)
}

delegate_text_input_manager!(KynexState);

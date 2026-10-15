// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Wayland Input Method protocol handler (`zwp_input_method_v2`).
//!
//! Provides support for privileged input method editors (e.g., `Fcitx`, `IBus`).
//! Access to this global is strictly gated by client peer credentials and allow-listing.

use smithay::delegate_input_method_manager;
use smithay::desktop::PopupKind;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DisplayHandle};
use smithay::utils::{Logical, Rectangle};
pub use smithay::wayland::input_method::{
    InputMethodHandle, InputMethodHandler, InputMethodManagerState, InputMethodSeat, PopupSurface,
};

use crate::client::ClientState;
use crate::state::KynexState;

/// Initializes and registers the `zwp_input_method_manager_v2` privileged global.
///
/// Gated by a `can_view` security filter checking whether the client is authorized
/// via allow-list or privileged client configuration.
#[must_use]
pub fn init_input_method_global(dh: &DisplayHandle) -> InputMethodManagerState {
    tracing::info!("Initializing zwp_input_method_manager_v2 privileged global (version 1)");
    InputMethodManagerState::new::<KynexState, _>(dh, |client: &Client| {
        client
            .get_data::<ClientState>()
            .is_some_and(ClientState::is_input_method_allowed)
    })
}

impl InputMethodHandler for KynexState {
    fn new_popup(&mut self, surface: PopupSurface) {
        let kind = PopupKind::from(surface);
        let _ = self.popups.track_popup(kind);
    }

    fn dismiss_popup(&mut self, _surface: PopupSurface) {
        // Popups are dismissed when destroyed or unmapped
    }

    fn popup_repositioned(&mut self, _surface: PopupSurface) {
        // Popup position updated
    }

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, Logical> {
        if let Some(&wid) = self.surface_to_window_id.get(parent) {
            if let Some(window) = self.window_map.get(&wid) {
                return window.geometry();
            }
        }
        for window in self.window_map.values() {
            let mut belongs = false;
            window.with_surfaces(|s, _| {
                if s == parent {
                    belongs = true;
                }
            });
            if belongs {
                return window.geometry();
            }
        }
        Rectangle::new((0, 0).into(), (0, 0).into())
    }
}

delegate_input_method_manager!(KynexState);

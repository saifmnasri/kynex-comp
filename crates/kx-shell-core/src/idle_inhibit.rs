// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Wayland Idle Inhibit protocol handler (`zwp_idle_inhibit_unstable_v1`).
//!
//! Allows clients to inhibit the compositor's idle behavior (such as display blanking,
//! screen lock, or system suspend) when specific surfaces are visible and active
//! (e.g. video playback or gaming).
//!
//! Classified as a Restricted global per `docs/SECURITY.md`.

use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
pub use smithay::wayland::idle_inhibit::{IdleInhibitHandler, IdleInhibitManagerState};

use crate::state::KynexState;

smithay::delegate_idle_inhibit!(KynexState);

impl IdleInhibitHandler for KynexState {
    fn inhibit(&mut self, surface: WlSurface) {
        tracing::info!(surface = ?surface.id(), "Client requested idle inhibitor on surface");
        if !self.idle_inhibitors.iter().any(|s| s == &surface) {
            self.idle_inhibitors.push(surface);
        }
    }

    fn uninhibit(&mut self, surface: WlSurface) {
        tracing::info!(surface = ?surface.id(), "Idle inhibitor removed on surface");
        self.idle_inhibitors
            .retain(|s| s.is_alive() && s != &surface);
    }
}

impl KynexState {
    /// Returns true if any client currently holds an active idle inhibitor.
    #[must_use]
    pub fn is_idle_inhibited(&self) -> bool {
        !self.idle_inhibitors.is_empty()
    }
}

/// Initializes and registers the `zwp_idle_inhibit_manager_v1` global.
#[must_use]
pub fn init_idle_inhibit_global(dh: &DisplayHandle) -> IdleInhibitManagerState {
    tracing::info!("Initializing zwp_idle_inhibit_manager_v1 global (version 1)");
    IdleInhibitManagerState::new::<KynexState>(dh)
}

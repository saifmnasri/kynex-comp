// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Wayland Session Lock protocol handler (`ext_session_lock_v1`).
//!
//! Provides support for privileged lock screen clients.
//! Access to this global is strictly gated by client peer credentials and allow-listing.
//! Enforces fail-closed security per Rule 05: if the session is locked and the lock
//! client terminates without calling `UnlockAndDestroy`, the compositor remains locked.

use smithay::delegate_session_lock;
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::{Client, DisplayHandle};
use smithay::utils::Size;
pub use smithay::wayland::session_lock::{
    LockSurface, LockSurfaceState, SessionLockHandler, SessionLockManagerState, SessionLockState,
    SessionLocker,
};

use crate::client::ClientState;
use crate::state::KynexState;

/// Initializes and registers the `ext_session_lock_manager_v1` privileged global.
///
/// Gated by a `can_view` security filter checking whether the client is authorized
/// via allow-list or privileged client configuration.
#[must_use]
pub fn init_session_lock_global(dh: &DisplayHandle) -> SessionLockManagerState {
    tracing::info!("Initializing ext_session_lock_manager_v1 privileged global (version 1)");
    SessionLockManagerState::new::<KynexState, _>(dh, |client: &Client| {
        client
            .get_data::<ClientState>()
            .is_some_and(ClientState::is_session_lock_allowed)
    })
}

impl SessionLockHandler for KynexState {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.session_lock_state
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        tracing::info!("Session lock active; locking compositor");
        self.is_session_locked = true;
        confirmation.lock();
    }

    fn unlock(&mut self) {
        tracing::info!("Session lock released; unlocking compositor");
        self.is_session_locked = false;
        self.lock_surfaces.clear();
    }

    fn new_surface(&mut self, surface: LockSurface, output: WlOutput) {
        let output_obj = Output::from_resource(&output);
        let size = output_obj
            .and_then(|out| self.space.output_geometry(&out))
            .map_or_else(
                || Size::from((1920, 1080)),
                |geo| {
                    let width = u32::try_from(geo.size.w.max(0)).unwrap_or(1920);
                    let height = u32::try_from(geo.size.h.max(0)).unwrap_or(1080);
                    Size::from((width, height))
                },
            );

        surface.with_pending_state(|state| {
            state.size = Some(size);
        });

        self.lock_surfaces.push(surface);
    }
}

delegate_session_lock!(KynexState);

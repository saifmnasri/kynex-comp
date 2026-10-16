// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Wayland Foreign Toplevel List protocol handler (`ext_foreign_toplevel_list_v1`).
//!
//! Provides top-level window listing and state events for taskbars, docks, and pagers.
//! Classified as a Restricted global per Rule 05 / `docs/SECURITY.md`, gated by
//! peer credentials and allow-listing via `ClientState::is_foreign_toplevel_allowed`.

use smithay::delegate_foreign_toplevel_list;
use smithay::reexports::wayland_server::{Client, DisplayHandle};
pub use smithay::wayland::foreign_toplevel_list::{
    ForeignToplevelHandle, ForeignToplevelListHandler, ForeignToplevelListState,
    ForeignToplevelWeakHandle,
};

use crate::client::ClientState;
use crate::state::KynexState;

/// Initializes and registers the `ext_foreign_toplevel_list_v1` global.
///
/// Restricted global gated by `can_view` security filter checking whether the client
/// is authorized via allow-list or privileged configuration.
#[must_use]
pub fn init_foreign_toplevel_list_global(dh: &DisplayHandle) -> ForeignToplevelListState {
    tracing::info!("Initializing ext_foreign_toplevel_list_v1 global (version 1)");
    ForeignToplevelListState::new_with_filter::<KynexState>(dh, |client: &Client| {
        client
            .get_data::<ClientState>()
            .is_some_and(ClientState::is_foreign_toplevel_allowed)
    })
}

/// Registers a new foreign toplevel handle for the given window ID.
pub fn register_foreign_toplevel(
    state: &mut KynexState,
    window_id: kx_layout::WindowId,
    title: &str,
    app_id: &str,
) {
    let handle = state
        .foreign_toplevel_list_state
        .new_toplevel::<KynexState>(title, app_id);
    state.foreign_toplevel_handles.insert(window_id, handle);
}

/// Closes and removes the foreign toplevel handle for the given window ID.
pub fn unregister_foreign_toplevel(state: &mut KynexState, window_id: kx_layout::WindowId) {
    if let Some(handle) = state.foreign_toplevel_handles.remove(&window_id) {
        state.foreign_toplevel_list_state.remove_toplevel(&handle);
    }
}

/// Updates the advertised title of a foreign toplevel handle.
pub fn update_foreign_toplevel_title(
    state: &KynexState,
    window_id: kx_layout::WindowId,
    title: &str,
) {
    if let Some(handle) = state.foreign_toplevel_handles.get(&window_id) {
        handle.send_title(title);
        handle.send_done();
    }
}

/// Updates the advertised `app_id` of a foreign toplevel handle.
pub fn update_foreign_toplevel_app_id(
    state: &KynexState,
    window_id: kx_layout::WindowId,
    app_id: &str,
) {
    if let Some(handle) = state.foreign_toplevel_handles.get(&window_id) {
        handle.send_app_id(app_id);
        handle.send_done();
    }
}

impl ForeignToplevelListHandler for KynexState {
    fn foreign_toplevel_list_state(&mut self) -> &mut ForeignToplevelListState {
        &mut self.foreign_toplevel_list_state
    }
}

delegate_foreign_toplevel_list!(KynexState);

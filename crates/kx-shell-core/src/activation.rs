// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! XDG Activation protocol implementation (`xdg-activation-v1`).
//!
//! Enables cooperative focus transfer between applications with token expiration
//! and per-client resource quota protection against denial-of-service token flooding.

use std::time::Duration;

use smithay::delegate_xdg_activation;
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::xdg_activation::{
    XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
};

use crate::client::ClientState;
use crate::state::KynexState;

/// Default expiration duration for activation tokens (30 seconds).
pub const DEFAULT_ACTIVATION_EXPIRATION: Duration = Duration::from_secs(30);

impl XdgActivationHandler for KynexState {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.xdg_activation_state
    }

    fn token_created(&mut self, _token: XdgActivationToken, data: XdgActivationTokenData) -> bool {
        self.purge_expired_tokens();

        let Some(client_id) = data.client_id else {
            return true;
        };

        if let Ok(client_data) = self
            .display_handle
            .backend_handle()
            .get_client_data(client_id)
        {
            if let Some(client_data) = client_data.downcast_ref::<ClientState>() {
                if !client_data.try_increment_activation_token() {
                    tracing::warn!(
                        max = client_data.limits.max_activation_tokens,
                        "Client exceeded max activation tokens quota; rejecting token creation"
                    );
                    return false;
                }
            }
        }

        true
    }

    fn request_activation(
        &mut self,
        token: XdgActivationToken,
        token_data: XdgActivationTokenData,
        surface: WlSurface,
    ) {
        self.xdg_activation_state.remove_token(&token);
        self.decrement_token_client_quota(token_data.client_id);

        if token_data.timestamp.elapsed() > DEFAULT_ACTIVATION_EXPIRATION {
            tracing::info!("Activation token expired; ignoring activation request");
            return;
        }

        let Some(&window_id) = self.surface_to_window_id.get(&surface) else {
            tracing::debug!("Surface not recognized as managed window; ignoring activation");
            return;
        };

        crate::action::activate_and_focus_window(self, window_id);
        crate::window::sync_layout_to_space(self);
    }
}

impl KynexState {
    /// Decrements the client quota for a consumed or purged activation token.
    pub(crate) fn decrement_token_client_quota(&self, client_id: Option<ClientId>) {
        let Some(client_id) = client_id else {
            return;
        };

        if let Ok(client_data) = self
            .display_handle
            .backend_handle()
            .get_client_data(client_id)
        {
            if let Some(client_data) = client_data.downcast_ref::<ClientState>() {
                client_data.decrement_activation_token();
            }
        }
    }

    /// Purges all tokens older than [`DEFAULT_ACTIVATION_EXPIRATION`] and reclaims client quotas.
    pub(crate) fn purge_expired_tokens(&mut self) {
        let mut expired_clients = Vec::new();
        self.xdg_activation_state.retain_tokens(|_token, data| {
            if data.timestamp.elapsed() > DEFAULT_ACTIVATION_EXPIRATION {
                if let Some(ref client_id) = data.client_id {
                    expired_clients.push(client_id.clone());
                }
                false
            } else {
                true
            }
        });

        for client_id in expired_clients {
            self.decrement_token_client_quota(Some(client_id));
        }
    }
}

delegate_xdg_activation!(KynexState);

#[cfg(test)]
#[path = "activation_tests.rs"]
mod activation_tests;

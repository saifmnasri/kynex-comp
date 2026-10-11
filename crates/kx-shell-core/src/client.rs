// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Wayland client tracking and resource management.

use std::sync::atomic::{AtomicUsize, Ordering};

use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::wayland::compositor::CompositorClientState;

/// Limits for Wayland client resource allocation to mitigate denial-of-service risks.
#[derive(Debug, Clone, Copy)]
pub struct ClientLimits {
    /// Maximum number of active surfaces allowed per client.
    pub max_surfaces: usize,
    /// Maximum number of shared-memory pools allowed per client.
    pub max_shm_pools: usize,
    /// Maximum total shared-memory bytes allowed per client.
    pub max_shm_bytes: usize,
}

impl Default for ClientLimits {
    fn default() -> Self {
        Self {
            max_surfaces: 1024,
            max_shm_pools: 256,
            max_shm_bytes: 256 * 1024 * 1024, // 256 MiB
        }
    }
}

/// Per-client state tracked by the compositor.
#[derive(Debug, Default)]
pub struct ClientState {
    /// Smithay compositor client state tracking surface roles and bindings.
    pub compositor_state: CompositorClientState,
    /// Number of active surfaces owned by this client.
    pub surface_count: AtomicUsize,
    /// Configured resource limits for this client.
    pub limits: ClientLimits,
}

impl ClientState {
    /// Checks if a new surface can be created without exceeding resource limits.
    ///
    /// Returns `true` if within limits and increments the counter, or `false` if the limit is reached.
    #[must_use]
    pub fn try_increment_surface(&self) -> bool {
        let current = self.surface_count.load(Ordering::Relaxed);
        if current >= self.limits.max_surfaces {
            false
        } else {
            self.surface_count.fetch_add(1, Ordering::Relaxed);
            true
        }
    }

    /// Decrements the active surface count when a surface is destroyed.
    pub fn decrement_surface(&self) {
        let mut current = self.surface_count.load(Ordering::Relaxed);
        while current > 0 {
            match self.surface_count.compare_exchange_weak(
                current,
                current.saturating_sub(1),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }
}

impl ClientData for ClientState {
    fn initialized(&self, client_id: ClientId) {
        tracing::debug!(?client_id, "Wayland client connected and initialized");
    }

    fn disconnected(&self, client_id: ClientId, reason: DisconnectReason) {
        tracing::debug!(?client_id, ?reason, "Wayland client disconnected");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_limits_enforcement() {
        let state = ClientState {
            limits: ClientLimits {
                max_surfaces: 2,
                max_shm_pools: 2,
                max_shm_bytes: 1024,
            },
            ..Default::default()
        };

        assert!(state.try_increment_surface());
        assert_eq!(state.surface_count.load(Ordering::Relaxed), 1);
        assert!(state.try_increment_surface());
        assert_eq!(state.surface_count.load(Ordering::Relaxed), 2);
        // Exceeds limit of 2:
        assert!(!state.try_increment_surface());
        assert_eq!(state.surface_count.load(Ordering::Relaxed), 2);

        // Decrement down to 0
        state.decrement_surface();
        assert_eq!(state.surface_count.load(Ordering::Relaxed), 1);
        state.decrement_surface();
        assert_eq!(state.surface_count.load(Ordering::Relaxed), 0);
        // Saturates at 0
        state.decrement_surface();
        assert_eq!(state.surface_count.load(Ordering::Relaxed), 0);
    }
}

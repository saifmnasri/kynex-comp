// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Wayland client tracking and resource management.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};

use smithay::reexports::wayland_server::backend::{
    ClientData, ClientId, Credentials, DisconnectReason,
};
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
    /// Maximum number of active layer surfaces allowed per client.
    pub max_layer_surfaces: usize,
    /// Maximum number of concurrent activation tokens allowed per client.
    pub max_activation_tokens: usize,
}

impl Default for ClientLimits {
    fn default() -> Self {
        Self {
            max_surfaces: 1024,
            max_shm_pools: 256,
            max_shm_bytes: 256 * 1024 * 1024, // 256 MiB
            max_layer_surfaces: 8,
            max_activation_tokens: 16,
        }
    }
}

/// Per-client state tracked by the compositor.
#[derive(Debug)]
pub struct ClientState {
    /// Smithay compositor client state tracking surface roles and bindings.
    pub compositor_state: CompositorClientState,
    /// Number of active surfaces owned by this client.
    pub surface_count: AtomicUsize,
    /// Number of active layer surfaces owned by this client.
    pub layer_surface_count: AtomicUsize,
    /// Number of active activation tokens created by this client.
    pub activation_token_count: AtomicUsize,
    /// Configured resource limits for this client.
    pub limits: ClientLimits,
    /// Process ID from socket peer credentials (-1 if unset/unavailable).
    pub pid: AtomicI32,
    /// User ID from socket peer credentials (`u32::MAX` if unset/unavailable).
    pub uid: AtomicU32,
    /// Group ID from socket peer credentials (`u32::MAX` if unset/unavailable).
    pub gid: AtomicU32,
    /// Canonical executable path read from `/proc/<pid>/exe`.
    pub executable_path: Mutex<Option<PathBuf>>,
    /// Cached authorization decision for `zwlr_layer_shell_v1`.
    pub is_layer_shell_allowed: AtomicBool,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            compositor_state: CompositorClientState::default(),
            surface_count: AtomicUsize::new(0),
            layer_surface_count: AtomicUsize::new(0),
            activation_token_count: AtomicUsize::new(0),
            limits: ClientLimits::default(),
            pid: AtomicI32::new(-1),
            uid: AtomicU32::new(u32::MAX),
            gid: AtomicU32::new(u32::MAX),
            executable_path: Mutex::new(None),
            is_layer_shell_allowed: AtomicBool::new(false),
        }
    }
}

impl ClientState {
    /// Checks if a new surface can be created without exceeding resource limits.
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
        decrement_atomic_counter(&self.surface_count);
    }

    /// Checks if a new layer surface can be created without exceeding resource limits.
    #[must_use]
    pub fn try_increment_layer_surface(&self) -> bool {
        let current = self.layer_surface_count.load(Ordering::Relaxed);
        if current >= self.limits.max_layer_surfaces {
            false
        } else {
            self.layer_surface_count.fetch_add(1, Ordering::Relaxed);
            true
        }
    }

    /// Decrements the active layer surface count when a layer surface is destroyed.
    pub fn decrement_layer_surface(&self) {
        decrement_atomic_counter(&self.layer_surface_count);
    }

    /// Checks if a new activation token can be created without exceeding resource limits.
    #[must_use]
    pub fn try_increment_activation_token(&self) -> bool {
        let current = self.activation_token_count.load(Ordering::Relaxed);
        if current >= self.limits.max_activation_tokens {
            false
        } else {
            self.activation_token_count.fetch_add(1, Ordering::Relaxed);
            true
        }
    }

    /// Decrements the active activation token count when a token is consumed or expires.
    pub fn decrement_activation_token(&self) {
        decrement_atomic_counter(&self.activation_token_count);
    }

    /// Returns `true` if this client is authorized to bind `zwlr_layer_shell_v1`.
    #[must_use]
    pub fn is_layer_shell_allowed(&self) -> bool {
        self.is_layer_shell_allowed.load(Ordering::Acquire)
    }

    /// Inspects peer socket credentials and caches the authorization decision.
    pub fn inspect_and_cache_privilege(&self, creds: &Credentials, allow_list: &[String]) {
        let compositor_uid = smithay::reexports::rustix::process::getuid().as_raw();
        let (is_allowed, resolved_exe) = evaluate_privilege(creds, allow_list, compositor_uid);

        self.pid.store(creds.pid, Ordering::Release);
        self.uid.store(creds.uid, Ordering::Release);
        self.gid.store(creds.gid, Ordering::Release);
        self.is_layer_shell_allowed
            .store(is_allowed, Ordering::Release);

        if let Ok(mut exe_lock) = self.executable_path.lock() {
            *exe_lock = resolved_exe;
        }
    }
}

fn decrement_atomic_counter(counter: &AtomicUsize) {
    let mut current = counter.load(Ordering::Relaxed);
    while current > 0 {
        match counter.compare_exchange_weak(
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

fn evaluate_privilege(
    creds: &Credentials,
    allow_list: &[String],
    compositor_uid: u32,
) -> (bool, Option<PathBuf>) {
    if creds.uid != compositor_uid {
        tracing::info!(
            client_pid = creds.pid,
            client_uid = creds.uid,
            compositor_uid,
            "Layer-shell access denied: client UID mismatch"
        );
        return (false, None);
    }

    if creds.pid <= 0 {
        tracing::warn!(
            client_pid = creds.pid,
            "Invalid client PID for privilege check"
        );
        return (false, None);
    }

    let proc_exe_path = format!("/proc/{}/exe", creds.pid);
    let resolved = match std::fs::read_link(&proc_exe_path) {
        Ok(path) => path,
        Err(err) => {
            tracing::warn!(client_pid = creds.pid, %err, "Failed to resolve client /proc/<pid>/exe");
            return (false, None);
        }
    };

    let resolved_str = resolved.to_string_lossy();
    let is_allowed = allow_list
        .iter()
        .any(|allowed| allowed == resolved_str.as_ref());

    if is_allowed {
        tracing::info!(
            client_pid = creds.pid,
            client_uid = creds.uid,
            exe = %resolved_str,
            "Layer-shell privilege granted to allow-listed client"
        );
    } else {
        tracing::info!(
            client_pid = creds.pid,
            client_uid = creds.uid,
            exe = %resolved_str,
            "Layer-shell privilege denied (not in allow-list)"
        );
    }

    (is_allowed, Some(resolved))
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
                max_layer_surfaces: 2,
                max_activation_tokens: 2,
            },
            ..Default::default()
        };

        // Surface limits
        assert!(state.try_increment_surface());
        assert_eq!(state.surface_count.load(Ordering::Relaxed), 1);
        assert!(state.try_increment_surface());
        assert_eq!(state.surface_count.load(Ordering::Relaxed), 2);
        assert!(!state.try_increment_surface());
        assert_eq!(state.surface_count.load(Ordering::Relaxed), 2);

        state.decrement_surface();
        assert_eq!(state.surface_count.load(Ordering::Relaxed), 1);
        state.decrement_surface();
        assert_eq!(state.surface_count.load(Ordering::Relaxed), 0);
        state.decrement_surface();
        assert_eq!(state.surface_count.load(Ordering::Relaxed), 0);

        // Layer surface limits
        assert!(state.try_increment_layer_surface());
        assert_eq!(state.layer_surface_count.load(Ordering::Relaxed), 1);
        assert!(state.try_increment_layer_surface());
        assert_eq!(state.layer_surface_count.load(Ordering::Relaxed), 2);
        assert!(!state.try_increment_layer_surface());
        assert_eq!(state.layer_surface_count.load(Ordering::Relaxed), 2);

        state.decrement_layer_surface();
        assert_eq!(state.layer_surface_count.load(Ordering::Relaxed), 1);
        state.decrement_layer_surface();
        assert_eq!(state.layer_surface_count.load(Ordering::Relaxed), 0);
        state.decrement_layer_surface();
        assert_eq!(state.layer_surface_count.load(Ordering::Relaxed), 0);

        // Activation token limits
        assert!(state.try_increment_activation_token());
        assert_eq!(state.activation_token_count.load(Ordering::Relaxed), 1);
        assert!(state.try_increment_activation_token());
        assert_eq!(state.activation_token_count.load(Ordering::Relaxed), 2);
        assert!(!state.try_increment_activation_token());
        assert_eq!(state.activation_token_count.load(Ordering::Relaxed), 2);

        state.decrement_activation_token();
        assert_eq!(state.activation_token_count.load(Ordering::Relaxed), 1);
        state.decrement_activation_token();
        assert_eq!(state.activation_token_count.load(Ordering::Relaxed), 0);
        state.decrement_activation_token();
        assert_eq!(state.activation_token_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_privilege_evaluation() {
        let creds_mismatch = Credentials {
            pid: 1234,
            uid: 1001,
            gid: 1001,
        };
        let (allowed, exe) = evaluate_privilege(&creds_mismatch, &[], 1000);
        assert!(!allowed);
        assert!(exe.is_none());

        let creds_invalid_pid = Credentials {
            pid: -1,
            uid: 1000,
            gid: 1000,
        };
        let (allowed, exe) = evaluate_privilege(&creds_invalid_pid, &[], 1000);
        assert!(!allowed);
        assert!(exe.is_none());

        // Test with current process
        let proc_pid = i32::try_from(std::process::id()).unwrap_or(1);
        let self_uid = smithay::reexports::rustix::process::getuid().as_raw();
        let creds_current = Credentials {
            pid: proc_pid,
            uid: self_uid,
            gid: 0,
        };

        // When allow-list is empty, must be denied
        let (allowed, exe) = evaluate_privilege(&creds_current, &[], self_uid);
        assert!(!allowed);
        assert!(exe.is_some());

        // When current exe is in allow-list, must be allowed
        let current_exe = std::fs::read_link(format!("/proc/{proc_pid}/exe")).unwrap();
        let current_exe_str = current_exe.to_string_lossy().to_string();
        let (allowed, exe) = evaluate_privilege(&creds_current, &[current_exe_str], self_uid);
        assert!(allowed);
        assert_eq!(exe, Some(current_exe));
    }
}

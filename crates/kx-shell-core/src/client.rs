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
    /// Maximum number of concurrent active idle inhibitors allowed per client.
    pub max_idle_inhibitors: usize,
}

impl Default for ClientLimits {
    fn default() -> Self {
        Self {
            max_surfaces: 1024,
            max_shm_pools: 256,
            max_shm_bytes: 256 * 1024 * 1024, // 256 MiB
            max_layer_surfaces: 8,
            max_activation_tokens: 16,
            max_idle_inhibitors: 8,
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
    /// Number of active idle inhibitors owned by this client.
    pub idle_inhibitor_count: AtomicUsize,
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
    /// Cached authorization decision for `zwp_input_method_manager_v2`.
    pub is_input_method_allowed: AtomicBool,
    /// Cached authorization decision for `ext_session_lock_manager_v1`.
    pub is_session_lock_allowed: AtomicBool,
    /// Cached authorization decision for `ext_foreign_toplevel_list_v1`.
    pub is_foreign_toplevel_allowed: AtomicBool,
    /// Cached authorization decision for `ext_idle_notifier_v1`.
    pub is_idle_notify_allowed: AtomicBool,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            compositor_state: CompositorClientState::default(),
            surface_count: AtomicUsize::new(0),
            layer_surface_count: AtomicUsize::new(0),
            activation_token_count: AtomicUsize::new(0),
            idle_inhibitor_count: AtomicUsize::new(0),
            limits: ClientLimits::default(),
            pid: AtomicI32::new(-1),
            uid: AtomicU32::new(u32::MAX),
            gid: AtomicU32::new(u32::MAX),
            executable_path: Mutex::new(None),
            is_layer_shell_allowed: AtomicBool::new(false),
            is_input_method_allowed: AtomicBool::new(false),
            is_session_lock_allowed: AtomicBool::new(false),
            is_foreign_toplevel_allowed: AtomicBool::new(false),
            is_idle_notify_allowed: AtomicBool::new(false),
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

    /// Checks if a new idle inhibitor can be created without exceeding resource limits.
    #[must_use]
    pub fn try_increment_idle_inhibitor(&self) -> bool {
        let current = self.idle_inhibitor_count.load(Ordering::Relaxed);
        if current >= self.limits.max_idle_inhibitors {
            false
        } else {
            self.idle_inhibitor_count.fetch_add(1, Ordering::Relaxed);
            true
        }
    }

    /// Decrements the active idle inhibitor count when an inhibitor is destroyed.
    pub fn decrement_idle_inhibitor(&self) {
        decrement_atomic_counter(&self.idle_inhibitor_count);
    }

    /// Returns `true` if this client is authorized to bind `zwlr_layer_shell_v1`.
    #[must_use]
    pub fn is_layer_shell_allowed(&self) -> bool {
        self.is_layer_shell_allowed.load(Ordering::Acquire)
    }

    /// Returns `true` if this client is authorized to bind `zwp_input_method_manager_v2`.
    #[must_use]
    pub fn is_input_method_allowed(&self) -> bool {
        self.is_input_method_allowed.load(Ordering::Acquire)
    }

    /// Sets authorization for `zwp_input_method_manager_v2`.
    pub fn set_input_method_allowed(&self, allowed: bool) {
        self.is_input_method_allowed
            .store(allowed, Ordering::Release);
    }

    /// Returns `true` if this client is authorized to bind `ext_session_lock_manager_v1`.
    #[must_use]
    pub fn is_session_lock_allowed(&self) -> bool {
        self.is_session_lock_allowed.load(Ordering::Acquire)
    }

    /// Sets authorization for `ext_session_lock_manager_v1`.
    pub fn set_session_lock_allowed(&self, allowed: bool) {
        self.is_session_lock_allowed
            .store(allowed, Ordering::Release);
    }

    /// Returns `true` if this client is authorized to bind `ext_foreign_toplevel_list_v1`.
    #[must_use]
    pub fn is_foreign_toplevel_allowed(&self) -> bool {
        self.is_foreign_toplevel_allowed.load(Ordering::Acquire)
    }

    /// Sets authorization for `ext_foreign_toplevel_list_v1`.
    pub fn set_foreign_toplevel_allowed(&self, allowed: bool) {
        self.is_foreign_toplevel_allowed
            .store(allowed, Ordering::Release);
    }

    /// Returns `true` if this client is authorized to bind `ext_idle_notifier_v1`.
    #[must_use]
    pub fn is_idle_notify_allowed(&self) -> bool {
        self.is_idle_notify_allowed.load(Ordering::Acquire)
    }

    /// Sets authorization for `ext_idle_notifier_v1`.
    pub fn set_idle_notify_allowed(&self, allowed: bool) {
        self.is_idle_notify_allowed
            .store(allowed, Ordering::Release);
    }

    /// Inspects peer socket credentials and caches the authorization decisions independently.
    pub fn inspect_and_cache_privilege(
        &self,
        creds: &Credentials,
        config: &kx_config::PrivilegeConfig,
    ) {
        let compositor_uid = smithay::reexports::rustix::process::getuid().as_raw();
        let (decisions, resolved_exe) = evaluate_all_privileges(creds, config, compositor_uid);

        self.pid.store(creds.pid, Ordering::Release);
        self.uid.store(creds.uid, Ordering::Release);
        self.gid.store(creds.gid, Ordering::Release);
        self.is_layer_shell_allowed.store(
            decisions.contains(ClientPrivileges::LAYER_SHELL),
            Ordering::Release,
        );
        self.is_foreign_toplevel_allowed.store(
            decisions.contains(ClientPrivileges::FOREIGN_TOPLEVEL),
            Ordering::Release,
        );
        self.is_idle_notify_allowed.store(
            decisions.contains(ClientPrivileges::IDLE_NOTIFY),
            Ordering::Release,
        );
        self.is_session_lock_allowed.store(
            decisions.contains(ClientPrivileges::SESSION_LOCK),
            Ordering::Release,
        );
        self.is_input_method_allowed.store(
            decisions.contains(ClientPrivileges::INPUT_METHOD),
            Ordering::Release,
        );

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

bitflags::bitflags! {
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    struct ClientPrivileges: u8 {
        const LAYER_SHELL = 1 << 0;
        const FOREIGN_TOPLEVEL = 1 << 1;
        const IDLE_NOTIFY = 1 << 2;
        const SESSION_LOCK = 1 << 3;
        const INPUT_METHOD = 1 << 4;
    }
}

fn is_secure_privileged_binary(path: &std::path::Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match std::fs::metadata(path) {
        Ok(meta) => meta.uid() == 0 && (meta.mode() & 0o022) == 0,
        Err(err) => {
            tracing::warn!(path = %path.display(), %err, "Failed to inspect metadata of privileged binary");
            false
        }
    }
}

fn evaluate_all_privileges(
    creds: &Credentials,
    config: &kx_config::PrivilegeConfig,
    compositor_uid: u32,
) -> (ClientPrivileges, Option<PathBuf>) {
    if creds.uid != compositor_uid {
        tracing::info!(
            client_pid = creds.pid,
            client_uid = creds.uid,
            compositor_uid,
            "Access denied: client UID mismatch"
        );
        return (ClientPrivileges::empty(), None);
    }

    if creds.pid <= 0 {
        tracing::warn!(
            client_pid = creds.pid,
            "Invalid client PID for privilege check"
        );
        return (ClientPrivileges::empty(), None);
    }

    let proc_exe_path = format!("/proc/{}/exe", creds.pid);
    let resolved = match std::fs::read_link(&proc_exe_path) {
        Ok(path) => path,
        Err(err) => {
            tracing::warn!(client_pid = creds.pid, %err, "Failed to resolve client /proc/<pid>/exe");
            return (ClientPrivileges::empty(), None);
        }
    };

    let resolved_str = resolved.to_string_lossy();
    let is_in_list =
        |list: &[String]| -> bool { list.iter().any(|allowed| allowed == resolved_str.as_ref()) };

    let is_secure_binary = is_secure_privileged_binary(&resolved);
    let session_lock_in_list = is_in_list(&config.session_lock_allow);
    let session_lock = session_lock_in_list && is_secure_binary;
    if session_lock_in_list && !session_lock {
        tracing::warn!(
            client_pid = creds.pid,
            exe = %resolved_str,
            "Session-lock access denied: binary not root-owned or group/world writable"
        );
    }

    let input_method_in_list = is_in_list(&config.input_method_allow);
    let input_method = input_method_in_list && is_secure_binary;
    if input_method_in_list && !input_method {
        tracing::warn!(
            client_pid = creds.pid,
            exe = %resolved_str,
            "Input-method access denied: binary not root-owned or group/world writable"
        );
    }

    let mut privileges = ClientPrivileges::empty();
    if is_in_list(&config.layer_shell_allow) {
        privileges |= ClientPrivileges::LAYER_SHELL;
    }
    if is_in_list(&config.foreign_toplevel_allow) {
        privileges |= ClientPrivileges::FOREIGN_TOPLEVEL;
    }
    if is_in_list(&config.idle_notify_allow) {
        privileges |= ClientPrivileges::IDLE_NOTIFY;
    }
    if session_lock {
        privileges |= ClientPrivileges::SESSION_LOCK;
    }
    if input_method {
        privileges |= ClientPrivileges::INPUT_METHOD;
    }

    tracing::info!(
        client_pid = creds.pid,
        exe = %resolved_str,
        ?privileges,
        "Evaluated client privileges"
    );

    (privileges, Some(resolved))
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
                max_idle_inhibitors: 2,
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
        let priv_empty = kx_config::PrivilegeConfig::default();
        let (decisions, exe) = evaluate_all_privileges(&creds_mismatch, &priv_empty, 1000);
        assert!(!decisions.contains(ClientPrivileges::LAYER_SHELL));
        assert!(exe.is_none());

        let creds_invalid_pid = Credentials {
            pid: -1,
            uid: 1000,
            gid: 1000,
        };
        let (decisions, exe) = evaluate_all_privileges(&creds_invalid_pid, &priv_empty, 1000);
        assert!(!decisions.contains(ClientPrivileges::LAYER_SHELL));
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
        let (decisions, exe) = evaluate_all_privileges(&creds_current, &priv_empty, self_uid);
        assert!(!decisions.contains(ClientPrivileges::LAYER_SHELL));
        assert!(exe.is_some());

        // When current exe is in allow-list, must be allowed
        let current_exe = std::fs::read_link(format!("/proc/{proc_pid}/exe")).unwrap();
        let current_exe_str = current_exe.to_string_lossy().to_string();
        let priv_layer_shell = kx_config::PrivilegeConfig {
            layer_shell_allow: vec![current_exe_str.clone()],
            ..Default::default()
        };
        let (decisions, exe) = evaluate_all_privileges(&creds_current, &priv_layer_shell, self_uid);
        assert!(decisions.contains(ClientPrivileges::LAYER_SHELL));
        assert!(!decisions.contains(ClientPrivileges::FOREIGN_TOPLEVEL));
        assert_eq!(exe, Some(current_exe.clone()));

        // Test inspect_and_cache_privilege updates foreign toplevel allowed
        let state = ClientState::default();
        assert!(!state.is_foreign_toplevel_allowed());
        let priv_config = kx_config::PrivilegeConfig {
            foreign_toplevel_allow: vec![current_exe_str.clone()],
            ..Default::default()
        };
        state.inspect_and_cache_privilege(&creds_current, &priv_config);
        assert!(state.is_foreign_toplevel_allowed());
        assert!(!state.is_layer_shell_allowed());
        assert!(!state.is_idle_notify_allowed());
        assert!(!state.is_session_lock_allowed());
        assert!(!state.is_input_method_allowed());
        state.set_foreign_toplevel_allowed(false);
        assert!(!state.is_foreign_toplevel_allowed());
    }

    #[test]
    fn test_idle_inhibitor_limits() {
        let state = ClientState {
            limits: ClientLimits {
                max_idle_inhibitors: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(state.try_increment_idle_inhibitor());
        assert!(state.try_increment_idle_inhibitor());
        assert!(!state.try_increment_idle_inhibitor());
        state.decrement_idle_inhibitor();
        assert!(state.try_increment_idle_inhibitor());
    }

    #[test]
    fn test_privileged_executable_rejects_non_root() {
        let proc_pid = i32::try_from(std::process::id()).unwrap_or(1);
        let self_uid = smithay::reexports::rustix::process::getuid().as_raw();
        let creds_current = Credentials {
            pid: proc_pid,
            uid: self_uid,
            gid: 0,
        };
        let current_exe = std::fs::read_link(format!("/proc/{proc_pid}/exe")).unwrap();
        let current_exe_str = current_exe.to_string_lossy().to_string();

        let state = ClientState::default();
        let priv_config = kx_config::PrivilegeConfig {
            session_lock_allow: vec![current_exe_str.clone()],
            input_method_allow: vec![current_exe_str],
            ..Default::default()
        };
        state.inspect_and_cache_privilege(&creds_current, &priv_config);

        let is_root = std::fs::metadata(&current_exe).is_ok_and(|m| {
            use std::os::unix::fs::MetadataExt;
            m.uid() == 0
        });

        if !is_root {
            assert!(!state.is_session_lock_allowed());
            assert!(!state.is_input_method_allowed());
        }
    }
}

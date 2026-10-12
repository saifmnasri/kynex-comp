// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Systemd readiness and watchdog notification support (`sd_notify`).
//!
//! Communicates with the systemd service manager over the Unix datagram socket
//! specified in `$NOTIFY_SOCKET` without requiring external C libraries or extra dependencies.

use std::env;
use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixDatagram;
use std::path::Path;

/// Errors that can occur during systemd notification.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SystemdNotifyError {
    /// Failed to create an unbound Unix datagram socket.
    #[error("failed to create unbound Unix datagram socket: {0}")]
    SocketCreate(io::Error),

    /// Failed to send notification datagram to socket at `{path}`.
    #[error("failed to send systemd notification datagram to '{path}': {source}")]
    SocketSend {
        /// Target socket path or identifier.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
}

/// Standard systemd notification states.
pub mod notify_state {
    /// Service startup finished and is ready to process requests.
    pub const READY: &str = "READY=1";
    /// Periodic watchdog keepalive ping.
    pub const WATCHDOG: &str = "WATCHDOG=1";
    /// Service is initiating clean shutdown.
    pub const STOPPING: &str = "STOPPING=1";
}

/// Sends a notification datagram to an explicit socket destination.
///
/// # Errors
/// Returns [`SystemdNotifyError`] if creating the datagram socket or sending the notification fails.
pub fn sd_notify_to(socket_dest: Option<&str>, state: &str) -> Result<bool, SystemdNotifyError> {
    let socket_str = match socket_dest {
        Some(val) if !val.is_empty() => val,
        _ => return Ok(false),
    };

    let socket = UnixDatagram::unbound().map_err(SystemdNotifyError::SocketCreate)?;

    let path_storage;
    let path = if let Some(stripped) = socket_str.strip_prefix('@') {
        let mut bytes = Vec::with_capacity(stripped.len() + 1);
        bytes.push(0);
        bytes.extend_from_slice(stripped.as_bytes());
        path_storage = bytes;
        Path::new(OsStr::from_bytes(&path_storage))
    } else {
        Path::new(socket_str)
    };

    socket
        .send_to(state.as_bytes(), path)
        .map_err(|source| SystemdNotifyError::SocketSend {
            path: socket_str.to_string(),
            source,
        })?;

    Ok(true)
}

/// Sends a notification state string to systemd via `$NOTIFY_SOCKET`.
///
/// If `$NOTIFY_SOCKET` is not set or empty, this function immediately returns
/// `Ok(false)` without error (indicating that the process is not managed by systemd).
///
/// If `$NOTIFY_SOCKET` is set:
/// - Handles regular file paths.
/// - Handles Linux abstract domain sockets (paths prefixed with `@`, where the
///   first byte is converted to `\0`).
///
/// # Returns
/// - `Ok(true)` if the notification message was successfully sent to the socket.
/// - `Ok(false)` if `$NOTIFY_SOCKET` was not set or empty.
///
/// # Errors
/// Returns [`SystemdNotifyError`] if creating the socket or sending the datagram fails.
pub fn sd_notify(state: &str) -> Result<bool, SystemdNotifyError> {
    let notify_socket = env::var("NOTIFY_SOCKET").ok();
    sd_notify_to(notify_socket.as_deref(), state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_sd_notify_to_none_or_empty_returns_ok_false() {
        assert!(!sd_notify_to(None, notify_state::READY).unwrap());
        assert!(!sd_notify_to(Some(""), notify_state::WATCHDOG).unwrap());
    }

    #[test]
    fn test_sd_notify_to_nonexistent_socket_returns_error() {
        let err = sd_notify_to(
            Some("/tmp/kynex_nonexistent_socket_test.sock"),
            notify_state::READY,
        )
        .unwrap_err();

        assert!(matches!(err, SystemdNotifyError::SocketSend { .. }));
    }

    #[test]
    fn test_sd_notify_to_file_socket_success() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("notify.sock");
        let server = UnixDatagram::bind(&socket_path).unwrap();

        let socket_path_str = socket_path.to_str().unwrap();
        let notified = sd_notify_to(Some(socket_path_str), notify_state::READY).unwrap();
        assert!(notified);

        let mut buf = [0u8; 64];
        let (bytes_read, _) = server.recv_from(&mut buf).unwrap();
        let received = std::str::from_utf8(&buf[..bytes_read]).unwrap();
        assert_eq!(received, "READY=1");
    }

    #[test]
    fn test_sd_notify_to_watchdog_and_stopping() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("watchdog.sock");
        let server = UnixDatagram::bind(&socket_path).unwrap();

        let socket_path_str = socket_path.to_str().unwrap();

        assert!(sd_notify_to(Some(socket_path_str), notify_state::WATCHDOG).unwrap());
        let mut buf = [0u8; 64];
        let (n1, _) = server.recv_from(&mut buf).unwrap();
        assert_eq!(std::str::from_utf8(&buf[..n1]).unwrap(), "WATCHDOG=1");

        assert!(sd_notify_to(Some(socket_path_str), notify_state::STOPPING).unwrap());
        let (n2, _) = server.recv_from(&mut buf).unwrap();
        assert_eq!(std::str::from_utf8(&buf[..n2]).unwrap(), "STOPPING=1");
    }
}

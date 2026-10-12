// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Session management integration via libseat for the DRM backend.

use std::os::unix::io::OwnedFd;
use std::path::Path;

use smithay::backend::session::Session;
use smithay::backend::session::libseat::{LibSeatSession, LibSeatSessionNotifier};
use smithay::reexports::rustix::fs::OFlags;
use tracing::info;

use crate::error::ShellError;

/// Active compositor session managing seat devices and virtual terminals.
#[derive(Debug)]
pub struct DrmSession {
    session: LibSeatSession,
}

impl DrmSession {
    /// Open a new session via `libseat`.
    ///
    /// # Errors
    ///
    /// Returns `ShellError::Session` if session opening fails (e.g. no seatd/logind).
    pub fn open() -> Result<(Self, LibSeatSessionNotifier), ShellError> {
        let (session, notifier) = LibSeatSession::new().map_err(|err| {
            ShellError::Session(format!("failed to initialize libseat session: {err}"))
        })?;
        let seat = session.seat();
        info!(
            seat = %seat,
            active = session.is_active(),
            "DRM session opened via libseat"
        );
        Ok((Self { session }, notifier))
    }

    /// Retrieve the seat name.
    #[must_use]
    pub fn seat(&self) -> String {
        self.session.seat()
    }

    /// Check if the session is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.session.is_active()
    }

    /// Retrieve a clone of the underlying `LibSeatSession` handle.
    #[must_use]
    pub fn libseat_session(&self) -> LibSeatSession {
        self.session.clone()
    }

    /// Open a device node managed by the session.
    ///
    /// # Errors
    ///
    /// Returns `ShellError::Session` if libseat fails to open the device.
    pub fn open_device(&mut self, path: &Path, flags: OFlags) -> Result<OwnedFd, ShellError> {
        self.session.open(path, flags).map_err(|err| {
            ShellError::Session(format!(
                "failed to open device node {}: {err}",
                path.display()
            ))
        })
    }

    /// Close a previously opened device fd.
    ///
    /// # Errors
    ///
    /// Returns `ShellError::Session` if libseat fails to close the device fd.
    pub fn close_device(&mut self, fd: OwnedFd) -> Result<(), ShellError> {
        self.session
            .close(fd)
            .map_err(|err| ShellError::Session(format!("failed to close device fd: {err}")))
    }

    /// Switch to a virtual terminal (VT).
    ///
    /// # Errors
    ///
    /// Returns `ShellError::Session` if VT switch fails.
    pub fn change_vt(&mut self, vt: i32) -> Result<(), ShellError> {
        self.session
            .change_vt(vt)
            .map_err(|err| ShellError::Session(format!("failed to switch to VT {vt}: {err}")))
    }
}

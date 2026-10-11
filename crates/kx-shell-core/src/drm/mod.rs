// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! DRM/KMS hardware backend for `KynexComp`.

pub mod device;
pub mod session;

use std::time::Duration;

pub use device::{ConnectorInfo, DrmDeviceInfo, DrmDiagnosticReport, ModeInfo};
pub use session::DrmSession;
use tracing::info;

use crate::error::ShellError;

/// Options for configuring DRM backend execution.
#[derive(Debug, Clone, Default)]
pub struct DrmOptions {
    /// Optional exit timer duration.
    pub exit_after: Option<Duration>,
    /// When true, only query and log hardware info, then exit immediately.
    pub diagnostic_only: bool,
}

/// Entry point to initialize and run the DRM backend.
///
/// # Errors
///
/// Returns `ShellError` if session or DRM device initialization fails.
pub fn run_drm(options: &DrmOptions) -> Result<(), ShellError> {
    info!("Initializing DRM backend...");

    let (mut session, _notifier) = DrmSession::open()?;

    let report = device::inspect_drm(&mut session)?;
    report.log_report();

    if options.diagnostic_only {
        info!("Diagnostic scan complete, exiting as requested by --diagnostic");
        return Ok(());
    }

    // In Milestone 2D+, full event loop, DRM device initialization and renderer setup will follow.
    info!("DRM full modesetting backend will be initialized in Milestone 2D.");
    Ok(())
}

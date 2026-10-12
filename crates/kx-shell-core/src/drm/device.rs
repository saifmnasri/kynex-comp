// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Read-only DRM device, connector, CRTC, and plane inspection and reporting.

use std::os::unix::io::{AsFd, BorrowedFd, OwnedFd};
use std::path::{Path, PathBuf};

use smithay::backend::udev::{all_gpus, primary_gpu};
use smithay::reexports::drm::control::{
    Device as ControlDevice, ModeTypeFlags, RawResourceHandle, connector,
};
use smithay::reexports::rustix::fs::OFlags;
use tracing::{info, warn};

use super::session::DrmSession;
use crate::error::ShellError;

/// Detailed diagnostic report of detected DRM devices and KMS resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrmDiagnosticReport {
    /// Seat name on which devices were enumerated.
    pub seat: String,
    /// Primary GPU device node path, if identified by udev.
    pub primary_gpu: Option<PathBuf>,
    /// Enumerated DRM devices with their capabilities.
    pub devices: Vec<DrmDeviceInfo>,
}

/// Information about a single DRM device node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrmDeviceInfo {
    /// Device node path (e.g. `/dev/dri/card0`).
    pub path: PathBuf,
    /// Whether this device was tagged as the primary GPU.
    pub is_primary: bool,
    /// Connectors discovered on this device.
    pub connectors: Vec<ConnectorInfo>,
    /// Number of hardware CRTCs available.
    pub crtc_count: usize,
    /// Number of hardware display planes available.
    pub plane_count: usize,
}

/// Information about a hardware display connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorInfo {
    /// Numeric connector ID assigned by DRM.
    pub id: u32,
    /// Interface type name (e.g. `Virtual`, `HDMIA`, `DisplayPort`).
    pub interface: String,
    /// Whether a display monitor is currently connected.
    pub is_connected: bool,
    /// List of supported display video modes.
    pub modes: Vec<ModeInfo>,
}

/// Video mode parameters for a display connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeInfo {
    /// Horizontal pixel resolution.
    pub width: u16,
    /// Vertical pixel resolution.
    pub height: u16,
    /// Vertical refresh rate in Hz.
    pub refresh_rate: u32,
    /// Whether this mode is marked as the preferred display mode.
    pub is_preferred: bool,
}

impl DrmDiagnosticReport {
    /// Emit structured log messages detailing the detected DRM devices and KMS resources.
    pub fn log_report(&self) {
        info!(
            seat = %self.seat,
            devices_count = self.devices.len(),
            primary_gpu = ?self.primary_gpu,
            "=== DRM KMS Diagnostic Report ==="
        );

        for dev in &self.devices {
            dev.log_device();
        }

        info!("=== End DRM KMS Diagnostic Report ===");
    }
}

impl DrmDeviceInfo {
    /// Emit structured log messages for this DRM device and its connectors.
    pub fn log_device(&self) {
        info!(
            path = %self.path.display(),
            is_primary = self.is_primary,
            crtcs = self.crtc_count,
            planes = self.plane_count,
            connectors = self.connectors.len(),
            "DRM Device Node"
        );

        for conn in &self.connectors {
            conn.log_connector();
        }
    }
}

impl ConnectorInfo {
    /// Emit structured log messages for this display connector and its modes.
    pub fn log_connector(&self) {
        info!(
            connector_id = self.id,
            interface = %self.interface,
            connected = self.is_connected,
            modes_count = self.modes.len(),
            "  Connector"
        );

        for mode in &self.modes {
            info!(
                connector_id = self.id,
                width = mode.width,
                height = mode.height,
                refresh_rate = mode.refresh_rate,
                preferred = mode.is_preferred,
                "    Mode"
            );
        }
    }
}

/// Inspect all DRM devices available to the session and produce a diagnostic report.
///
/// # Errors
///
/// Returns `ShellError::Udev` if device discovery via udev fails.
pub fn inspect_drm(session: &mut DrmSession) -> Result<DrmDiagnosticReport, ShellError> {
    let seat = session.seat();
    let primary = primary_gpu(&seat)
        .map_err(|e| ShellError::Udev(format!("primary_gpu query failed: {e}")))?;
    let mut gpus =
        all_gpus(&seat).map_err(|e| ShellError::Udev(format!("all_gpus query failed: {e}")))?;

    if let Some(ref prim) = primary {
        if !gpus.contains(prim) {
            gpus.insert(0, prim.clone());
        }
    }

    if gpus.is_empty() {
        warn!(seat = %seat, "No DRM GPU devices found via udev");
    }

    let mut devices = Vec::new();
    for path in &gpus {
        let is_primary = primary.as_ref() == Some(path);
        match inspect_single_device(session, path, is_primary) {
            Ok(info) => devices.push(info),
            Err(err) => {
                warn!(
                    path = %path.display(),
                    error = %err,
                    "Failed to inspect DRM device, skipping"
                );
            }
        }
    }

    Ok(DrmDiagnosticReport {
        seat,
        primary_gpu: primary,
        devices,
    })
}

struct InspectDrmFd<'a>(&'a OwnedFd);

impl AsFd for InspectDrmFd<'_> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl smithay::reexports::drm::Device for InspectDrmFd<'_> {}
impl ControlDevice for InspectDrmFd<'_> {}

fn inspect_single_device(
    session: &mut DrmSession,
    path: &Path,
    is_primary: bool,
) -> Result<DrmDeviceInfo, ShellError> {
    let fd = session.open_device(path, OFlags::RDWR | OFlags::CLOEXEC)?;
    let inspect_dev = InspectDrmFd(&fd);

    let res = inspect_dev
        .resource_handles()
        .map_err(|e| ShellError::Drm(format!("failed to query DRM resource handles: {e}")));

    let planes = inspect_dev
        .plane_handles()
        .map_err(|e| ShellError::Drm(format!("failed to query DRM plane handles: {e}")));

    let connectors = match &res {
        Ok(r) => inspect_connectors(&inspect_dev, r.connectors()),
        Err(_) => Vec::new(),
    };

    // Always release the device from the session before returning or propagating errors.
    if let Err(close_err) = session.close_device(fd) {
        warn!(
            path = %path.display(),
            error = %close_err,
            "Failed to close inspected DRM device via session"
        );
    }

    let res = res?;
    let planes = planes?;

    let crtc_count = res.crtcs().len();
    let plane_count = planes.len();

    Ok(DrmDeviceInfo {
        path: path.to_path_buf(),
        is_primary,
        connectors,
        crtc_count,
        plane_count,
    })
}

fn inspect_connectors(
    drm_fd: &impl ControlDevice,
    handles: &[connector::Handle],
) -> Vec<ConnectorInfo> {
    let mut connectors = Vec::with_capacity(handles.len());

    for handle in handles {
        let Ok(info) = drm_fd.get_connector(*handle, false) else {
            continue;
        };

        let raw_id = Into::<RawResourceHandle>::into(info.handle()).get();
        let interface = format!("{:?}", info.interface());
        let is_connected = info.state() == connector::State::Connected;

        let modes = info
            .modes()
            .iter()
            .map(|m| {
                let (width, height) = m.size();
                ModeInfo {
                    width,
                    height,
                    refresh_rate: m.vrefresh(),
                    is_preferred: m.mode_type().contains(ModeTypeFlags::PREFERRED),
                }
            })
            .collect();

        connectors.push(ConnectorInfo {
            id: raw_id,
            interface,
            is_connected,
            modes,
        });
    }

    connectors
}

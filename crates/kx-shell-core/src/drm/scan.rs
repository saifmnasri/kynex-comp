// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! DRM connector discovery and physical output abstraction creation.

use smithay::backend::drm::DrmDeviceFd;
use smithay::output::{Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::drm::control::{
    Device, Mode, ModeTypeFlags, RawResourceHandle, ResourceHandles, connector, crtc,
};
use smithay::utils::{Logical, Point, Transform};

use crate::drm::layout::ConnectorCandidate;
use crate::error::ShellError;

/// Detected display connector with parsed modes and layout candidate.
#[derive(Debug, Clone)]
pub struct ScannedConnector {
    /// Connected connector hardware handle.
    pub handle: connector::Handle,
    /// Connected connector hardware info.
    pub info: connector::Info,
    /// Preferred or highest-resolution video mode.
    pub mode: Mode,
    /// Candidate representation for CRTC allocation.
    pub candidate: ConnectorCandidate<connector::Handle>,
}

/// Discovers connected DRM connectors and builds candidate mode configurations.
pub(crate) fn scan_connected_connectors(
    drm_fd: &DrmDeviceFd,
) -> Result<Vec<ScannedConnector>, ShellError> {
    let res = drm_fd
        .resource_handles()
        .map_err(|e| ShellError::Drm(format!("failed to query DRM resource handles: {e}")))?;

    let available_crtcs = res.crtcs().to_vec();
    let mut detected = Vec::new();

    for handle in res.connectors() {
        if let Some(scanned) = scan_single_connector(drm_fd, *handle, &res, &available_crtcs)? {
            detected.push(scanned);
        }
    }

    Ok(detected)
}

fn scan_single_connector(
    drm_fd: &DrmDeviceFd,
    handle: connector::Handle,
    res: &ResourceHandles,
    available_crtcs: &[crtc::Handle],
) -> Result<Option<ScannedConnector>, ShellError> {
    let Ok(info) = drm_fd.get_connector(handle, false) else {
        return Ok(None);
    };
    if info.state() != connector::State::Connected || info.modes().is_empty() {
        return Ok(None);
    }

    let preferred_mode = info
        .modes()
        .iter()
        .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
        .or_else(|| info.modes().first())
        .copied()
        .ok_or_else(|| {
            ShellError::Output("connected connector has no available modes".to_string())
        })?;

    let (width, height) = preferred_mode.size();
    let size = smithay::utils::Size::from((i32::from(width), i32::from(height)));
    let possible_crtcs = query_possible_crtcs(drm_fd, &info, res, available_crtcs);

    let raw_id: u32 = Into::<RawResourceHandle>::into(handle).get();
    let name = format!("{:?}-{raw_id}", info.interface());

    let candidate = ConnectorCandidate {
        handle,
        name,
        size,
        possible_crtcs,
    };

    Ok(Some(ScannedConnector {
        handle,
        info,
        mode: preferred_mode,
        candidate,
    }))
}

fn query_possible_crtcs(
    drm_fd: &DrmDeviceFd,
    info: &connector::Info,
    res: &ResourceHandles,
    available_crtcs: &[crtc::Handle],
) -> Vec<u32> {
    let mut possible_crtcs = Vec::new();
    for enc_handle in info.encoders() {
        if let Ok(enc_info) = drm_fd.get_encoder(*enc_handle) {
            let matching_crtcs = res.filter_crtcs(enc_info.possible_crtcs());
            for (idx, crtc) in available_crtcs.iter().enumerate() {
                if matching_crtcs.contains(crtc) {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "CRTC count on DRM device is small"
                    )]
                    possible_crtcs.push(idx as u32);
                }
            }
        }
    }
    possible_crtcs
}

pub(crate) fn create_kynex_output(
    connector_handle: connector::Handle,
    connector_info: &connector::Info,
    mode: &Mode,
    position: Point<i32, Logical>,
) -> Output {
    let raw_id: u32 = Into::<RawResourceHandle>::into(connector_handle).get();
    let output = Output::new(
        format!("DRM-{raw_id}"),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Kynex".to_string(),
            model: format!("{:?}", connector_info.interface()),
        },
    );

    let (width, height) = mode.size();
    let mode_info = smithay::output::Mode {
        size: (i32::from(width), i32::from(height)).into(),
        refresh: i32::try_from(mode.vrefresh())
            .unwrap_or(60)
            .saturating_mul(1000),
    };
    output.set_preferred(mode_info);
    output.change_current_state(
        Some(mode_info),
        Some(Transform::Normal),
        Some(Scale::Integer(1)),
        Some(position),
    );

    output
}

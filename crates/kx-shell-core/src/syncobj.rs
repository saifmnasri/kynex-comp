// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Linux DRM Syncobj explicit synchronization protocol handler (`wp_linux_drm_syncobj_manager_v1`).
//!
//! Provides explicit timeline point synchronization between client rendering and compositor
//! presentation, eliminating implicit GPU synchronization stalls.

use std::path::Path;

use smithay::backend::drm::DrmDeviceFd;
use smithay::delegate_drm_syncobj;
use smithay::reexports::rustix::fs::{Mode, OFlags, open};
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::DeviceFd;
use smithay::wayland::drm_syncobj::{DrmSyncobjHandler, DrmSyncobjState, supports_syncobj_eventfd};

use crate::state::KynexState;

/// Initializes the `wp_linux_drm_syncobj_manager_v1` global if a valid DRM device
/// supporting syncobj eventfds is provided or discovered.
///
/// If `device` is `Some`, tests that device for syncobj eventfd support.
/// If `device` is `None`, probes standard DRM render nodes (e.g. `/dev/dri/renderD128`)
/// for nested and headless environments.
#[must_use]
pub fn init_drm_syncobj_global(
    dh: &DisplayHandle,
    device: Option<DrmDeviceFd>,
) -> Option<DrmSyncobjState> {
    if let Some(dev) = device {
        return create_syncobj_state(dh, dev);
    }

    // Attempt discovery of render node for nested/headless backend
    probe_render_node(dh)
}

/// Probes `/dev/dri/renderD128` to support explicit sync in nested/headless sessions.
fn probe_render_node(dh: &DisplayHandle) -> Option<DrmSyncobjState> {
    let render_node_path = Path::new("/dev/dri/renderD128");
    let fd = open(
        render_node_path,
        OFlags::RDWR | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .ok()?;
    let dev_fd = DeviceFd::from(fd);
    let drm_device = DrmDeviceFd::new(dev_fd);
    create_syncobj_state(dh, drm_device)
}

/// Creates a `DrmSyncobjState` if the DRM device supports syncobj eventfds.
fn create_syncobj_state(dh: &DisplayHandle, device: DrmDeviceFd) -> Option<DrmSyncobjState> {
    if supports_syncobj_eventfd(&device) {
        tracing::info!("Initializing wp_linux_drm_syncobj_manager_v1 global");
        Some(DrmSyncobjState::new::<KynexState>(dh, device))
    } else {
        tracing::warn!("DRM device does not support syncobj eventfds; explicit sync disabled");
        None
    }
}

impl DrmSyncobjHandler for KynexState {
    fn drm_syncobj_state(&mut self) -> Option<&mut DrmSyncobjState> {
        self.drm_syncobj_state.as_mut()
    }
}

delegate_drm_syncobj!(KynexState);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syncobj_probe_render_node_does_not_panic() {
        let display = smithay::reexports::wayland_server::Display::<KynexState>::new()
            .expect("failed to create test wayland display");
        let dh = display.handle();
        // Probing should either succeed or return None depending on environment DRM support,
        // but must never panic.
        let _ = probe_render_node(&dh);
    }
}

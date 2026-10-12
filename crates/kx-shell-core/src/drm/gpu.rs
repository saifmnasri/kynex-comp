// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! DRM GPU hardware initialization and allocation resources.

use std::path::Path;

use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::drm::exporter::gbm::GbmFramebufferExporter;
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, DrmDeviceNotifier};
use smithay::backend::egl::EGLDisplay;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::reexports::rustix::fs::OFlags;
use smithay::utils::DeviceFd;
use tracing::info;

use crate::drm::egl::create_drm_renderer;
use crate::drm::session::DrmSession;
use crate::error::ShellError;

/// Bundle of initialized DRM, GBM, and EGL resources for a single GPU.
pub struct DrmGpuResources {
    /// Active DRM KMS device interface (taken by output manager upon pipeline creation).
    pub drm_device: Option<DrmDevice>,
    /// Calloop event notifier for page-flip / vblank events.
    pub drm_notifier: Option<DrmDeviceNotifier>,
    /// GBM allocator device.
    pub gbm: GbmDevice<DrmDeviceFd>,
    /// GBM buffer allocator for primary scanout swapchains.
    pub allocator: GbmAllocator<DrmDeviceFd>,
    /// Framebuffer exporter for KMS scanout buffers.
    pub exporter: GbmFramebufferExporter<DrmDeviceFd>,
    /// EGL display associated with this GBM device.
    pub egl_display: EGLDisplay,
    /// OpenGL ES hardware renderer.
    pub renderer: GlesRenderer,
    /// Open DRM device file descriptor.
    pub drm_fd: DrmDeviceFd,
}

impl DrmGpuResources {
    /// Initialize GPU resources from a given DRM card path within an active session.
    ///
    /// # Errors
    ///
    /// Returns [`ShellError`] if device opening, DRM initialization, GBM creation,
    /// or EGL/GLES setup fails.
    pub fn init(session: &mut DrmSession, card_path: &Path) -> Result<Self, ShellError> {
        info!(card = %card_path.display(), "Opening DRM GPU device");
        let fd = session.open_device(card_path, OFlags::RDWR | OFlags::CLOEXEC)?;
        let dev_fd = DeviceFd::from(fd);
        let drm_fd = DrmDeviceFd::new(dev_fd);

        let (drm_device, drm_notifier) = DrmDevice::new(drm_fd.clone(), false)?;

        let gbm = GbmDevice::new(drm_fd.clone())
            .map_err(|e| ShellError::Drm(format!("failed to initialize GBM device: {e}")))?;

        let allocator = GbmAllocator::new(
            gbm.clone(),
            GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
        );
        let exporter = GbmFramebufferExporter::new(gbm.clone(), None);

        let (egl_display, renderer) = create_drm_renderer(&gbm)?;

        info!(
            card = %card_path.display(),
            "DRM GPU, GBM, and EGL GLES renderer initialized successfully"
        );

        Ok(Self {
            drm_device: Some(drm_device),
            drm_notifier: Some(drm_notifier),
            gbm,
            allocator,
            exporter,
            egl_display,
            renderer,
            drm_fd,
        })
    }
}

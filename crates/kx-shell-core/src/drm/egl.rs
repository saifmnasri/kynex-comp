// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Isolated EGL and OpenGL ES renderer instantiation for DRM/GBM.

#![expect(
    unsafe_code,
    reason = "EGLDisplay and GlesRenderer initialization wrap libEGL driver FFI"
)]

use std::os::unix::io::AsFd;

use smithay::backend::allocator::gbm::GbmDevice;
use smithay::backend::egl::{EGLContext, EGLDisplay};
use smithay::backend::renderer::gles::GlesRenderer;

use crate::error::ShellError;

/// Create an [`EGLDisplay`] and [`GlesRenderer`] from a [`GbmDevice`].
///
/// # Errors
///
/// Returns [`ShellError::Egl`] or [`ShellError::Gles`] if display, context, or renderer
/// initialization fails.
pub fn create_drm_renderer<A: AsFd + Clone + Send + 'static>(
    gbm: &GbmDevice<A>,
) -> Result<(EGLDisplay, GlesRenderer), ShellError> {
    // SAFETY: gbm is an open, valid GbmDevice owned exclusively by our compositor session.
    // No other library or thread in this process calls eglGetPlatformDisplay or eglTerminate.
    let display = unsafe { EGLDisplay::new(gbm.clone()) }
        .map_err(|err| ShellError::Egl(format!("failed to initialize EGLDisplay: {err}")))?;

    let context = EGLContext::new(&display)
        .map_err(|err| ShellError::Egl(format!("failed to create EGLContext: {err}")))?;

    // SAFETY: context is a newly initialized, valid EGLContext associated with the active
    // EGLDisplay. GlesRenderer initializes GL ES function tables on the current thread.
    let renderer = unsafe { GlesRenderer::new(context) }
        .map_err(|err| ShellError::Gles(format!("failed to initialize GlesRenderer: {err}")))?;

    Ok((display, renderer))
}

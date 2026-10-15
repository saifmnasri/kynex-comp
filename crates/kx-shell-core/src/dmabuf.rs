// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Linux DMA-BUF protocol handler (`zwp_linux_dmabuf_v1`).
//!
//! Provides zero-copy buffer sharing between clients and compositor using
//! DMA-BUF file descriptors, version 4 format tables, and default device feedback tranches.

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::{Buffer, Format, Fourcc, Modifier};
use smithay::delegate_dmabuf;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::dmabuf::{
    DmabufFeedback, DmabufFeedbackBuilder, DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier,
};

use crate::state::KynexState;

/// Maximum allowable dimension for a DMA-BUF plane in pixels.
///
/// Prevents integer overflow and memory exhaustion `DoS` attacks from hostile clients.
pub const MAX_DMABUF_DIMENSION: u32 = 16_384;

/// Default formats advertised by the dmabuf protocol handler.
#[must_use]
pub fn default_dmabuf_formats() -> Vec<Format> {
    vec![
        Format {
            code: Fourcc::Argb8888,
            modifier: Modifier::Linear,
        },
        Format {
            code: Fourcc::Xrgb8888,
            modifier: Modifier::Linear,
        },
        Format {
            code: Fourcc::Abgr8888,
            modifier: Modifier::Linear,
        },
        Format {
            code: Fourcc::Xbgr8888,
            modifier: Modifier::Linear,
        },
    ]
}

/// Builds the default dmabuf feedback for the compositor.
///
/// If a device is provided (e.g. from a DRM render node), uses its `dev_t`;
/// otherwise falls back to a device identifier of 0.
///
/// # Errors
/// Returns an [`std::io::Error`] if the memory-mapped file for format tables
/// cannot be allocated.
pub fn build_default_feedback(
    main_device: Option<u64>,
    formats: &[Format],
) -> Result<DmabufFeedback, std::io::Error> {
    let device = main_device.unwrap_or(0);
    let builder = DmabufFeedbackBuilder::new(device, formats.iter().copied());
    builder.build()
}

/// Initializes the `zwp_linux_dmabuf_v1` global with default feedback tables.
#[must_use]
pub fn init_dmabuf_global(
    dh: &DisplayHandle,
    state: &mut DmabufState,
    main_device: Option<u64>,
    custom_formats: Option<Vec<Format>>,
) -> Option<DmabufGlobal> {
    let formats = custom_formats.unwrap_or_else(default_dmabuf_formats);
    match build_default_feedback(main_device, &formats) {
        Ok(feedback) => {
            let global = state.create_global_with_default_feedback::<KynexState>(dh, &feedback);
            Some(global)
        }
        Err(err) => {
            tracing::error!(?err, "Failed to build default dmabuf feedback table");
            None
        }
    }
}

impl DmabufHandler for KynexState {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        // Enforce plane limits (Rule 04/05 anti-DoS)
        let num_planes = dmabuf.num_planes();
        if num_planes == 0 || num_planes > 4 {
            tracing::warn!(num_planes, "Rejected dmabuf: invalid number of planes");
            notifier.failed();
            return;
        }

        // Enforce dimension sanity (Rule 04/05 anti-DoS)
        let width = dmabuf.width();
        let height = dmabuf.height();
        if width == 0
            || height == 0
            || width > MAX_DMABUF_DIMENSION
            || height > MAX_DMABUF_DIMENSION
        {
            tracing::warn!(width, height, "Rejected dmabuf: invalid dimensions");
            notifier.invalid_dimensions();
            return;
        }

        if let Err(err) = notifier.successful::<KynexState>() {
            tracing::warn!(?err, "Failed to complete dmabuf import notifier");
        }
    }
}

delegate_dmabuf!(KynexState);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_dmabuf_formats() {
        let formats = default_dmabuf_formats();
        assert_ne!(formats.len(), 0);
        assert!(formats.iter().any(|f| f.code == Fourcc::Argb8888));
        assert!(formats.iter().any(|f| f.code == Fourcc::Xrgb8888));
    }

    #[test]
    fn test_build_default_feedback() {
        let formats = default_dmabuf_formats();
        let feedback = build_default_feedback(None, &formats);
        assert!(
            feedback.is_ok(),
            "Default feedback builder must succeed with fallback dev_t"
        );
    }
}

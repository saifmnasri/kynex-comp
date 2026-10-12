// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! DRM output modesetting pipeline and KMS display initialization.

use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::gbm::GbmAllocator;
use smithay::backend::drm::DrmDeviceFd;
use smithay::backend::drm::exporter::gbm::GbmFramebufferExporter;
use smithay::backend::drm::output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements};
use smithay::backend::renderer::ImportDma;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::output::{Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::drm::control::connector;
use smithay::reexports::drm::control::{Device, Mode, ModeTypeFlags, RawResourceHandle, crtc};
use smithay::utils::{Point, Transform};
use tracing::info;

use crate::drm::gpu::DrmGpuResources;
use crate::error::ShellError;

/// Active DRM output modesetting pipeline managing scanout on a CRTC and connector.
pub struct DrmOutputPipeline {
    /// DRM output manager driving atomic/legacy commits and swapchains.
    pub manager: DrmOutputManager<
        GbmAllocator<DrmDeviceFd>,
        GbmFramebufferExporter<DrmDeviceFd>,
        (),
        DrmDeviceFd,
    >,
    /// Managed DRM output interface.
    pub output:
        DrmOutput<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>,
    /// Assigned CRTC hardware handle.
    pub crtc: crtc::Handle,
    /// Applied DRM video mode.
    pub mode: Mode,
    /// Associated compositor logical/physical output representation.
    pub kynex_output: Output,
}

impl DrmOutputPipeline {
    /// Initialize the primary output on the provided GPU resources.
    ///
    /// # Errors
    ///
    /// Returns [`ShellError::Output`] if no connected connector or CRTC is found,
    /// or if output modeset initialization fails.
    pub fn init(gpu: &mut DrmGpuResources) -> Result<Self, ShellError> {
        let (connector_handle, connector_info, preferred_mode) =
            find_primary_connector(&gpu.drm_fd)?;

        let crtcs = gpu
            .drm_fd
            .resource_handles()
            .map_err(|e| ShellError::Drm(format!("failed to query DRM resources: {e}")))?
            .crtcs()
            .to_vec();

        let crtc = crtcs.first().copied().ok_or_else(|| {
            ShellError::Output("no available CRTC found on DRM device".to_string())
        })?;

        let kynex_output = create_kynex_output(connector_handle, &connector_info, &preferred_mode);

        let color_formats = [Fourcc::Argb8888, Fourcc::Xrgb8888];
        let renderer_formats = gpu.renderer.dmabuf_formats();

        let drm_device = gpu.drm_device.take().ok_or_else(|| {
            ShellError::Output("DRM device has already been initialized".to_string())
        })?;

        let mut manager = DrmOutputManager::new(
            drm_device,
            gpu.allocator.clone(),
            gpu.exporter.clone(),
            Some(gpu.gbm.clone()),
            color_formats,
            renderer_formats,
        );

        let render_elements = DrmOutputRenderElements::<
            GlesRenderer,
            WaylandSurfaceRenderElement<GlesRenderer>,
        >::default();

        let output = manager
            .initialize_output(
                crtc,
                preferred_mode,
                &[connector_handle],
                &kynex_output,
                None,
                &mut gpu.renderer,
                &render_elements,
            )
            .map_err(|e| ShellError::Output(format!("failed to initialize DRM output: {e:?}")))?;

        info!(
            connector = ?connector_handle,
            ?crtc,
            mode = ?preferred_mode,
            "DRM primary output pipeline initialized successfully"
        );

        Ok(Self {
            manager,
            output,
            crtc,
            mode: preferred_mode,
            kynex_output,
        })
    }
}

fn find_primary_connector(
    drm_fd: &DrmDeviceFd,
) -> Result<(connector::Handle, connector::Info, Mode), ShellError> {
    let res = drm_fd
        .resource_handles()
        .map_err(|e| ShellError::Drm(format!("failed to query DRM resource handles: {e}")))?;

    for handle in res.connectors() {
        let Ok(info) = drm_fd.get_connector(*handle, false) else {
            continue;
        };
        if info.state() == connector::State::Connected && !info.modes().is_empty() {
            let preferred_mode = info
                .modes()
                .iter()
                .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
                .or_else(|| info.modes().first())
                .copied()
                .ok_or_else(|| {
                    ShellError::Output("connected connector has no available modes".to_string())
                })?;
            return Ok((*handle, info, preferred_mode));
        }
    }

    Err(ShellError::Output(
        "no connected DRM display connector found".to_string(),
    ))
}

fn create_kynex_output(
    connector_handle: connector::Handle,
    connector_info: &connector::Info,
    mode: &Mode,
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
        Some(Point::from((0, 0))),
    );

    output
}

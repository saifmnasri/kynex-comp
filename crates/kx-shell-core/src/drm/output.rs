// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! DRM output modesetting pipeline and multi-monitor KMS display initialization.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::gbm::GbmAllocator;
use smithay::backend::drm::DrmDeviceFd;
use smithay::backend::drm::compositor::{FrameFlags, PrimaryPlaneElement};
use smithay::backend::drm::exporter::gbm::GbmFramebufferExporter;
use smithay::backend::drm::output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements};
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::{Color32F, ImportDma};
use smithay::output::{Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::drm::control::connector;
use smithay::reexports::drm::control::{Device, Mode, ModeTypeFlags, RawResourceHandle, crtc};
use smithay::utils::{Logical, Point, Transform};
use tracing::info;

use crate::drm::gpu::DrmGpuResources;
use crate::drm::layout::{ConnectorCandidate, allocate_outputs};
use crate::error::ShellError;
use crate::frame_clock::FrameClock;

/// Single active DRM output driving a CRTC and connector.
pub struct DrmOutputEntry {
    /// Managed DRM output interface driving scanout on this CRTC.
    pub output:
        DrmOutput<GbmAllocator<DrmDeviceFd>, GbmFramebufferExporter<DrmDeviceFd>, (), DrmDeviceFd>,
    /// Assigned CRTC hardware handle.
    pub crtc: crtc::Handle,
    /// Connected connector hardware handle.
    pub connector: connector::Handle,
    /// Applied DRM video mode.
    pub mode: Mode,
    /// Associated compositor logical/physical output representation.
    pub kynex_output: Output,
    /// Per-output frame pacing clock.
    pub clock: FrameClock,
    /// Frame counter for this output.
    pub frame_count: u64,
    /// Global coordinate space placement origin.
    pub position: Point<i32, Logical>,
}

/// Type alias for the DRM output manager type used by the pipeline.
pub type PipelineDrmOutputManager = DrmOutputManager<
    GbmAllocator<DrmDeviceFd>,
    GbmFramebufferExporter<DrmDeviceFd>,
    (),
    DrmDeviceFd,
>;

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

/// Active DRM output modesetting pipeline managing scanout on multiple CRTCs and connectors.
pub struct DrmOutputPipeline {
    /// DRM output manager driving atomic/legacy commits and swapchains.
    pub manager: PipelineDrmOutputManager,
    /// Active outputs mapped by assigned CRTC handle.
    pub outputs: HashMap<crtc::Handle, DrmOutputEntry>,
}

impl DrmOutputPipeline {
    /// Initialize all connected outputs on the provided GPU resources.
    ///
    /// Connected connectors are automatically matched to available CRTCs using
    /// deterministic CRTC allocation, placed horizontally side-by-side in global space.
    ///
    /// # Errors
    ///
    /// Returns [`ShellError::Output`] if no connected connector or CRTC is found,
    /// or if output modeset initialization fails.
    pub fn init(gpu: &mut DrmGpuResources) -> Result<Self, ShellError> {
        let detected = scan_connected_connectors(&gpu.drm_fd)?;
        if detected.is_empty() {
            return Err(ShellError::Output(
                "no connected DRM display connector found".to_string(),
            ));
        }

        let crtcs = gpu
            .drm_fd
            .resource_handles()
            .map_err(|e| ShellError::Drm(format!("failed to query DRM resources: {e}")))?
            .crtcs()
            .to_vec();

        let candidates: Vec<_> = detected.iter().map(|c| c.candidate.clone()).collect();
        let assignments = allocate_outputs(&candidates, &crtcs);
        if assignments.is_empty() {
            return Err(ShellError::Output(
                "failed to allocate any CRTC for connected connectors".to_string(),
            ));
        }

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

        let [r, g, b, a] = crate::nested::DEFAULT_BG_COLOR;
        let bg_color = Color32F::new(r, g, b, a);

        let mut outputs = HashMap::with_capacity(assignments.len());

        for assignment in &assignments {
            let Some(scanned) = detected.iter().find(|c| c.handle == assignment.connector) else {
                continue;
            };

            let entry = initialize_output_entry(
                &mut manager,
                &mut gpu.renderer,
                assignment,
                scanned,
                bg_color,
            )?;
            outputs.insert(assignment.crtc, entry);
        }

        Ok(Self { manager, outputs })
    }

    /// Returns the handle of the primary output CRTC (positioned at `(0, 0)`).
    #[must_use]
    pub fn primary_crtc(&self) -> Option<crtc::Handle> {
        self.outputs
            .iter()
            .find(|(_, entry)| entry.position == Point::from((0, 0)))
            .map(|(crtc, _)| *crtc)
            .or_else(|| self.outputs.keys().next().copied())
    }

    /// Returns a reference to the primary output entry.
    #[must_use]
    pub fn primary_output(&self) -> Option<&DrmOutputEntry> {
        self.primary_crtc().and_then(|crtc| self.outputs.get(&crtc))
    }

    /// Returns a mutable reference to the primary output entry.
    pub fn primary_output_mut(&mut self) -> Option<&mut DrmOutputEntry> {
        let crtc = self.primary_crtc()?;
        self.outputs.get_mut(&crtc)
    }

    /// Marks scanout completed for the output associated with `crtc` and returns its entry.
    pub fn on_vblank(&mut self, crtc: crtc::Handle) -> Option<&mut DrmOutputEntry> {
        let entry = self.outputs.get_mut(&crtc)?;
        let _ = entry.output.frame_submitted();
        let now = Instant::now();
        if let Err(err) = entry.clock.on_vblank(now) {
            tracing::trace!(%err, ?crtc, "Frame clock vblank notification");
        }
        tracing::trace!(
            ?crtc,
            frame_count = entry.clock.frame_count(),
            missed_vblanks = entry.clock.missed_vblanks(),
            "DRM vblank presented"
        );
        Some(entry)
    }

    /// Renders and queues a frame on the output associated with the specified CRTC.
    ///
    /// # Errors
    ///
    /// Returns [`ShellError::Output`] if rendering or queueing fails on the DRM device.
    pub fn render_output_frame(
        &mut self,
        crtc: crtc::Handle,
        gpu: &mut DrmGpuResources,
        bg_color: Color32F,
    ) -> Result<(), ShellError> {
        let Some(entry) = self.outputs.get_mut(&crtc) else {
            tracing::warn!(?crtc, "Attempted to render unknown or removed CRTC");
            return Ok(());
        };

        let now = Instant::now();
        entry.clock.schedule_frame();
        if let Err(err) = entry.clock.on_render_begin(now) {
            tracing::trace!(%err, ?crtc, "Frame clock render begin notification");
        }

        let render_elements: [WaylandSurfaceRenderElement<GlesRenderer>; 0] = [];
        let frame_mode = FrameFlags::DEFAULT | FrameFlags::ALLOW_CURSOR_PLANE_SCANOUT;

        let render_result = entry
            .output
            .render_frame(&mut gpu.renderer, &render_elements, bg_color, frame_mode)
            .map_err(|err| {
                ShellError::Output(format!("DRM render_frame error on CRTC {crtc:?}: {err:?}"))
            })?;

        if render_result.is_empty {
            tracing::trace!(?crtc, "Render frame has no damage; skipping queue_frame");
            entry.clock.on_render_skipped();
            return Ok(());
        }

        if render_result.needs_sync() {
            if let PrimaryPlaneElement::Swapchain(ref element) = render_result.primary_element {
                if let Err(err) = element.sync.wait() {
                    tracing::warn!(%err, ?crtc, "Failed to wait on swapchain sync fence");
                }
            }
        }

        entry.output.queue_frame(()).map_err(|err| {
            ShellError::Output(format!("DRM queue_frame error on CRTC {crtc:?}: {err:?}"))
        })?;

        let submitted_now = Instant::now();
        if let Err(err) = entry.clock.on_render_submitted(submitted_now) {
            tracing::trace!(%err, ?crtc, "Frame clock render submitted notification");
        }

        Self::log_frame_timing(
            crtc,
            submitted_now.saturating_duration_since(now),
            &entry.clock,
        );

        Ok(())
    }

    fn log_frame_timing(
        crtc: crtc::Handle,
        render_duration: Duration,
        clock: &crate::frame_clock::FrameClock,
    ) {
        tracing::trace!(
            ?crtc,
            duration_us = render_duration.as_micros(),
            p99_us = clock.p99_render_duration().as_micros(),
            "DRM frame submitted to hardware"
        );
        if render_duration > Duration::from_millis(4) {
            tracing::debug!(
                ?crtc,
                duration_us = render_duration.as_micros(),
                "Frame render CPU duration exceeded 4ms budget"
            );
        }
    }

    /// Renders a solid background frame to all active outputs.
    ///
    /// # Errors
    ///
    /// Returns [`ShellError::Output`] if rendering or queueing fails on any output.
    pub fn render_all_clear_frames(
        &mut self,
        gpu: &mut DrmGpuResources,
        bg_color: Color32F,
    ) -> Result<(), ShellError> {
        let crtcs: Vec<_> = self.outputs.keys().copied().collect();
        for crtc in crtcs {
            self.render_output_frame(crtc, gpu, bg_color)?;
        }
        Ok(())
    }

    /// Returns the global bounding box size spanning all active mapped outputs.
    #[must_use]
    pub fn global_bounds(&self) -> smithay::utils::Size<i32, Logical> {
        let max_x = self
            .outputs
            .values()
            .map(|e| {
                let w = e
                    .kynex_output
                    .current_mode()
                    .map_or(1920, |m| m.size.to_logical(1).w);
                e.position.x.saturating_add(w)
            })
            .max()
            .unwrap_or(1920);

        let max_y = self
            .outputs
            .values()
            .map(|e| {
                let h = e
                    .kynex_output
                    .current_mode()
                    .map_or(1080, |m| m.size.to_logical(1).h);
                e.position.y.saturating_add(h)
            })
            .max()
            .unwrap_or(1080);

        smithay::utils::Size::from((max_x, max_y))
    }
}

pub(crate) fn initialize_output_entry(
    manager: &mut PipelineDrmOutputManager,
    renderer: &mut GlesRenderer,
    assignment: &crate::drm::layout::OutputAssignment<connector::Handle, crtc::Handle>,
    scanned: &ScannedConnector,
    bg_color: Color32F,
) -> Result<DrmOutputEntry, ShellError> {
    let kynex_output = create_kynex_output(
        scanned.handle,
        &scanned.info,
        &scanned.mode,
        assignment.position,
    );

    let mut render_elements = DrmOutputRenderElements::<
        GlesRenderer,
        WaylandSurfaceRenderElement<GlesRenderer>,
    >::default();
    render_elements.add_output(
        &assignment.crtc,
        bg_color,
        std::iter::empty::<WaylandSurfaceRenderElement<GlesRenderer>>(),
    );

    let output = manager
        .initialize_output(
            assignment.crtc,
            scanned.mode,
            &[scanned.handle],
            &kynex_output,
            None,
            renderer,
            &render_elements,
        )
        .map_err(|e| {
            ShellError::Output(format!(
                "failed to initialize DRM output on CRTC {:?}: {e:?}",
                assignment.crtc
            ))
        })?;

    let refresh_rate_mhz = scanned.mode.vrefresh().saturating_mul(1000);
    let clock = FrameClock::new(refresh_rate_mhz);

    info!(
        connector = ?scanned.handle,
        crtc = ?assignment.crtc,
        mode = ?scanned.mode,
        position = ?assignment.position,
        "DRM output pipeline initialized successfully"
    );

    Ok(DrmOutputEntry {
        output,
        crtc: assignment.crtc,
        connector: scanned.handle,
        mode: scanned.mode,
        kynex_output,
        clock,
        frame_count: 0,
        position: assignment.position,
    })
}

pub(crate) fn scan_connected_connectors(
    drm_fd: &DrmDeviceFd,
) -> Result<Vec<ScannedConnector>, ShellError> {
    let res = drm_fd
        .resource_handles()
        .map_err(|e| ShellError::Drm(format!("failed to query DRM resource handles: {e}")))?;

    let available_crtcs = res.crtcs().to_vec();
    let mut detected = Vec::new();

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

            let (width, height) = preferred_mode.size();
            let size = smithay::utils::Size::from((i32::from(width), i32::from(height)));

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

            let raw_id: u32 = Into::<RawResourceHandle>::into(*handle).get();
            let name = format!("{:?}-{raw_id}", info.interface());

            let candidate = ConnectorCandidate {
                handle: *handle,
                name,
                size,
                possible_crtcs,
            };

            detected.push(ScannedConnector {
                handle: *handle,
                info,
                mode: preferred_mode,
                candidate,
            });
        }
    }

    Ok(detected)
}

fn create_kynex_output(
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

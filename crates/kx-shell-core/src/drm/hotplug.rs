// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Dynamic DRM display hotplug handling and monitor migration.

use std::collections::HashSet;

use smithay::backend::renderer::Color32F;
use smithay::reexports::drm::control::{Device, connector, crtc};
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Point, Size};
use tracing::{info, warn};

use crate::drm::gpu::DrmGpuResources;
use crate::drm::layout::OutputAssignment;
use crate::drm::output::{
    DrmOutputPipeline, ScannedConnector, initialize_output_entry, scan_connected_connectors,
};
use crate::error::ShellError;
use crate::state::KynexState;

fn find_surviving_primary(
    pipeline: &DrmOutputPipeline,
    unplugged_crtcs: &[crtc::Handle],
) -> Option<(
    Point<i32, smithay::utils::Logical>,
    Size<i32, smithay::utils::Logical>,
)> {
    pipeline
        .outputs
        .iter()
        .find(|(crtc, _)| !unplugged_crtcs.contains(crtc))
        .map(|(_, entry)| {
            let mode_size = entry
                .kynex_output
                .current_mode()
                .map_or(Size::from((1920, 1080)), |m| m.size.to_logical(1));
            (entry.position, mode_size)
        })
}

fn handle_unplugged_outputs(
    pipeline: &mut DrmOutputPipeline,
    state: &mut KynexState,
    detected: &[ScannedConnector],
) {
    let connected_handles: HashSet<connector::Handle> = detected.iter().map(|c| c.handle).collect();

    let unplugged_crtcs: Vec<_> = pipeline
        .outputs
        .iter()
        .filter(|(_, entry)| !connected_handles.contains(&entry.connector))
        .map(|(crtc, _)| *crtc)
        .collect();

    if unplugged_crtcs.is_empty() {
        return;
    }

    let surviving_primary = find_surviving_primary(pipeline, &unplugged_crtcs);

    for crtc in unplugged_crtcs {
        if let Some(removed) = pipeline.outputs.remove(&crtc) {
            info!(
                ?crtc,
                connector = ?removed.connector,
                "DRM display unplugged, removing output"
            );

            if let Some((primary_pos, primary_size)) = surviving_primary {
                let unplugged_size = removed
                    .kynex_output
                    .current_mode()
                    .map_or(Size::from((1920, 1080)), |m| m.size.to_logical(1));
                state.migrate_windows_from_unplugged(
                    removed.position,
                    unplugged_size,
                    primary_pos,
                    primary_size,
                );
            }

            state.unmap_output(&removed.kynex_output);
        }
    }

    if pipeline.outputs.is_empty() {
        warn!("All DRM outputs disconnected; operating in headless fallback mode");
    }
}

fn find_available_crtcs(
    gpu: &DrmGpuResources,
    pipeline: &DrmOutputPipeline,
) -> Result<Vec<crtc::Handle>, ShellError> {
    let all_crtcs = gpu
        .drm_fd
        .resource_handles()
        .map_err(|e| ShellError::Drm(format!("failed to query DRM resources on hotplug: {e}")))?
        .crtcs()
        .to_vec();

    Ok(all_crtcs
        .into_iter()
        .filter(|c| !pipeline.outputs.contains_key(c))
        .collect())
}

fn compute_next_output_x(pipeline: &DrmOutputPipeline) -> i32 {
    pipeline
        .outputs
        .values()
        .map(|e| {
            let w = e
                .kynex_output
                .current_mode()
                .map_or(0, |m| m.size.to_logical(1).w);
            e.position.x.saturating_add(w)
        })
        .max()
        .unwrap_or(0)
}

fn handle_new_connectors(
    pipeline: &mut DrmOutputPipeline,
    gpu: &mut DrmGpuResources,
    state: &mut KynexState,
    dh: &DisplayHandle,
    new_connectors: &[ScannedConnector],
) -> Result<(), ShellError> {
    if new_connectors.is_empty() {
        return Ok(());
    }

    let available_crtcs = find_available_crtcs(gpu, pipeline)?;
    let mut current_x = compute_next_output_x(pipeline);
    let [r, g, b, a] = crate::nested::DEFAULT_BG_COLOR;
    let bg_color = Color32F::new(r, g, b, a);

    for scanned in new_connectors {
        let Some(&matched_crtc) = available_crtcs
            .iter()
            .find(|c| !pipeline.outputs.contains_key(c))
        else {
            warn!(connector = ?scanned.handle, "No available CRTC for newly connected display");
            continue;
        };

        let position = Point::from((current_x, 0));
        current_x = current_x.saturating_add(scanned.candidate.size.w);

        let assignment = OutputAssignment {
            connector: scanned.handle,
            name: scanned.candidate.name.clone(),
            crtc: matched_crtc,
            size: scanned.candidate.size,
            position,
        };

        let entry = initialize_output_entry(
            &mut pipeline.manager,
            &mut gpu.renderer,
            &assignment,
            scanned,
            bg_color,
        )?;

        state.map_output(dh, &entry.kynex_output, entry.position);
        pipeline.outputs.insert(matched_crtc, entry);

        pipeline.render_output_frame(matched_crtc, gpu, bg_color)?;
        info!(
            crtc = ?matched_crtc,
            connector = ?scanned.handle,
            ?position,
            "Newly plugged DRM output initialized and mapped"
        );
    }

    Ok(())
}

impl DrmOutputPipeline {
    /// Dynamically handles monitor hotplug and unplug events discovered via udev.
    ///
    /// Connected connectors are scanned, unplugged outputs removed with window
    /// migration to surviving displays, and newly detected monitors initialized and mapped.
    ///
    /// # Errors
    ///
    /// Returns [`ShellError`] if rescanning DRM connectors or initializing new displays fails.
    pub fn handle_hotplug(
        &mut self,
        gpu: &mut DrmGpuResources,
        state: &mut KynexState,
        dh: &DisplayHandle,
    ) -> Result<(), ShellError> {
        let detected = scan_connected_connectors(&gpu.drm_fd)?;
        handle_unplugged_outputs(self, state, &detected);

        let active_connectors: HashSet<connector::Handle> =
            self.outputs.values().map(|e| e.connector).collect();
        let new_connectors: Vec<_> = detected
            .into_iter()
            .filter(|c| !active_connectors.contains(&c.handle))
            .collect();

        handle_new_connectors(self, gpu, state, dh, &new_connectors)
    }
}

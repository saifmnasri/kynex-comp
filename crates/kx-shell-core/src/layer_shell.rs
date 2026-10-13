// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Layer shell protocol implementation (`zwlr_layer_shell_v1`).
//!
//! Provides support for desktop shell components (panels, docks, notifications,
//! wallpapers) with strict authorization gating via `PrivilegeConfig` allow-lists.

use smithay::delegate_layer_shell;
use smithay::desktop::{LayerSurface, WindowSurfaceType, layer_map_for_output};
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::shell::wlr_layer::{
    Layer, LayerSurface as WlrLayerSurface, LayerSurfaceConfigure, WlrLayerShellHandler,
    WlrLayerShellState,
};

use crate::client::ClientState;
use crate::state::KynexState;

impl WlrLayerShellHandler for KynexState {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        if !check_layer_surface_privilege_and_quota(&surface) {
            return;
        }

        let target_output = resolve_layer_target_output(
            self.space.outputs().cloned(),
            &self.output,
            output.as_ref(),
        );

        // Bound namespace length to 256 bytes (Rule 05)
        let bounded_namespace = if namespace.len() > 256 {
            namespace.chars().take(256).collect()
        } else {
            namespace
        };

        let layer_surface = LayerSurface::new(surface, bounded_namespace);
        let mut layer_map = layer_map_for_output(&target_output);
        if let Err(err) = layer_map.map_layer(&layer_surface) {
            tracing::error!(%err, "Failed to map layer surface to output");
            return;
        }

        // Send initial configure to prompt client buffer attachment
        layer_surface.layer_surface().send_configure();
    }

    fn ack_configure(&mut self, surface: WlSurface, _configure: LayerSurfaceConfigure) {
        for output in self.space.outputs() {
            let mut layer_map = layer_map_for_output(output);
            if layer_map
                .layer_for_surface(&surface, WindowSurfaceType::all())
                .is_some()
            {
                layer_map.arrange();
                break;
            }
        }
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        if let Some(client) = surface.wl_surface().client() {
            if let Some(client_data) = client.get_data::<ClientState>() {
                client_data.decrement_layer_surface();
            }
        }

        for output in self.space.outputs() {
            let mut layer_map = layer_map_for_output(output);
            let to_remove = layer_map
                .layers()
                .find(|l| l.layer_surface() == &surface)
                .cloned();
            if let Some(layer) = to_remove {
                layer_map.unmap_layer(&layer);
                break;
            }
        }
    }
}

delegate_layer_shell!(KynexState);

fn check_layer_surface_privilege_and_quota(surface: &WlrLayerSurface) -> bool {
    let Some(client) = surface.wl_surface().client() else {
        return false;
    };

    // Fail-closed authorization check
    let is_allowed = client
        .get_data::<ClientState>()
        .is_some_and(ClientState::is_layer_shell_allowed);

    if !is_allowed {
        tracing::warn!("Unprivileged client attempted to create layer surface; denying request");
        return false;
    }

    // Anti-DoS resource quota enforcement per client
    if let Some(client_data) = client.get_data::<ClientState>() {
        if !client_data.try_increment_layer_surface() {
            tracing::warn!(
                max = client_data.limits.max_layer_surfaces,
                "Client exceeded max layer surfaces quota; posting protocol error"
            );
            // Protocol error 2: wl_display::Error::NoMemory per Wayland specification
            surface
                .wl_surface()
                .post_error(2u32, "Maximum layer surface quota exceeded");
            return false;
        }
    }

    true
}

fn resolve_layer_target_output(
    outputs: impl Iterator<Item = Output>,
    default_output: &Output,
    requested: Option<&WlOutput>,
) -> Output {
    requested
        .and_then(Output::from_resource)
        .or_else(|| outputs.into_iter().next())
        .unwrap_or_else(|| default_output.clone())
}

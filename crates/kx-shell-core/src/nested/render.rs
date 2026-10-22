// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Render element aggregation and session-lock rendering gating for nested compositor.

use smithay::backend::renderer::element::render_elements;
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::{ImportAll, ImportMem};
use smithay::desktop::layer_map_for_output;
use smithay::utils::Scale;

use crate::state::KynexState;

render_elements! {
    /// Combined render element for nested compositor scene graph.
    pub NestedRenderElement<R> where R: ImportAll + ImportMem;
    /// Wayland client surface element (lock surface, layer shell).
    Surface=WaylandSurfaceRenderElement<R>,
    /// Solid color server-side decoration border element.
    SolidColor=SolidColorRenderElement,
}

/// Aggregates custom render elements for the current nested frame.
///
/// Enforces SEC-01 fail-closed session lock gating:
/// - When locked: desktop windows in `space` are NOT rendered, SSD decoration
///   borders are suppressed, background clear color is forced to opaque black,
///   and only active lock surfaces (if any exist) are composited.
/// - When not locked: desktop space is rendered, SSD elements and layer surfaces
///   are composited, and configured background color is preserved.
#[must_use]
pub fn collect_nested_render_elements(
    state: &KynexState,
    renderer: &mut GlesRenderer,
) -> Vec<NestedRenderElement<GlesRenderer>> {
    let mut elements = Vec::new();

    if state.is_session_locked {
        for lock_surface in &state.lock_surfaces {
            if !lock_surface.alive() {
                continue;
            }
            let surface_elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                render_elements_from_surface_tree(
                    renderer,
                    lock_surface.wl_surface(),
                    (0, 0),
                    Scale::from(1.0),
                    1.0,
                    smithay::backend::renderer::element::Kind::Unspecified,
                );
            elements.extend(surface_elements.into_iter().map(NestedRenderElement::from));
        }
        return elements;
    }

    // 1. Layer-shell surfaces (status bar, wallpaper, dock)
    let layer_map = layer_map_for_output(&state.output);
    for layer_surface in layer_map.layers() {
        let loc = layer_map
            .layer_geometry(layer_surface)
            .map_or((0, 0), |g| (g.loc.x, g.loc.y));
        let surface_elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
            render_elements_from_surface_tree(
                renderer,
                layer_surface.wl_surface(),
                loc,
                Scale::from(1.0),
                1.0,
                smithay::backend::renderer::element::Kind::Unspecified,
            );
        elements.extend(surface_elements.into_iter().map(NestedRenderElement::from));
    }

    // 2. Server-side decoration borders and titlebars
    let ssd_elements = crate::decoration::collect_ssd_render_elements(state);
    elements.extend(ssd_elements.into_iter().map(NestedRenderElement::from));

    elements
}

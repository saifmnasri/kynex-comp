// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Window management, state synchronization, and space integration for `kx-shell-core`.

use std::collections::HashSet;

use kx_layout::{
    MAX_STRING_BYTES, MapWindowParams, SizeConstraints, WindowGeometry, WindowId, truncate_utf8,
};
use smithay::desktop::Window;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::Point;
use smithay::wayland::compositor;
use smithay::wayland::shell::xdg::{SurfaceCachedState, ToplevelSurface, XdgToplevelSurfaceData};

use crate::state::KynexState;

/// Computes the usable logical geometry of the primary display output.
#[must_use]
pub fn primary_output_usable_geometry(state: &KynexState) -> WindowGeometry {
    state.space.output_geometry(&state.output).map_or_else(
        || WindowGeometry::new(0, 0, 1280, 720),
        |geo| WindowGeometry::new(geo.loc.x, geo.loc.y, geo.size.w, geo.size.h),
    )
}

/// Extracts title, app ID, and size constraints from a Smithay [`ToplevelSurface`].
#[must_use]
pub fn extract_toplevel_info(surface: &ToplevelSurface) -> (String, String, SizeConstraints) {
    let (raw_title, raw_app_id) = compositor::with_states(surface.wl_surface(), |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .and_then(|data| {
                data.lock().ok().map(|role| {
                    (
                        role.title.clone().unwrap_or_default(),
                        role.app_id.clone().unwrap_or_default(),
                    )
                })
            })
    })
    .unwrap_or_default();

    let constraints = compositor::with_states(surface.wl_surface(), |states| {
        let mut binding = states.cached_state.get::<SurfaceCachedState>();
        let cached = binding.current();
        SizeConstraints {
            min_width: cached.min_size.w.max(0),
            min_height: cached.min_size.h.max(0),
            max_width: if cached.max_size.w > 0 {
                Some(cached.max_size.w)
            } else {
                None
            },
            max_height: if cached.max_size.h > 0 {
                Some(cached.max_size.h)
            } else {
                None
            },
        }
    });

    let safe_title = truncate_utf8(&raw_title, MAX_STRING_BYTES);
    let safe_app_id = truncate_utf8(&raw_app_id, MAX_STRING_BYTES);

    (safe_title, safe_app_id, constraints)
}

/// Synchronizes the authoritative layout geometry from [`kx_layout::WindowManager`]
/// into Smithay's 2D [`smithay::desktop::Space`].
pub fn sync_layout_to_space(state: &mut KynexState) {
    let visible = state.window_manager.visible_windows();
    let visible_set: HashSet<WindowId> = visible.iter().map(|(id, _)| *id).collect();

    for (id, geom) in visible {
        if let Some(window) = state.window_map.get(&id).cloned() {
            let target_loc = Point::from((geom.x, geom.y));
            let current_loc = state.space.element_location(&window);
            if current_loc != Some(target_loc) {
                state.space.map_element(window, target_loc, false);
            }
        }
    }

    let mapped_elements: Vec<Window> = state.space.elements().cloned().collect();
    for window in mapped_elements {
        let win_id = state
            .window_map
            .iter()
            .find_map(|(id, w)| if w == &window { Some(*id) } else { None });

        if let Some(id) = win_id {
            if !visible_set.contains(&id) {
                state.space.unmap_elem(&window);
            }
        }
    }

    state.space.refresh();
}

/// Handles creation and authoritative mapping of a new [`ToplevelSurface`].
pub fn map_new_toplevel(state: &mut KynexState, surface: &ToplevelSurface) {
    let (title, app_id, constraints) = extract_toplevel_info(surface);
    let usable_area = primary_output_usable_geometry(state);

    let params = MapWindowParams {
        geometry: WindowGeometry::new(0, 0, 800, 600),
        constraints,
        title: &title,
        app_id: &app_id,
        target_workspace: None,
        usable_area,
        focus_new_windows_cfg: true,
    };

    let window_id = match state.window_manager.map_window(&params) {
        Ok(id) => id,
        Err(err) => {
            tracing::warn!(?err, "Failed to map window into WindowManager; dropping");
            return;
        }
    };

    let window = Window::new_wayland_window(surface.clone());
    state.window_map.insert(window_id, window);
    state
        .surface_to_window_id
        .insert(surface.wl_surface().clone(), window_id);

    if let Some(win_data) = state.window_manager.window(window_id) {
        let geom = win_data.geometry;
        surface.with_pending_state(|toplevel_state| {
            toplevel_state.states.set(xdg_toplevel::State::Activated);
            toplevel_state.size = Some((geom.width, geom.height).into());
        });
    }

    surface.send_configure();
    sync_layout_to_space(state);

    if let Some(keyboard) = state.seat.get_keyboard() {
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        keyboard.set_focus(state, Some(surface.wl_surface().clone()), serial);
    }
}

/// Handles destruction and unmapping of a [`ToplevelSurface`].
pub fn unmap_toplevel(state: &mut KynexState, surface: &ToplevelSurface) {
    if let Some(keyboard) = state.seat.get_keyboard() {
        if keyboard.current_focus().as_ref() == Some(surface.wl_surface()) {
            let serial = smithay::utils::SERIAL_COUNTER.next_serial();
            keyboard.set_focus(state, None, serial);
        }
    }

    if let Some(window_id) = state.surface_to_window_id.remove(surface.wl_surface()) {
        state.window_manager.unmap_window(window_id);
        if let Some(window) = state.window_map.remove(&window_id) {
            state.space.unmap_elem(&window);
        }
        sync_layout_to_space(state);
    }
}

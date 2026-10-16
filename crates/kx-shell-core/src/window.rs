// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Window management, state synchronization, and space integration for `kx-shell-core`.

use std::collections::HashSet;

use kx_layout::{
    MAX_STRING_BYTES, MapWindowParams, SizeConstraints, WindowGeometry, WindowId, truncate_utf8,
};
use smithay::desktop::{Window, layer_map_for_output};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::Point;
use smithay::wayland::compositor;
use smithay::wayland::shell::xdg::{
    Configure, SurfaceCachedState, ToplevelSurface, XdgToplevelSurfaceData,
};

use crate::state::KynexState;

use smithay::output::Output;

/// Computes the usable logical geometry of the given display output.
#[must_use]
pub fn output_usable_geometry(state: &KynexState, output: &Output) -> WindowGeometry {
    state.space.output_geometry(output).map_or_else(
        || {
            let mode_size = output
                .current_mode()
                .map_or_else(|| (1280, 720).into(), |m| m.size);
            WindowGeometry::new(0, 0, mode_size.w, mode_size.h)
        },
        |geo| {
            let mut layer_map = layer_map_for_output(output);
            if layer_map.layers().next().is_none() {
                WindowGeometry::new(geo.loc.x, geo.loc.y, geo.size.w, geo.size.h)
            } else {
                layer_map.arrange();
                let zone = layer_map.non_exclusive_zone();
                if zone.size.w > 0 && zone.size.h > 0 {
                    WindowGeometry::new(
                        geo.loc.x.saturating_add(zone.loc.x),
                        geo.loc.y.saturating_add(zone.loc.y),
                        zone.size.w,
                        zone.size.h,
                    )
                } else {
                    WindowGeometry::new(geo.loc.x, geo.loc.y, geo.size.w, geo.size.h)
                }
            }
        },
    )
}

/// Computes the usable logical geometry of the primary display output.
#[must_use]
pub fn primary_output_usable_geometry(state: &KynexState) -> WindowGeometry {
    output_usable_geometry(state, &state.output)
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

    crate::foreign_toplevel::register_foreign_toplevel(state, window_id, &title, &app_id);

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
        crate::foreign_toplevel::unregister_foreign_toplevel(state, window_id);
        state.window_manager.unmap_window(window_id);
        if let Some(window) = state.window_map.remove(&window_id) {
            state.space.unmap_elem(&window);
        }
        sync_layout_to_space(state);
    }
}

/// Determines the primary [`Output`] for a window based on maximal overlap area.
#[must_use]
pub fn primary_output_for_window(state: &KynexState, wid: WindowId) -> Option<Output> {
    let win_geo = state.window_manager.window(wid)?.geometry;

    let mut best_output = None;
    let mut max_area: i64 = 0;

    for output in state.space.outputs() {
        if let Some(out_geo) = state.space.output_geometry(output) {
            let x1 = win_geo.x.max(out_geo.loc.x);
            let x2 = win_geo
                .x
                .saturating_add(win_geo.width)
                .min(out_geo.loc.x.saturating_add(out_geo.size.w));
            let y1 = win_geo.y.max(out_geo.loc.y);
            let y2 = win_geo
                .y
                .saturating_add(win_geo.height)
                .min(out_geo.loc.y.saturating_add(out_geo.size.h));

            if x2 > x1 && y2 > y1 {
                let w = i64::from(x2 - x1);
                let h = i64::from(y2 - y1);
                let area = w.saturating_mul(h);
                if area > max_area {
                    max_area = area;
                    best_output = Some(output.clone());
                }
            }
        }
    }

    best_output.or_else(|| Some(state.output.clone()))
}

/// Handles a client request to maximize the toplevel window.
pub fn handle_maximize_request(state: &mut KynexState, surface: &ToplevelSurface) {
    let Some(&wid) = state.surface_to_window_id.get(surface.wl_surface()) else {
        return;
    };
    let target_output =
        primary_output_for_window(state, wid).unwrap_or_else(|| state.output.clone());
    let usable_area = output_usable_geometry(state, &target_output);
    state.window_manager.maximize_window(wid, usable_area);

    if let Some(win) = state.window_manager.window(wid) {
        let geom = win.geometry;
        surface.with_pending_state(|toplevel_state| {
            toplevel_state.states.set(xdg_toplevel::State::Maximized);
            toplevel_state.size = Some((geom.width, geom.height).into());
        });
        surface.send_configure();
    }
    sync_layout_to_space(state);
}

/// Handles a client request to unmaximize the toplevel window.
pub fn handle_unmaximize_request(state: &mut KynexState, surface: &ToplevelSurface) {
    let Some(&wid) = state.surface_to_window_id.get(surface.wl_surface()) else {
        return;
    };
    let target_output =
        primary_output_for_window(state, wid).unwrap_or_else(|| state.output.clone());
    let usable_area = output_usable_geometry(state, &target_output);
    state.window_manager.unmaximize_window(wid, usable_area);

    if let Some(win) = state.window_manager.window(wid) {
        let geom = win.geometry;
        surface.with_pending_state(|toplevel_state| {
            toplevel_state.states.unset(xdg_toplevel::State::Maximized);
            toplevel_state.size = Some((geom.width, geom.height).into());
        });
        surface.send_configure();
    }
    sync_layout_to_space(state);
}

/// Handles a client request to fullscreen the toplevel window.
pub fn handle_fullscreen_request(
    state: &mut KynexState,
    surface: &ToplevelSurface,
    output: Option<WlOutput>,
) {
    let Some(&wid) = state.surface_to_window_id.get(surface.wl_surface()) else {
        return;
    };
    let target_output = output
        .as_ref()
        .and_then(Output::from_resource)
        .or_else(|| primary_output_for_window(state, wid))
        .unwrap_or_else(|| state.output.clone());

    let output_geo = state.space.output_geometry(&target_output).map_or_else(
        || {
            let mode_size = target_output
                .current_mode()
                .map_or_else(|| (1920, 1080).into(), |m| m.size);
            WindowGeometry::new(0, 0, mode_size.w, mode_size.h)
        },
        |geo| WindowGeometry::new(geo.loc.x, geo.loc.y, geo.size.w, geo.size.h),
    );
    state.window_manager.fullscreen_window(wid, output_geo);

    if let Some(win) = state.window_manager.window(wid) {
        let geom = win.geometry;
        surface.with_pending_state(|toplevel_state| {
            toplevel_state.states.set(xdg_toplevel::State::Fullscreen);
            toplevel_state.size = Some((geom.width, geom.height).into());
            toplevel_state.fullscreen_output = output;
        });
        surface.send_configure();
    }
    sync_layout_to_space(state);
}

/// Handles a client request to unfullscreen the toplevel window.
pub fn handle_unfullscreen_request(state: &mut KynexState, surface: &ToplevelSurface) {
    let Some(&wid) = state.surface_to_window_id.get(surface.wl_surface()) else {
        return;
    };
    let target_output =
        primary_output_for_window(state, wid).unwrap_or_else(|| state.output.clone());
    let usable_area = output_usable_geometry(state, &target_output);
    state.window_manager.unfullscreen_window(wid, usable_area);

    if let Some(win) = state.window_manager.window(wid) {
        let geom = win.geometry;
        surface.with_pending_state(|toplevel_state| {
            toplevel_state.states.unset(xdg_toplevel::State::Fullscreen);
            toplevel_state.size = Some((geom.width, geom.height).into());
            toplevel_state.fullscreen_output = None;
        });
        surface.send_configure();
    }
    sync_layout_to_space(state);
}

/// Handles a client request to minimize the toplevel window.
pub fn handle_minimize_request(state: &mut KynexState, surface: &ToplevelSurface) {
    let Some(&wid) = state.surface_to_window_id.get(surface.wl_surface()) else {
        return;
    };
    state.window_manager.minimize_window(wid);

    if let Some(focused_wid) = state.window_manager.focus().focused() {
        if let Some(next_window) = state.window_map.get(&focused_wid) {
            if let Some(toplevel) = next_window.toplevel() {
                if let Some(keyboard) = state.seat.get_keyboard() {
                    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                    keyboard.set_focus(state, Some(toplevel.wl_surface().clone()), serial);
                }
            }
        }
    } else if let Some(keyboard) = state.seat.get_keyboard() {
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        keyboard.set_focus(state, None, serial);
    }

    sync_layout_to_space(state);
}

/// Handles client `xdg_surface.ack_configure`.
pub fn handle_ack_configure(_state: &mut KynexState, _surface: &WlSurface, _configure: Configure) {
    // Serials are tracked and validated by Smithay's XdgShellState automatically.
}

/// Handles client title updates.
pub fn handle_title_changed(state: &mut KynexState, surface: &ToplevelSurface) {
    let Some(&wid) = state.surface_to_window_id.get(surface.wl_surface()) else {
        return;
    };
    let title = compositor::with_states(surface.wl_surface(), |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .and_then(|data| data.lock().ok().and_then(|role| role.title.clone()))
    })
    .unwrap_or_default();
    if let Some(win) = state.window_manager.window_mut(wid) {
        win.set_title(&title);
    }
    crate::foreign_toplevel::update_foreign_toplevel_title(state, wid, &title);
}

/// Handles client app ID updates.
pub fn handle_app_id_changed(state: &mut KynexState, surface: &ToplevelSurface) {
    let Some(&wid) = state.surface_to_window_id.get(surface.wl_surface()) else {
        return;
    };
    let app_id = compositor::with_states(surface.wl_surface(), |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .and_then(|data| data.lock().ok().and_then(|role| role.app_id.clone()))
    })
    .unwrap_or_default();
    if let Some(win) = state.window_manager.window_mut(wid) {
        win.set_app_id(&app_id);
    }
    crate::foreign_toplevel::update_foreign_toplevel_app_id(state, wid, &app_id);
}

/// Handles a new popup request, enforcing anti-DoS nesting depth bounds per Rules 04/05.
pub fn handle_new_popup(
    state: &mut KynexState,
    surface: &smithay::wayland::shell::xdg::PopupSurface,
    _positioner: smithay::wayland::shell::xdg::PositionerState,
) {
    const MAX_POPUP_DEPTH: usize = 8;
    let mut depth = 0;
    let mut current_parent = surface.get_parent_surface();
    while let Some(parent) = current_parent {
        depth += 1;
        if depth > MAX_POPUP_DEPTH {
            tracing::warn!(
                depth,
                "Popup nesting depth exceeded limit; sending popup_done"
            );
            surface.send_popup_done();
            return;
        }
        if let Some(parent_popup) = state.popups.find_popup(&parent) {
            current_parent = match parent_popup {
                smithay::desktop::PopupKind::Xdg(ref p) => p.get_parent_surface(),
                smithay::desktop::PopupKind::InputMethod(_) => None,
            };
        } else {
            break;
        }
    }

    if let Err(err) = state
        .popups
        .track_popup(smithay::desktop::PopupKind::from(surface.clone()))
    {
        tracing::warn!(?err, "Failed to track popup in popup manager");
        return;
    }
    if let Err(err) = surface.send_configure() {
        tracing::warn!(?err, "Failed to send initial popup configure");
    }
}

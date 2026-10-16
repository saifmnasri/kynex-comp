// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Output management, layout updates, and multi-monitor output mapping.

use kx_layout::{WindowGeometry, WindowId};
use smithay::desktop::layer_map_for_output;
use smithay::output::{Mode, Output};
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Physical, Point, Size};

use crate::state::KynexState;

impl KynexState {
    /// Updates the output geometry when the nested window is resized.
    pub fn update_output_size(&mut self, new_size: Size<i32, Physical>) {
        let mode = Mode {
            size: new_size,
            refresh: 60_000,
        };
        self.output
            .change_current_state(Some(mode), None, None, None);
        self.space.refresh();

        layer_map_for_output(&self.output).arrange();

        let usable_area = crate::window::output_usable_geometry(self, &self.output);
        let output_geo = self.space.output_geometry(&self.output).map_or_else(
            || WindowGeometry::new(0, 0, new_size.w, new_size.h),
            |geo| WindowGeometry::new(geo.loc.x, geo.loc.y, geo.size.w, geo.size.h),
        );
        self.window_manager.arrange(output_geo, usable_area);

        for (&wid, window) in &self.window_map {
            if let Some(win_data) = self.window_manager.window(wid) {
                if win_data.state == kx_layout::WindowState::Maximized
                    || win_data.state == kx_layout::WindowState::Fullscreen
                {
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.with_pending_state(|toplevel_state| {
                            toplevel_state.size =
                                Some((win_data.geometry.width, win_data.geometry.height).into());
                        });
                        toplevel.send_configure();
                    }
                }
            }
        }

        crate::window::sync_layout_to_space(self);
    }

    /// Maps an additional display output into the compositor's global space.
    pub fn map_output(
        &mut self,
        dh: &DisplayHandle,
        output: &Output,
        location: impl Into<Point<i32, Logical>>,
    ) {
        let _global = output.create_global::<Self>(dh);
        self.space.map_output(output, location);
        self.space.refresh();
    }

    /// Unmaps a display output from the compositor's global space.
    pub fn unmap_output(&mut self, output: &Output) {
        let mut layer_map = layer_map_for_output(output);
        let layers: Vec<_> = layer_map.layers().cloned().collect();
        for layer in layers {
            layer_map.unmap_layer(&layer);
        }
        self.space.unmap_output(output);
        self.space.refresh();
    }

    /// Migrates windows away from an unplugged output so they remain accessible on surviving outputs.
    pub fn migrate_windows_from_unplugged(
        &mut self,
        unplugged_origin: Point<i32, Logical>,
        unplugged_size: Size<i32, Logical>,
        primary_origin: Point<i32, Logical>,
        primary_size: Size<i32, Logical>,
    ) {
        let windows: Vec<(WindowId, Point<i32, Logical>)> = self
            .window_map
            .keys()
            .copied()
            .filter_map(|wid| {
                self.window_manager
                    .window(wid)
                    .map(|w| (wid, Point::from((w.geometry.x, w.geometry.y))))
            })
            .collect();

        for (wid, loc) in windows {
            let new_loc = crate::drm::layout::migrate_window_position(
                loc,
                unplugged_origin,
                unplugged_size,
                primary_origin,
                primary_size,
            );
            if new_loc != loc {
                if let Some(win) = self.window_manager.window(wid) {
                    let mut geom = win.geometry;
                    geom.x = new_loc.x;
                    geom.y = new_loc.y;
                    self.window_manager.set_window_geometry(wid, geom);
                }
            }
        }

        crate::window::sync_layout_to_space(self);
    }
}

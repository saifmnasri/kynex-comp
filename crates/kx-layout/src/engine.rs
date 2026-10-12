// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Floating window layout engine, usable area calculation, and reachability validation.

use crate::window::{WindowData, WindowGeometry, WindowId, WindowState};

/// Pixels to offset each subsequent cascading window horizontally and vertically.
pub const CASCADE_OFFSET_STEP: i32 = 32;

/// Minimum window width that must remain on-screen within usable area (Amendment A8).
pub const MIN_ONSCREEN_WIDTH: i32 = 64;

/// Minimum window height that must remain on-screen within usable area (Amendment A8).
pub const MIN_ONSCREEN_HEIGHT: i32 = 32;

/// Exclusive margins subtracted from the output geometry by layer-shell surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExclusiveMargins {
    /// Pixels reserved at the top of the output (e.g. top panel).
    pub top: i32,
    /// Pixels reserved at the bottom of the output (e.g. dock).
    pub bottom: i32,
    /// Pixels reserved at the left of the output.
    pub left: i32,
    /// Pixels reserved at the right of the output.
    pub right: i32,
}

impl ExclusiveMargins {
    /// Creates exclusive margins with validated non-negative values.
    #[must_use]
    pub fn new(top: i32, bottom: i32, left: i32, right: i32) -> Self {
        Self {
            top: top.max(0),
            bottom: bottom.max(0),
            left: left.max(0),
            right: right.max(0),
        }
    }

    /// Computes the usable area of an output after subtracting exclusive margins.
    ///
    /// Ensures width and height are at least 1 logical pixel.
    #[must_use]
    pub fn compute_usable_area(&self, output_geo: WindowGeometry) -> WindowGeometry {
        let x = output_geo.x.saturating_add(self.left);
        let y = output_geo.y.saturating_add(self.top);
        let horizontal_margins = self.left.saturating_add(self.right);
        let vertical_margins = self.top.saturating_add(self.bottom);

        let width = output_geo.width.saturating_sub(horizontal_margins).max(1);
        let height = output_geo.height.saturating_sub(vertical_margins).max(1);

        WindowGeometry::new(x, y, width, height)
    }
}

/// Clamps window geometry to ensure its titlebar and contents remain reachable on-screen.
///
/// Ensures at least [`MIN_ONSCREEN_WIDTH`] (64px) and [`MIN_ONSCREEN_HEIGHT`] (32px)
/// remains visible within the usable area.
#[must_use]
pub fn clamp_to_reachability(geo: WindowGeometry, usable_area: WindowGeometry) -> WindowGeometry {
    let min_w = MIN_ONSCREEN_WIDTH.min(usable_area.width);
    let min_h = MIN_ONSCREEN_HEIGHT.min(usable_area.height);

    // X clamping:
    // Left edge must not be farther right than usable_area.right() - min_w
    // Right edge must not be farther left than usable_area.left() + min_w
    let max_x = usable_area
        .x
        .saturating_add(usable_area.width)
        .saturating_sub(min_w);
    let min_x = usable_area
        .x
        .saturating_add(min_w)
        .saturating_sub(geo.width);
    let clamped_x = geo.x.clamp(min_x, max_x);

    // Y clamping:
    // Window titlebar (top edge) must be visible at or below usable_area.y
    // and must not be pushed below usable_area.bottom() - min_h
    let min_y = usable_area.y;
    let max_y = usable_area
        .y
        .saturating_add(usable_area.height)
        .saturating_sub(min_h);
    let clamped_y = geo.y.clamp(min_y, max_y);

    WindowGeometry::new(clamped_x, clamped_y, geo.width, geo.height)
}

/// Centers a window within the usable area (used for initial placement or Super+C recovery).
#[must_use]
pub fn center_window(window_size: (i32, i32), usable_area: WindowGeometry) -> WindowGeometry {
    let (w, h) = window_size;
    let w = w.max(1);
    let h = h.max(1);

    let offset_x = usable_area.width.saturating_sub(w) / 2;
    let offset_y = usable_area.height.saturating_sub(h) / 2;

    let x = usable_area.x.saturating_add(offset_x);
    let y = usable_area.y.saturating_add(offset_y);

    clamp_to_reachability(WindowGeometry::new(x, y, w, h), usable_area)
}

/// Geometry of a window when maximized (occupies full usable area).
#[must_use]
pub fn maximized_geometry(usable_area: WindowGeometry) -> WindowGeometry {
    usable_area
}

/// Geometry of a window when fullscreen (occupies entire output display).
#[must_use]
pub fn fullscreen_geometry(output_geo: WindowGeometry) -> WindowGeometry {
    output_geo
}

/// Tracks cascading offsets for placing consecutive floating windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CascadeTracker {
    offset: i32,
    step: i32,
}

impl Default for CascadeTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CascadeTracker {
    /// Creates a new cascade tracker starting at offset 0 with default step 32px.
    #[must_use]
    pub fn new() -> Self {
        Self {
            offset: 0,
            step: CASCADE_OFFSET_STEP,
        }
    }

    /// Calculates the next cascading position for a window of size `(width, height)`.
    ///
    /// Wraps back to offset 0 when the window would exceed the usable area boundary.
    pub fn next_position(
        &mut self,
        window_size: (i32, i32),
        usable_area: WindowGeometry,
    ) -> WindowGeometry {
        let (w, h) = window_size;
        let candidate_x = usable_area.x.saturating_add(self.offset);
        let candidate_y = usable_area.y.saturating_add(self.offset);

        let right = candidate_x.saturating_add(w);
        let bottom = candidate_y.saturating_add(h);

        let area_right = usable_area.x.saturating_add(usable_area.width);
        let area_bottom = usable_area.y.saturating_add(usable_area.height);

        let (x, y) = if right > area_right || bottom > area_bottom {
            self.offset = 0;
            (usable_area.x, usable_area.y)
        } else {
            (candidate_x, candidate_y)
        };

        self.offset = self.offset.saturating_add(self.step);

        clamp_to_reachability(WindowGeometry::new(x, y, w, h), usable_area)
    }

    /// Resets the cascade offset to 0.
    pub fn reset(&mut self) {
        self.offset = 0;
    }
}

/// Common trait implemented by layout policies.
pub trait LayoutEngine: Send + Sync {
    /// Computes and applies geometries to managed windows.
    fn arrange(
        &self,
        output_geo: WindowGeometry,
        usable_area: WindowGeometry,
        windows: &mut [(&WindowId, &mut WindowData)],
    );
}

/// Default floating window layout engine for `KynexComp`.
#[derive(Debug, Clone, Default)]
pub struct FloatingLayout;

impl LayoutEngine for FloatingLayout {
    fn arrange(
        &self,
        output_geo: WindowGeometry,
        usable_area: WindowGeometry,
        windows: &mut [(&WindowId, &mut WindowData)],
    ) {
        for (_wid, win) in windows {
            match win.state {
                WindowState::Normal => {
                    win.geometry = clamp_to_reachability(win.geometry, usable_area);
                }
                WindowState::Maximized => {
                    win.geometry = maximized_geometry(usable_area);
                }
                WindowState::Fullscreen => {
                    win.geometry = fullscreen_geometry(output_geo);
                }
                WindowState::Minimized => {
                    // Geometry retained; surface unmapped in renderer
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::SizeConstraints;
    use slotmap::SlotMap;

    #[test]
    fn test_exclusive_margins_usable_area() {
        let output = WindowGeometry::new(0, 0, 1920, 1080);
        let margins = ExclusiveMargins::new(32, 48, 0, 0); // top bar 32px, bottom dock 48px

        let usable = margins.compute_usable_area(output);
        assert_eq!(usable.x, 0);
        assert_eq!(usable.y, 32);
        assert_eq!(usable.width, 1920);
        assert_eq!(usable.height, 1080 - 32 - 48);

        // Huge margins clamped to at least 1x1
        let huge_margins = ExclusiveMargins::new(1000, 1000, 1000, 1000);
        let clamped_usable = huge_margins.compute_usable_area(output);
        assert_eq!(clamped_usable.width, 1);
        assert_eq!(clamped_usable.height, 1);
    }

    #[test]
    fn test_cascade_tracker_wrapping() {
        let usable = WindowGeometry::new(100, 50, 400, 300);
        let mut tracker = CascadeTracker::new();

        let pos1 = tracker.next_position((200, 150), usable);
        assert_eq!(pos1.x, 100);
        assert_eq!(pos1.y, 50);

        let pos2 = tracker.next_position((200, 150), usable);
        assert_eq!(pos2.x, 132);
        assert_eq!(pos2.y, 82);

        // Advance until it wraps
        let mut wrapped = false;
        for _ in 0..20 {
            let pos = tracker.next_position((200, 150), usable);
            if pos.x == 100 && pos.y == 50 {
                wrapped = true;
                break;
            }
        }
        assert!(
            wrapped,
            "Cascade tracker must wrap when exceeding usable bounds"
        );
    }

    #[test]
    fn test_reachability_clamping() {
        let usable = WindowGeometry::new(0, 0, 1000, 800);

        // Far off to the left: right edge clamped so 64px is on screen
        let far_left = WindowGeometry::new(-500, 100, 400, 300);
        let clamped = clamp_to_reachability(far_left, usable);
        assert_eq!(clamped.x, 64 - 400); // right edge is at x + 400 = 64

        // Far off to the right: left edge clamped so 64px is on screen
        let far_right = WindowGeometry::new(1500, 100, 400, 300);
        let clamped_right = clamp_to_reachability(far_right, usable);
        assert_eq!(clamped_right.x, 1000 - 64);

        // Above the screen: titlebar (y) clamped to usable.y (0)
        let above = WindowGeometry::new(100, -200, 400, 300);
        let clamped_above = clamp_to_reachability(above, usable);
        assert_eq!(clamped_above.y, 0);

        // Below the screen: titlebar clamped so 32px is on screen
        let below = WindowGeometry::new(100, 1000, 400, 300);
        let clamped_below = clamp_to_reachability(below, usable);
        assert_eq!(clamped_below.y, 800 - 32);
    }

    #[test]
    fn test_center_window() {
        let usable = WindowGeometry::new(100, 100, 1000, 800);
        let centered = center_window((400, 200), usable);
        assert_eq!(centered.x, 100 + (1000 - 400) / 2);
        assert_eq!(centered.y, 100 + (800 - 200) / 2);
        assert_eq!(centered.width, 400);
        assert_eq!(centered.height, 200);
    }

    #[test]
    fn test_floating_layout_arrange() {
        let output = WindowGeometry::new(0, 0, 1920, 1080);
        let usable = WindowGeometry::new(0, 32, 1920, 1048);

        let mut map = SlotMap::with_key();
        let w1: WindowId = map.insert(());
        let w2: WindowId = map.insert(());
        let w3: WindowId = map.insert(());

        let mut win1 = WindowData::new(
            w1,
            WindowGeometry::new(100, 100, 500, 400),
            SizeConstraints::default(),
            "win1",
            "app1",
        );
        let mut win2 = WindowData::new(
            w2,
            WindowGeometry::new(100, 100, 500, 400),
            SizeConstraints::default(),
            "win2",
            "app2",
        );
        win2.state = WindowState::Maximized;

        let mut win3 = WindowData::new(
            w3,
            WindowGeometry::new(100, 100, 500, 400),
            SizeConstraints::default(),
            "win3",
            "app3",
        );
        win3.state = WindowState::Fullscreen;

        let layout = FloatingLayout;
        let mut items = [(&w1, &mut win1), (&w2, &mut win2), (&w3, &mut win3)];
        layout.arrange(output, usable, &mut items);

        // win1 normal remains reachable
        assert_eq!(win1.geometry, WindowGeometry::new(100, 100, 500, 400));
        // win2 maximized matches usable area
        assert_eq!(win2.geometry, usable);
        // win3 fullscreen matches output geometry
        assert_eq!(win3.geometry, output);
    }
}

// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Window decoration geometry, button layout, and hit testing.
//!
//! Pure-logic implementation of ADR-0026 titlebar geometry and window border
//! edge detection for interactive resizing and window controls.

use crate::WindowGeometry;

/// Height of server-side titlebar in logical pixels.
pub const TITLEBAR_HEIGHT: i32 = 28;

/// Width and height of titlebar control buttons in logical pixels.
pub const BUTTON_SIZE: i32 = 12;

/// Margin from the top of the titlebar to control buttons.
pub const BUTTON_TOP_MARGIN: i32 = 8;

/// Margin from the right edge for the close button.
pub const CLOSE_BUTTON_RIGHT_MARGIN: i32 = 12;

/// Margin from the right edge for the maximize button.
pub const MAXIMIZE_BUTTON_RIGHT_MARGIN: i32 = 32;

/// Margin from the right edge for the minimize button.
pub const MINIMIZE_BUTTON_RIGHT_MARGIN: i32 = 52;

/// Interactive border margin in logical pixels for edge resizing.
pub const BORDER_MARGIN: i32 = 8;

/// Standard RGB/RGBA color constants for titlebar buttons (ADR-0026).
pub mod colors {
    /// Solid close button color: Red (#FF5F56).
    pub const CLOSE_BUTTON: [f32; 4] = [1.0, 0.3725, 0.3373, 1.0];
    /// Solid maximize button color: Green (#27C93F).
    pub const MAXIMIZE_BUTTON: [f32; 4] = [0.1529, 0.7882, 0.2471, 1.0];
    /// Solid minimize button color: Yellow (#FFBD2E).
    pub const MINIMIZE_BUTTON: [f32; 4] = [1.0, 0.7412, 0.1804, 1.0];
    /// Solid active titlebar background color: Dark Slate (#2A2C33).
    pub const TITLEBAR_ACTIVE: [f32; 4] = [0.1647, 0.1725, 0.2000, 1.0];
    /// Solid inactive titlebar background color: Muted Slate (#1E2024).
    pub const TITLEBAR_INACTIVE: [f32; 4] = [0.1176, 0.1255, 0.1412, 1.0];
}

/// 8-way resize direction for window borders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResizeEdge {
    /// Top horizontal border.
    Top,
    /// Bottom horizontal border.
    Bottom,
    /// Left vertical border.
    Left,
    /// Right vertical border.
    Right,
    /// Top-left corner.
    TopLeft,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-right corner.
    BottomRight,
}

/// Hit test target for window decoration regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationHitTest {
    /// Clicked on close button (Red).
    CloseButton,
    /// Clicked on maximize/restore button (Green).
    MaximizeButton,
    /// Clicked on minimize button (Yellow).
    MinimizeButton,
    /// Clicked on titlebar background (drag to move, double-click to maximize).
    Titlebar,
    /// Clicked or hovering over an 8px resize border edge.
    ResizeBorder(ResizeEdge),
    /// Inside window content area.
    Content,
    /// Outside window boundaries.
    Outside,
}

/// Computed geometry of window decoration elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecorationGeometry {
    /// Titlebar rectangle in logical space.
    pub titlebar: WindowGeometry,
    /// Close button rectangle in logical space.
    pub close_button: WindowGeometry,
    /// Maximize button rectangle in logical space.
    pub maximize_button: WindowGeometry,
    /// Minimize button rectangle in logical space.
    pub minimize_button: WindowGeometry,
}

impl DecorationGeometry {
    /// Computes decoration layout rectangles from window content geometry.
    #[must_use]
    pub fn for_window(window_geo: WindowGeometry) -> Self {
        let titlebar_y = window_geo.y.saturating_sub(TITLEBAR_HEIGHT);
        let titlebar =
            WindowGeometry::new(window_geo.x, titlebar_y, window_geo.width, TITLEBAR_HEIGHT);

        let button_y = titlebar_y.saturating_add(BUTTON_TOP_MARGIN);

        let close_x = window_geo
            .x
            .saturating_add(window_geo.width)
            .saturating_sub(CLOSE_BUTTON_RIGHT_MARGIN)
            .saturating_sub(BUTTON_SIZE);
        let close_button = WindowGeometry::new(close_x, button_y, BUTTON_SIZE, BUTTON_SIZE);

        let max_x = window_geo
            .x
            .saturating_add(window_geo.width)
            .saturating_sub(MAXIMIZE_BUTTON_RIGHT_MARGIN)
            .saturating_sub(BUTTON_SIZE);
        let maximize_button = WindowGeometry::new(max_x, button_y, BUTTON_SIZE, BUTTON_SIZE);

        let min_x = window_geo
            .x
            .saturating_add(window_geo.width)
            .saturating_sub(MINIMIZE_BUTTON_RIGHT_MARGIN)
            .saturating_sub(BUTTON_SIZE);
        let minimize_button = WindowGeometry::new(min_x, button_y, BUTTON_SIZE, BUTTON_SIZE);

        Self {
            titlebar,
            close_button,
            maximize_button,
            minimize_button,
        }
    }
}

/// Detects if a coordinate is within the border margin of a window rectangle.
///
/// Checks a margin band of `margin` pixels both inside and outside the window edges.
#[must_use]
pub fn detect_border_edge(
    px: i32,
    py: i32,
    geo: WindowGeometry,
    margin: i32,
) -> Option<ResizeEdge> {
    let margin = margin.max(1);
    let outer_left = geo.x.saturating_sub(margin);
    let outer_top = geo.y.saturating_sub(margin);
    let outer_right = geo.x.saturating_add(geo.width).saturating_add(margin);
    let outer_bottom = geo.y.saturating_add(geo.height).saturating_add(margin);

    if px < outer_left || px >= outer_right || py < outer_top || py >= outer_bottom {
        return None;
    }

    let is_left = px < geo.x.saturating_add(margin);
    let is_right = px >= geo.x.saturating_add(geo.width).saturating_sub(margin);
    let is_top = py < geo.y.saturating_add(margin);
    let is_bottom = py >= geo.y.saturating_add(geo.height).saturating_sub(margin);

    match (is_top, is_bottom, is_left, is_right) {
        (true, _, true, _) => Some(ResizeEdge::TopLeft),
        (true, _, _, true) => Some(ResizeEdge::TopRight),
        (true, false, false, false) => Some(ResizeEdge::Top),
        (_, true, true, _) => Some(ResizeEdge::BottomLeft),
        (_, true, _, true) => Some(ResizeEdge::BottomRight),
        (false, true, false, false) => Some(ResizeEdge::Bottom),
        (false, false, true, false) => Some(ResizeEdge::Left),
        (false, false, false, true) => Some(ResizeEdge::Right),
        _ => None,
    }
}

/// Performs hit testing against a window and its optional server-side decorations.
#[must_use]
pub fn hit_test_window(
    px: i32,
    py: i32,
    geo: WindowGeometry,
    has_ssd: bool,
    margin: i32,
) -> DecorationHitTest {
    if has_ssd {
        let dec = DecorationGeometry::for_window(geo);
        if dec.close_button.contains(px, py) {
            return DecorationHitTest::CloseButton;
        }
        if dec.maximize_button.contains(px, py) {
            return DecorationHitTest::MaximizeButton;
        }
        if dec.minimize_button.contains(px, py) {
            return DecorationHitTest::MinimizeButton;
        }
        if dec.titlebar.contains(px, py) {
            return DecorationHitTest::Titlebar;
        }
    }

    if let Some(edge) = detect_border_edge(px, py, geo, margin) {
        return DecorationHitTest::ResizeBorder(edge);
    }

    if geo.contains(px, py) {
        DecorationHitTest::Content
    } else {
        DecorationHitTest::Outside
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoration_geometry_calculation() {
        let geo = WindowGeometry::new(100, 100, 400, 300);
        let dec = DecorationGeometry::for_window(geo);

        assert_eq!(dec.titlebar.x, 100);
        assert_eq!(dec.titlebar.y, 72);
        assert_eq!(dec.titlebar.width, 400);
        assert_eq!(dec.titlebar.height, 28);

        // Buttons must be inside titlebar
        assert!(
            dec.titlebar
                .contains(dec.close_button.x, dec.close_button.y)
        );
        assert!(
            dec.titlebar
                .contains(dec.maximize_button.x, dec.maximize_button.y)
        );
        assert!(
            dec.titlebar
                .contains(dec.minimize_button.x, dec.minimize_button.y)
        );

        // Buttons must have non-overlapping X order: minimize < maximize < close
        assert!(dec.minimize_button.x < dec.maximize_button.x);
        assert!(dec.maximize_button.x < dec.close_button.x);
    }

    #[test]
    fn test_detect_border_edge_all_directions() {
        let geo = WindowGeometry::new(100, 100, 200, 200);
        let m = 8;

        // Corners
        assert_eq!(
            detect_border_edge(96, 96, geo, m),
            Some(ResizeEdge::TopLeft)
        );
        assert_eq!(
            detect_border_edge(304, 96, geo, m),
            Some(ResizeEdge::TopRight)
        );
        assert_eq!(
            detect_border_edge(96, 304, geo, m),
            Some(ResizeEdge::BottomLeft)
        );
        assert_eq!(
            detect_border_edge(304, 304, geo, m),
            Some(ResizeEdge::BottomRight)
        );

        // Edges
        assert_eq!(detect_border_edge(200, 96, geo, m), Some(ResizeEdge::Top));
        assert_eq!(
            detect_border_edge(200, 304, geo, m),
            Some(ResizeEdge::Bottom)
        );
        assert_eq!(detect_border_edge(96, 200, geo, m), Some(ResizeEdge::Left));
        assert_eq!(
            detect_border_edge(304, 200, geo, m),
            Some(ResizeEdge::Right)
        );

        // Center is content (None)
        assert_eq!(detect_border_edge(200, 200, geo, m), None);

        // Far outside is None
        assert_eq!(detect_border_edge(50, 200, geo, m), None);
    }

    #[test]
    fn test_hit_test_window_ssd_and_buttons() {
        let geo = WindowGeometry::new(100, 100, 400, 300);
        let dec = DecorationGeometry::for_window(geo);

        // Close button hit
        let hit = hit_test_window(dec.close_button.x + 2, dec.close_button.y + 2, geo, true, 8);
        assert_eq!(hit, DecorationHitTest::CloseButton);

        // Maximize button hit
        let hit = hit_test_window(
            dec.maximize_button.x + 2,
            dec.maximize_button.y + 2,
            geo,
            true,
            8,
        );
        assert_eq!(hit, DecorationHitTest::MaximizeButton);

        // Minimize button hit
        let hit = hit_test_window(
            dec.minimize_button.x + 2,
            dec.minimize_button.y + 2,
            geo,
            true,
            8,
        );
        assert_eq!(hit, DecorationHitTest::MinimizeButton);

        // Titlebar body hit
        let hit = hit_test_window(120, 80, geo, true, 8);
        assert_eq!(hit, DecorationHitTest::Titlebar);

        // Content hit
        let hit = hit_test_window(200, 200, geo, true, 8);
        assert_eq!(hit, DecorationHitTest::Content);
    }
}

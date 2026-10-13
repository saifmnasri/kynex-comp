// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Geometry calculations and edge detection for interactive window resize.

use kx_layout::window::SizeConstraints;
use kx_layout::{WindowGeometry, clamp_to_reachability};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Point};

/// Detects which edge of the window was clicked based on a 3x3 grid division.
#[must_use]
pub fn detect_resize_edge(
    loc: Point<f64, Logical>,
    geo: WindowGeometry,
) -> xdg_toplevel::ResizeEdge {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "pointer logical coordinate fits in i32"
    )]
    let rel_x = (loc.x as i32).saturating_sub(geo.x);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "pointer logical coordinate fits in i32"
    )]
    let rel_y = (loc.y as i32).saturating_sub(geo.y);

    let w_third = (geo.width / 3).max(1);
    let h_third = (geo.height / 3).max(1);

    let left = rel_x < w_third;
    let right = rel_x >= geo.width.saturating_sub(w_third);
    let top = rel_y < h_third;
    let bottom = rel_y >= geo.height.saturating_sub(h_third);

    match (top, bottom, left, right) {
        (true, _, true, _) => xdg_toplevel::ResizeEdge::TopLeft,
        (true, _, _, true) => xdg_toplevel::ResizeEdge::TopRight,
        (true, _, false, false) => xdg_toplevel::ResizeEdge::Top,
        (_, true, true, _) => xdg_toplevel::ResizeEdge::BottomLeft,
        (_, true, _, true) | (false, false, false, false) => xdg_toplevel::ResizeEdge::BottomRight,
        (_, true, false, false) => xdg_toplevel::ResizeEdge::Bottom,
        (false, false, true, _) => xdg_toplevel::ResizeEdge::Left,
        (false, false, _, true) => xdg_toplevel::ResizeEdge::Right,
    }
}

/// Applies edge motion deltas to produce candidate width and height.
#[must_use]
pub fn apply_edge_deltas(
    initial_geo: WindowGeometry,
    edges: xdg_toplevel::ResizeEdge,
    dx: i32,
    dy: i32,
) -> (i32, i32) {
    let candidate_w = match edges {
        xdg_toplevel::ResizeEdge::Left
        | xdg_toplevel::ResizeEdge::TopLeft
        | xdg_toplevel::ResizeEdge::BottomLeft => initial_geo.width.saturating_sub(dx),
        xdg_toplevel::ResizeEdge::Right
        | xdg_toplevel::ResizeEdge::TopRight
        | xdg_toplevel::ResizeEdge::BottomRight => initial_geo.width.saturating_add(dx),
        _ => initial_geo.width,
    };

    let candidate_h = match edges {
        xdg_toplevel::ResizeEdge::Top
        | xdg_toplevel::ResizeEdge::TopLeft
        | xdg_toplevel::ResizeEdge::TopRight => initial_geo.height.saturating_sub(dy),
        xdg_toplevel::ResizeEdge::Bottom
        | xdg_toplevel::ResizeEdge::BottomLeft
        | xdg_toplevel::ResizeEdge::BottomRight => initial_geo.height.saturating_add(dy),
        _ => initial_geo.height,
    };

    (candidate_w, candidate_h)
}

/// Computes candidate top-left position when resizing from left or top edges.
#[must_use]
pub fn adjust_top_left_position(
    initial_geo: WindowGeometry,
    edges: xdg_toplevel::ResizeEdge,
    constrained_w: i32,
    constrained_h: i32,
) -> (i32, i32) {
    let candidate_x = match edges {
        xdg_toplevel::ResizeEdge::Left
        | xdg_toplevel::ResizeEdge::TopLeft
        | xdg_toplevel::ResizeEdge::BottomLeft => initial_geo
            .x
            .saturating_add(initial_geo.width.saturating_sub(constrained_w)),
        _ => initial_geo.x,
    };

    let candidate_y = match edges {
        xdg_toplevel::ResizeEdge::Top
        | xdg_toplevel::ResizeEdge::TopLeft
        | xdg_toplevel::ResizeEdge::TopRight => initial_geo
            .y
            .saturating_add(initial_geo.height.saturating_sub(constrained_h)),
        _ => initial_geo.y,
    };

    (candidate_x, candidate_y)
}

/// Calculates the target window geometry after applying edge delta, constraints, and reachability.
#[must_use]
pub fn calculate_resize_geometry(
    initial_geo: WindowGeometry,
    constraints: &SizeConstraints,
    edges: xdg_toplevel::ResizeEdge,
    dx: i32,
    dy: i32,
    usable_area: WindowGeometry,
) -> WindowGeometry {
    let (candidate_w, candidate_h) = apply_edge_deltas(initial_geo, edges, dx, dy);
    let (constrained_w, constrained_h) = constraints.constrain(candidate_w, candidate_h);
    let (candidate_x, candidate_y) =
        adjust_top_left_position(initial_geo, edges, constrained_w, constrained_h);

    let unconstrained = WindowGeometry::new(candidate_x, candidate_y, constrained_w, constrained_h);
    clamp_to_reachability(unconstrained, usable_area)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_resize_edge_corners_and_edges() {
        let geo = WindowGeometry::new(100, 100, 300, 300);

        // TopLeft
        let edge = detect_resize_edge(Point::from((110.0, 110.0)), geo);
        assert_eq!(edge, xdg_toplevel::ResizeEdge::TopLeft);

        // TopRight
        let edge = detect_resize_edge(Point::from((390.0, 110.0)), geo);
        assert_eq!(edge, xdg_toplevel::ResizeEdge::TopRight);

        // BottomLeft
        let edge = detect_resize_edge(Point::from((110.0, 390.0)), geo);
        assert_eq!(edge, xdg_toplevel::ResizeEdge::BottomLeft);

        // BottomRight
        let edge = detect_resize_edge(Point::from((390.0, 390.0)), geo);
        assert_eq!(edge, xdg_toplevel::ResizeEdge::BottomRight);

        // Top
        let edge = detect_resize_edge(Point::from((250.0, 110.0)), geo);
        assert_eq!(edge, xdg_toplevel::ResizeEdge::Top);

        // Bottom
        let edge = detect_resize_edge(Point::from((250.0, 390.0)), geo);
        assert_eq!(edge, xdg_toplevel::ResizeEdge::Bottom);

        // Left
        let edge = detect_resize_edge(Point::from((110.0, 250.0)), geo);
        assert_eq!(edge, xdg_toplevel::ResizeEdge::Left);

        // Right
        let edge = detect_resize_edge(Point::from((390.0, 250.0)), geo);
        assert_eq!(edge, xdg_toplevel::ResizeEdge::Right);
    }

    #[test]
    fn test_calculate_resize_geometry_right_bottom() {
        let initial = WindowGeometry::new(100, 100, 400, 300);
        let constraints = SizeConstraints::default();
        let usable = WindowGeometry::new(0, 0, 1920, 1080);

        let geo = calculate_resize_geometry(
            initial,
            &constraints,
            xdg_toplevel::ResizeEdge::BottomRight,
            50,
            30,
            usable,
        );

        assert_eq!(geo.x, 100);
        assert_eq!(geo.y, 100);
        assert_eq!(geo.width, 450);
        assert_eq!(geo.height, 330);
    }

    #[test]
    fn test_calculate_resize_geometry_left_top_constraints() {
        let initial = WindowGeometry::new(200, 200, 400, 300);
        let constraints = SizeConstraints {
            min_width: 200,
            min_height: 150,
            ..Default::default()
        };
        let usable = WindowGeometry::new(0, 0, 1920, 1080);

        // Shrink towards min width & height from TopLeft
        let geo = calculate_resize_geometry(
            initial,
            &constraints,
            xdg_toplevel::ResizeEdge::TopLeft,
            500, // excessively large shrink
            500,
            usable,
        );

        // Clamped at min_width 200, so width is 200, and x moves right by 400 - 200 = 200 -> 400
        assert_eq!(geo.width, 200);
        assert_eq!(geo.height, 150);
        assert_eq!(geo.x, 400);
        assert_eq!(geo.y, 350);
    }
}

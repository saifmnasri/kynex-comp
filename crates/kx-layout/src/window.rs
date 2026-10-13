// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Window identity, state, geometry constraints, and metadata for `KynexComp`.

use slotmap::new_key_type;

new_key_type! {
    /// O(1) typed identifier for a managed window.
    pub struct WindowId;
    /// O(1) typed identifier for an output display.
    pub struct OutputId;
}

/// Hard limit on string lengths from Wayland clients (Rule 05 anti-DoS).
pub const MAX_STRING_BYTES: usize = 4096;

/// Visual and operational state of a managed window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WindowState {
    /// Normal floating window.
    #[default]
    Normal,
    /// Maximized to the usable area of the output.
    Maximized,
    /// Fullscreen covering the entire output display mode.
    Fullscreen,
    /// Minimized (hidden from view, accessible in Alt+Tab and recovery).
    Minimized,
}

/// Logical rectangle defining the position and size of a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WindowGeometry {
    /// Top-left X coordinate in logical space.
    pub x: i32,
    /// Top-left Y coordinate in logical space.
    pub y: i32,
    /// Width in logical pixels (always $\ge 1$).
    pub width: i32,
    /// Height in logical pixels (always $\ge 1$).
    pub height: i32,
}

impl WindowGeometry {
    /// Creates a new window geometry, ensuring width and height are at least 1.
    #[must_use]
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width: width.max(1),
            height: height.max(1),
        }
    }

    /// Checks whether the point `(px, py)` is inside this geometry rectangle.
    #[must_use]
    pub fn contains(&self, px: i32, py: i32) -> bool {
        let x_end = self.x.saturating_add(self.width);
        let y_end = self.y.saturating_add(self.height);
        px >= self.x && px < x_end && py >= self.y && py < y_end
    }
}

/// Size constraints requested by a Wayland client via `xdg_toplevel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SizeConstraints {
    /// Minimum allowed width in logical pixels.
    pub min_width: i32,
    /// Minimum allowed height in logical pixels.
    pub min_height: i32,
    /// Maximum allowed width in logical pixels, if specified.
    pub max_width: Option<i32>,
    /// Maximum allowed height in logical pixels, if specified.
    pub max_height: Option<i32>,
}

impl SizeConstraints {
    /// Constrains the requested `(width, height)` according to client bounds.
    ///
    /// Always returns dimensions of at least 1 logical pixel and uses saturating
    /// arithmetic to defend against overflow or conflicting constraints.
    #[must_use]
    pub fn constrain(&self, width: i32, height: i32) -> (i32, i32) {
        let min_w = self.min_width.max(1);
        let min_h = self.min_height.max(1);

        let mut clamped_w = width.max(min_w);
        if let Some(max_w) = self.max_width {
            let max_w = max_w.max(min_w);
            clamped_w = clamped_w.min(max_w);
        }

        let mut clamped_h = height.max(min_h);
        if let Some(max_h) = self.max_height {
            let max_h = max_h.max(min_h);
            clamped_h = clamped_h.min(max_h);
        }

        (clamped_w, clamped_h)
    }
}

/// Truncates a UTF-8 string to `max_bytes` without slicing through a UTF-8 character boundary.
#[must_use]
pub fn truncate_utf8(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_string();
    }

    let mut boundary = max_bytes;
    while boundary > 0 && !input.is_char_boundary(boundary) {
        boundary = boundary.saturating_sub(1);
    }

    input.get(..boundary).unwrap_or_default().to_string()
}

/// Core model of a managed client window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowData {
    /// Unique typed window identifier.
    pub id: WindowId,
    /// Current operational state.
    pub state: WindowState,
    /// Current active geometry in logical coordinates.
    pub geometry: WindowGeometry,
    /// Saved geometry prior to maximizing or entering fullscreen.
    pub saved_geometry: Option<WindowGeometry>,
    /// Client-declared size constraints.
    pub constraints: SizeConstraints,
    /// Window title, safely truncated to 4 KiB.
    pub title: String,
    /// Application ID, safely truncated to 4 KiB.
    pub app_id: String,
    /// Parent window ID, if this window is a transient dialog or child toplevel.
    pub parent: Option<WindowId>,
    /// Whether this window has committed a buffer and is mapped.
    pub is_mapped: bool,
    /// Output displaying this window, if assigned.
    pub output_id: Option<OutputId>,
}

impl WindowData {
    /// Creates a new window model with validated constraints and truncated strings.
    #[must_use]
    pub fn new(
        id: WindowId,
        geometry: WindowGeometry,
        constraints: SizeConstraints,
        title: &str,
        app_id: &str,
    ) -> Self {
        let (w, h) = constraints.constrain(geometry.width, geometry.height);
        Self {
            id,
            state: WindowState::Normal,
            geometry: WindowGeometry::new(geometry.x, geometry.y, w, h),
            saved_geometry: None,
            constraints,
            title: truncate_utf8(title, MAX_STRING_BYTES),
            app_id: truncate_utf8(app_id, MAX_STRING_BYTES),
            parent: None,
            is_mapped: false,
            output_id: None,
        }
    }

    /// Sets the window title, truncating at char boundaries up to 4 KiB.
    pub fn set_title(&mut self, title: &str) {
        self.title = truncate_utf8(title, MAX_STRING_BYTES);
    }

    /// Sets the window application ID, truncating up to 4 KiB.
    pub fn set_app_id(&mut self, app_id: &str) {
        self.app_id = truncate_utf8(app_id, MAX_STRING_BYTES);
    }
}

/// Parameters for mapping a new window into [`WindowManager`](crate::manager::WindowManager).
#[derive(Debug, Clone)]
pub struct MapWindowParams<'a> {
    /// Desired initial geometry (x=0, y=0 triggers cascade positioning).
    pub geometry: WindowGeometry,
    /// Client-declared size constraints.
    pub constraints: SizeConstraints,
    /// Window title string.
    pub title: &'a str,
    /// Application identifier.
    pub app_id: &'a str,
    /// Target workspace ID (defaults to active workspace if `None`).
    pub target_workspace: Option<crate::workspace::WorkspaceId>,
    /// Usable area on the target display.
    pub usable_area: WindowGeometry,
    /// Whether configuration allows new windows to steal focus.
    pub focus_new_windows_cfg: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_constraints_clamping() {
        let constraints = SizeConstraints {
            min_width: 200,
            min_height: 150,
            max_width: Some(800),
            max_height: Some(600),
        };

        // Below minimum
        assert_eq!(constraints.constrain(50, 50), (200, 150));
        // Above maximum
        assert_eq!(constraints.constrain(1000, 900), (800, 600));
        // Within range
        assert_eq!(constraints.constrain(500, 400), (500, 400));
    }

    #[test]
    fn test_size_constraints_min_greater_than_max() {
        let hostile = SizeConstraints {
            min_width: 500,
            min_height: 500,
            max_width: Some(100),
            max_height: Some(100),
        };
        // Defensive: min takes precedence over max if client provides contradictory values
        assert_eq!(hostile.constrain(300, 300), (500, 500));
    }

    #[test]
    fn test_truncate_utf8_ascii_and_multibyte() {
        let text = "Hello, world!";
        assert_eq!(truncate_utf8(text, 5), "Hello");
        assert_eq!(truncate_utf8(text, 50), "Hello, world!");

        // Multibyte: 4-byte UTF-8 emoji
        let emoji = "🦀🦀🦀"; // each crab is 4 bytes (total 12 bytes)
        assert_eq!(truncate_utf8(emoji, 4), "🦀");
        // Boundary sliced inside second crab (e.g. 5, 6, 7 bytes) safely drops back to 4 bytes
        assert_eq!(truncate_utf8(emoji, 6), "🦀");
        assert_eq!(truncate_utf8(emoji, 7), "🦀");
        assert_eq!(truncate_utf8(emoji, 8), "🦀🦀");
    }

    #[test]
    fn test_window_geometry_contains() {
        let geom = WindowGeometry::new(10, 20, 100, 50);
        assert!(geom.contains(10, 20));
        assert!(geom.contains(50, 40));
        assert!(geom.contains(109, 69));
        assert!(!geom.contains(110, 69));
        assert!(!geom.contains(50, 70));
        assert!(!geom.contains(9, 20));
    }
}

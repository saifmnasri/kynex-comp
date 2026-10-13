// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Window layout engines, geometry, workspaces, and focus management for `KynexComp`.
//!
//! This is a pure-logic crate with zero dependencies on Smithay, Wayland, Calloop,
//! or GPU/rendering crates (per Rule 02). All policy decisions, layout computations,
//! focus stacking, and reachability validation live here for deterministic unit and property testing.

pub mod engine;
pub mod focus;
pub mod manager;
pub mod window;
pub mod workspace;

pub use engine::{
    CASCADE_OFFSET_STEP, CascadeTracker, ExclusiveMargins, FloatingLayout, LayoutEngine,
    MIN_ONSCREEN_HEIGHT, MIN_ONSCREEN_WIDTH, center_window, clamp_to_reachability,
    fullscreen_geometry, maximized_geometry,
};
pub use focus::{AltTabSession, FocusPolicy, FocusStack, StackingOrder, should_focus_new_window};
pub use manager::{MapWindowParams, WindowManager};
pub use window::{
    MAX_STRING_BYTES, OutputId, SizeConstraints, WindowData, WindowGeometry, WindowId, WindowState,
    truncate_utf8,
};
pub use workspace::{
    DEFAULT_WORKSPACE_COUNT, MAX_WINDOWS_PER_WORKSPACE, MAX_WORKSPACES, Workspace, WorkspaceError,
    WorkspaceId, WorkspaceManager,
};

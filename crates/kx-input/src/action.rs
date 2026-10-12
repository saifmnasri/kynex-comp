// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Compositor action definitions for input events.

/// Actions that can be triggered by keyboard shortcuts or input events.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CompositorAction {
    /// Request clean compositor shutdown.
    Quit,
    /// Terminate or disconnect the focused client.
    KillClient,
    /// Switch to a workspace by 1-based index.
    SwitchWorkspace(u32),
    /// Move the currently focused window to a workspace by 1-based index.
    MoveToWorkspace(u32),
    /// Toggle fullscreen state on the focused window.
    ToggleFullscreen,
    /// Request graceful close of the focused window.
    CloseWindow,
    /// Custom named action defined in configuration or scripts.
    Custom(String),
}

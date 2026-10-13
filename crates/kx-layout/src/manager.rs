// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Authoritative window manager coordinating windows, workspaces, focus, and layout.

use slotmap::SlotMap;

use crate::engine::{
    CascadeTracker, FloatingLayout, LayoutEngine, center_window, clamp_to_reachability,
};
use crate::focus::{FocusStack, StackingOrder, should_focus_new_window};
use crate::window::{SizeConstraints, WindowData, WindowGeometry, WindowId, WindowState};
use crate::workspace::{WorkspaceError, WorkspaceId, WorkspaceManager};

/// Parameters for mapping a new window into [`WindowManager`].
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
    pub target_workspace: Option<WorkspaceId>,
    /// Usable area on the target display.
    pub usable_area: WindowGeometry,
    /// Whether configuration allows new windows to steal focus.
    pub focus_new_windows_cfg: bool,
}

/// Central window manager uniting window entities, workspaces, focus stacks, and layout.
pub struct WindowManager {
    windows: SlotMap<WindowId, WindowData>,
    workspaces: WorkspaceManager,
    focus: FocusStack,
    stacking: StackingOrder,
    cascade: CascadeTracker,
    layout: Box<dyn LayoutEngine>,
}

impl std::fmt::Debug for WindowManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowManager")
            .field("windows_count", &self.windows.len())
            .field("workspaces", &self.workspaces)
            .field("focus", &self.focus)
            .field("stacking", &self.stacking)
            .finish_non_exhaustive()
    }
}

impl WindowManager {
    /// Creates a new window manager with `workspace_count` initial workspaces.
    #[must_use]
    pub fn new(workspace_count: usize) -> Self {
        Self {
            windows: SlotMap::with_key(),
            workspaces: WorkspaceManager::new(workspace_count),
            focus: FocusStack::new(),
            stacking: StackingOrder::new(),
            cascade: CascadeTracker::new(),
            layout: Box::new(FloatingLayout),
        }
    }

    /// Returns a reference to all managed windows.
    #[must_use]
    pub fn windows(&self) -> &SlotMap<WindowId, WindowData> {
        &self.windows
    }

    /// Returns a reference to a window by ID.
    #[must_use]
    pub fn window(&self, id: WindowId) -> Option<&WindowData> {
        self.windows.get(id)
    }

    /// Returns a mutable reference to a window by ID.
    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut WindowData> {
        self.windows.get_mut(id)
    }

    /// Returns a reference to the workspace manager.
    #[must_use]
    pub fn workspaces(&self) -> &WorkspaceManager {
        &self.workspaces
    }

    /// Returns a mutable reference to the workspace manager.
    pub fn workspaces_mut(&mut self) -> &mut WorkspaceManager {
        &mut self.workspaces
    }

    /// Returns a reference to the focus stack.
    #[must_use]
    pub fn focus(&self) -> &FocusStack {
        &self.focus
    }

    /// Returns a reference to the stacking order.
    #[must_use]
    pub fn stacking(&self) -> &StackingOrder {
        &self.stacking
    }

    /// Maps a new window into the window manager.
    ///
    /// # Errors
    /// Returns [`WorkspaceError`] if the target workspace does not exist or has reached capacity.
    pub fn map_window(&mut self, params: &MapWindowParams<'_>) -> Result<WindowId, WorkspaceError> {
        let target_ws = params
            .target_workspace
            .unwrap_or_else(|| self.workspaces.active_workspace());

        let initial_geo = if params.geometry.x == 0 && params.geometry.y == 0 {
            self.cascade.next_position(
                (params.geometry.width, params.geometry.height),
                params.usable_area,
            )
        } else {
            clamp_to_reachability(params.geometry, params.usable_area)
        };

        let wid = self.windows.insert_with_key(|id| {
            let mut data = WindowData::new(
                id,
                initial_geo,
                params.constraints,
                params.title,
                params.app_id,
            );
            data.is_mapped = true;
            data
        });

        if let Err(err) = self.workspaces.add_window(target_ws, wid) {
            self.windows.remove(wid);
            return Err(err);
        }

        self.stacking.insert_top(wid);

        if should_focus_new_window(self.focus.focused(), params.focus_new_windows_cfg) {
            self.focus.push_focused(wid);
        }

        Ok(wid)
    }

    /// Unmaps and removes a window completely.
    pub fn unmap_window(&mut self, wid: WindowId) -> Option<WindowData> {
        self.workspaces.remove_window(wid);
        self.stacking.remove(wid);
        self.focus.remove(wid);
        self.windows.remove(wid)
    }

    /// Grants focus to `wid` and raises it to top of the stacking order.
    ///
    /// Returns `(previous_focus, new_focus)`.
    pub fn focus_window(&mut self, wid: WindowId) -> (Option<WindowId>, Option<WindowId>) {
        if !self.windows.contains_key(wid) {
            return (self.focus.focused(), self.focus.focused());
        }

        let prev = self.focus.focused();
        self.focus.push_focused(wid);
        self.stacking.raise(wid);
        (prev, Some(wid))
    }

    /// Maximizes `wid`, recording its previous floating geometry.
    pub fn maximize_window(&mut self, wid: WindowId, usable_area: WindowGeometry) {
        if let Some(win) = self.windows.get_mut(wid) {
            if win.state != WindowState::Maximized {
                win.saved_geometry = Some(win.geometry);
                win.state = WindowState::Maximized;
                win.geometry = usable_area;
            }
        }
    }

    /// Restores `wid` from maximized state to its saved geometry.
    pub fn unmaximize_window(&mut self, wid: WindowId, usable_area: WindowGeometry) {
        if let Some(win) = self.windows.get_mut(wid) {
            if win.state == WindowState::Maximized {
                win.state = WindowState::Normal;
                let restored = win.saved_geometry.take().unwrap_or(win.geometry);
                win.geometry = clamp_to_reachability(restored, usable_area);
            }
        }
    }

    /// Sets `wid` to fullscreen covering `output_geo`.
    pub fn fullscreen_window(&mut self, wid: WindowId, output_geo: WindowGeometry) {
        if let Some(win) = self.windows.get_mut(wid) {
            if win.state != WindowState::Fullscreen {
                if win.state != WindowState::Maximized {
                    win.saved_geometry = Some(win.geometry);
                }
                win.state = WindowState::Fullscreen;
                win.geometry = output_geo;
            }
        }
    }

    /// Restores `wid` from fullscreen state.
    pub fn unfullscreen_window(&mut self, wid: WindowId, usable_area: WindowGeometry) {
        if let Some(win) = self.windows.get_mut(wid) {
            if win.state == WindowState::Fullscreen {
                win.state = WindowState::Normal;
                let restored = win.saved_geometry.take().unwrap_or(win.geometry);
                win.geometry = clamp_to_reachability(restored, usable_area);
            }
        }
    }

    /// Minimizes `wid` and automatically shifts focus to the next active window.
    pub fn minimize_window(&mut self, wid: WindowId) {
        if let Some(win) = self.windows.get_mut(wid) {
            if win.state != WindowState::Minimized {
                win.state = WindowState::Minimized;
            }
        } else {
            return;
        }

        // If the minimized window held focus, switch to the next eligible window
        if self.focus.focused() == Some(wid) {
            let active_ws = self.workspaces.active_workspace();
            let next_candidate = self.focus.mru_order().iter().copied().find(|&w| {
                w != wid
                    && self.workspaces.find_window(w) == Some(active_ws)
                    && self
                        .windows
                        .get(w)
                        .is_some_and(|wdata| wdata.state != WindowState::Minimized)
            });

            if let Some(next_wid) = next_candidate {
                self.focus.push_focused(next_wid);
                self.stacking.raise(next_wid);
            } else {
                self.focus.remove(wid);
            }
        }
    }

    /// Restores `wid` from minimized state, focusing and raising it.
    pub fn unminimize_window(&mut self, wid: WindowId, usable_area: WindowGeometry) {
        if let Some(win) = self.windows.get_mut(wid) {
            if win.state == WindowState::Minimized {
                win.state = WindowState::Normal;
                win.geometry = clamp_to_reachability(win.geometry, usable_area);
            }
        }
        self.focus.push_focused(wid);
        self.stacking.raise(wid);
    }

    /// Moves a window to another workspace.
    ///
    /// # Errors
    /// Returns [`WorkspaceError`] if the target workspace does not exist or is full.
    pub fn move_to_workspace(
        &mut self,
        wid: WindowId,
        target_ws: WorkspaceId,
    ) -> Result<(), WorkspaceError> {
        self.workspaces.move_window(wid, target_ws)
    }

    /// Centers `wid` within the usable area (Super+C recovery shortcut).
    pub fn center_window(&mut self, wid: WindowId, usable_area: WindowGeometry) {
        if let Some(win) = self.windows.get_mut(wid) {
            win.geometry = center_window((win.geometry.width, win.geometry.height), usable_area);
        }
    }

    /// Returns the list of currently visible windows on the active workspace in stacking order (bottom to top).
    #[must_use]
    pub fn visible_windows(&self) -> Vec<(WindowId, WindowGeometry)> {
        let active_ws = self.workspaces.active_workspace();
        let ws_windows = self.workspaces.windows_on(active_ws).unwrap_or(&[]);
        self.stacking
            .windows()
            .iter()
            .copied()
            .filter(|w| ws_windows.contains(w))
            .filter_map(|wid| {
                let win = self.windows.get(wid)?;
                if win.state != WindowState::Minimized && win.is_mapped {
                    Some((wid, win.geometry))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Arranges managed windows according to the active layout engine.
    pub fn arrange(&mut self, output_geo: WindowGeometry, usable_area: WindowGeometry) {
        let mut window_refs = Vec::with_capacity(self.windows.len());
        for (wid, win) in &mut self.windows {
            window_refs.push((wid, win));
        }
        self.layout
            .arrange(output_geo, usable_area, &mut window_refs);
    }

    /// Checks internal invariants across all coordinated structures.
    #[must_use]
    pub fn check_invariants(&self) -> bool {
        if !self.workspaces.check_invariants() {
            return false;
        }

        // Every window in workspace manager must exist in self.windows
        for ws_id in self.workspaces.workspace_ids() {
            if let Some(windows) = self.workspaces.windows_on(*ws_id) {
                for &wid in windows {
                    if !self.windows.contains_key(wid) {
                        return false;
                    }
                }
            }
        }

        // Every window in stacking order must exist in self.windows
        for &wid in self.stacking.windows() {
            if !self.windows.contains_key(wid) {
                return false;
            }
        }

        // Every window in focus stack must exist in self.windows
        for &wid in self.focus.mru_order() {
            if !self.windows.contains_key(wid) {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_manager_lifecycle() {
        let mut wm = WindowManager::new(4);
        let usable = WindowGeometry::new(0, 32, 1920, 1048);
        let output = WindowGeometry::new(0, 0, 1920, 1080);

        // Map window 1
        let w1 = wm
            .map_window(&MapWindowParams {
                geometry: WindowGeometry::new(0, 0, 800, 600),
                constraints: SizeConstraints::default(),
                title: "Window 1",
                app_id: "app1",
                target_workspace: None,
                usable_area: usable,
                focus_new_windows_cfg: true,
            })
            .expect("map succeeds");

        assert_eq!(wm.focus().focused(), Some(w1));
        assert_eq!(wm.stacking().topmost(), Some(w1));
        assert!(wm.check_invariants());

        // Map window 2
        let w2 = wm
            .map_window(&MapWindowParams {
                geometry: WindowGeometry::new(0, 0, 600, 400),
                constraints: SizeConstraints::default(),
                title: "Window 2",
                app_id: "app2",
                target_workspace: None,
                usable_area: usable,
                focus_new_windows_cfg: true,
            })
            .expect("map succeeds");

        assert_eq!(wm.focus().focused(), Some(w2));
        assert_eq!(wm.stacking().topmost(), Some(w2));
        assert!(wm.check_invariants());

        // Maximize w1
        wm.maximize_window(w1, usable);
        assert_eq!(wm.window(w1).map(|w| w.state), Some(WindowState::Maximized));
        assert_eq!(wm.window(w1).map(|w| w.geometry), Some(usable));

        // Unmaximize w1
        wm.unmaximize_window(w1, usable);
        assert_eq!(wm.window(w1).map(|w| w.state), Some(WindowState::Normal));

        // Minimize w2 -> focus reverts to w1
        wm.minimize_window(w2);
        assert_eq!(wm.window(w2).map(|w| w.state), Some(WindowState::Minimized));
        assert_eq!(wm.focus().focused(), Some(w1));

        // Restore w2
        wm.unminimize_window(w2, usable);
        assert_eq!(wm.window(w2).map(|w| w.state), Some(WindowState::Normal));
        assert_eq!(wm.focus().focused(), Some(w2));

        // Fullscreen w1
        wm.fullscreen_window(w1, output);
        assert_eq!(
            wm.window(w1).map(|w| w.state),
            Some(WindowState::Fullscreen)
        );
        assert_eq!(wm.window(w1).map(|w| w.geometry), Some(output));

        // Center w2
        wm.center_window(w2, usable);
        assert!(wm.check_invariants());

        // Unmap w1
        assert!(wm.unmap_window(w1).is_some());
        assert_eq!(wm.focus().focused(), Some(w2));
        assert!(wm.check_invariants());
    }
}

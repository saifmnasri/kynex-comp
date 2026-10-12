// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Bounded workspace model and workspace management for `KynexComp`.

use slotmap::{SlotMap, new_key_type};
use thiserror::Error;

use crate::window::WindowId;

new_key_type! {
    /// O(1) typed identifier for a virtual workspace.
    pub struct WorkspaceId;
}

/// Hard limit on the number of windows per workspace (Rule 05 anti-DoS).
pub const MAX_WINDOWS_PER_WORKSPACE: usize = 256;

/// Hard limit on total workspaces.
pub const MAX_WORKSPACES: usize = 32;

/// Default initial number of workspaces.
pub const DEFAULT_WORKSPACE_COUNT: usize = 4;

/// Errors returned by workspace operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkspaceError {
    /// Specified workspace does not exist.
    #[error("Workspace not found")]
    NotFound,
    /// Workspace capacity limit exceeded.
    #[error("Workspace capacity exceeded (max {0})")]
    CapacityExceeded(usize),
    /// Window not found.
    #[error("Window not found")]
    WindowNotFound,
}

/// A virtual workspace holding a bounded list of windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    /// Unique workspace identifier.
    pub id: WorkspaceId,
    /// Human-readable workspace name (e.g. "1", "2").
    pub name: String,
    pub(crate) windows: Vec<WindowId>,
}

impl Workspace {
    /// Returns the windows currently assigned to this workspace.
    #[must_use]
    pub fn windows(&self) -> &[WindowId] {
        &self.windows
    }

    /// Checks if a window belongs to this workspace.
    #[must_use]
    pub fn contains(&self, wid: WindowId) -> bool {
        self.windows.contains(&wid)
    }
}

/// Global workspace manager coordinating workspaces and window assignments.
#[derive(Debug, Clone)]
pub struct WorkspaceManager {
    workspaces: SlotMap<WorkspaceId, Workspace>,
    order: Vec<WorkspaceId>,
    active: WorkspaceId,
}

impl WorkspaceManager {
    /// Creates a new workspace manager with `count` initial workspaces (clamped to 1..=32).
    #[must_use]
    pub fn new(count: usize) -> Self {
        let count = count.clamp(1, MAX_WORKSPACES);
        let mut workspaces = SlotMap::with_key();
        let mut order = Vec::with_capacity(count);

        for i in 1..=count {
            let name = i.to_string();
            let id = workspaces.insert_with_key(|id| Workspace {
                id,
                name,
                windows: Vec::new(),
            });
            order.push(id);
        }

        let active = order.first().copied().unwrap_or_default();

        Self {
            workspaces,
            order,
            active,
        }
    }

    /// Returns the ID of the currently active workspace.
    #[must_use]
    pub fn active_workspace(&self) -> WorkspaceId {
        self.active
    }

    /// Returns the ordered list of workspace IDs.
    #[must_use]
    pub fn workspace_ids(&self) -> &[WorkspaceId] {
        &self.order
    }

    /// Returns a reference to the active workspace, if present.
    #[must_use]
    pub fn active(&self) -> Option<&Workspace> {
        self.workspaces.get(self.active)
    }

    /// Switches the active workspace to `id`. Returns `true` if changed.
    pub fn switch_to(&mut self, id: WorkspaceId) -> bool {
        if self.workspaces.contains_key(id) && self.active != id {
            self.active = id;
            true
        } else {
            false
        }
    }

    /// Switches the active workspace by 1-based index (e.g. 1 for first).
    pub fn switch_to_index(&mut self, index_1_based: usize) -> bool {
        if index_1_based == 0 || index_1_based > self.order.len() {
            return false;
        }
        if let Some(&id) = self.order.get(index_1_based.saturating_sub(1)) {
            self.switch_to(id)
        } else {
            false
        }
    }

    /// Adds a window to `target_ws`, removing it from any previous workspace.
    ///
    /// # Errors
    /// Returns [`WorkspaceError::NotFound`] if workspace does not exist,
    /// or [`WorkspaceError::CapacityExceeded`] if workspace is full.
    pub fn add_window(
        &mut self,
        target_ws: WorkspaceId,
        wid: WindowId,
    ) -> Result<(), WorkspaceError> {
        // Remove from existing workspace first if present to preserve the invariant
        // that a window belongs to at most one workspace.
        self.remove_window(wid);

        let ws = self
            .workspaces
            .get_mut(target_ws)
            .ok_or(WorkspaceError::NotFound)?;
        if ws.windows.len() >= MAX_WINDOWS_PER_WORKSPACE {
            return Err(WorkspaceError::CapacityExceeded(MAX_WINDOWS_PER_WORKSPACE));
        }

        ws.windows.push(wid);
        Ok(())
    }

    /// Removes a window from whichever workspace contains it, returning that workspace ID.
    pub fn remove_window(&mut self, wid: WindowId) -> Option<WorkspaceId> {
        for (id, ws) in &mut self.workspaces {
            if let Some(pos) = ws.windows.iter().position(|&w| w == wid) {
                ws.windows.remove(pos);
                return Some(id);
            }
        }
        None
    }

    /// Moves a window from its current workspace to `target_ws`.
    ///
    /// # Errors
    /// Returns [`WorkspaceError::NotFound`] if workspace does not exist,
    /// or [`WorkspaceError::CapacityExceeded`] if target workspace is full.
    pub fn move_window(
        &mut self,
        wid: WindowId,
        target_ws: WorkspaceId,
    ) -> Result<(), WorkspaceError> {
        self.add_window(target_ws, wid)
    }

    /// Finds which workspace contains `wid`.
    #[must_use]
    pub fn find_window(&self, wid: WindowId) -> Option<WorkspaceId> {
        for (id, ws) in &self.workspaces {
            if ws.contains(wid) {
                return Some(id);
            }
        }
        None
    }

    /// Returns the windows assigned to `ws`.
    #[must_use]
    pub fn windows_on(&self, ws: WorkspaceId) -> Option<&[WindowId]> {
        self.workspaces.get(ws).map(|w| w.windows.as_slice())
    }

    /// Verifies internal invariants: active workspace exists, and every window appears at most once.
    #[must_use]
    pub fn check_invariants(&self) -> bool {
        if !self.workspaces.contains_key(self.active) {
            return false;
        }
        let mut seen = std::collections::HashSet::new();
        for ws in self.workspaces.values() {
            for &wid in &ws.windows {
                if !seen.insert(wid) {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::SlotMap;

    #[test]
    fn test_workspace_creation_and_bounds() {
        let mgr = WorkspaceManager::new(4);
        assert_eq!(mgr.workspace_ids().len(), 4);
        assert!(mgr.check_invariants());

        // Clamping bounds
        let zero_mgr = WorkspaceManager::new(0);
        assert_eq!(zero_mgr.workspace_ids().len(), 1);

        let huge_mgr = WorkspaceManager::new(100);
        assert_eq!(huge_mgr.workspace_ids().len(), MAX_WORKSPACES);
    }

    #[test]
    fn test_workspace_switch() {
        let mut mgr = WorkspaceManager::new(3);
        let ws1 = mgr.workspace_ids()[0];
        let ws2 = mgr.workspace_ids()[1];

        assert_eq!(mgr.active_workspace(), ws1);
        assert!(mgr.switch_to(ws2));
        assert_eq!(mgr.active_workspace(), ws2);

        // Same workspace switch returns false
        assert!(!mgr.switch_to(ws2));

        // Index switch
        assert!(mgr.switch_to_index(1));
        assert_eq!(mgr.active_workspace(), ws1);
        assert!(!mgr.switch_to_index(99));
    }

    #[test]
    fn test_workspace_add_and_move_window() {
        let mut mgr = WorkspaceManager::new(2);
        let ws1 = mgr.workspace_ids()[0];
        let ws2 = mgr.workspace_ids()[1];

        let mut dummy_map = SlotMap::with_key();
        let w1: WindowId = dummy_map.insert(());

        assert!(mgr.add_window(ws1, w1).is_ok());
        assert_eq!(mgr.find_window(w1), Some(ws1));
        assert_eq!(mgr.windows_on(ws1), Some([w1].as_slice()));

        // Move to ws2
        assert!(mgr.move_window(w1, ws2).is_ok());
        assert_eq!(mgr.find_window(w1), Some(ws2));
        assert_eq!(mgr.windows_on(ws1), Some([].as_slice()));
        assert_eq!(mgr.windows_on(ws2), Some([w1].as_slice()));

        assert!(mgr.check_invariants());

        // Remove
        assert_eq!(mgr.remove_window(w1), Some(ws2));
        assert_eq!(mgr.find_window(w1), None);
    }

    #[test]
    fn test_workspace_capacity_limit() {
        let mut mgr = WorkspaceManager::new(1);
        let ws = mgr.workspace_ids()[0];

        let mut dummy_map = SlotMap::with_key();
        for _ in 0..MAX_WINDOWS_PER_WORKSPACE {
            let wid: WindowId = dummy_map.insert(());
            assert!(mgr.add_window(ws, wid).is_ok());
        }

        // 257th window must fail
        let extra_wid: WindowId = dummy_map.insert(());
        let res = mgr.add_window(ws, extra_wid);
        assert_eq!(
            res,
            Err(WorkspaceError::CapacityExceeded(MAX_WINDOWS_PER_WORKSPACE))
        );
    }
}

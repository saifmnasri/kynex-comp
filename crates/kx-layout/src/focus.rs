// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Focus stack, stacking order, focus policy, and Alt+Tab cycling for `KynexComp`.

use crate::window::WindowId;

/// Policy determining how keyboard and pointer focus is acquired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusPolicy {
    /// Focus changes only when a window or decoration is clicked.
    #[default]
    ClickToFocus,
    /// Focus follows the pointer when hovering across window boundaries.
    FocusFollowsMouse,
}

/// Most-Recently-Used (MRU) tracking of window focus.
///
/// The most recently focused window is at index 0.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FocusStack {
    mru: Vec<WindowId>,
}

impl FocusStack {
    /// Creates an empty focus stack.
    #[must_use]
    pub fn new() -> Self {
        Self { mru: Vec::new() }
    }

    /// Returns the currently focused window, if any.
    #[must_use]
    pub fn focused(&self) -> Option<WindowId> {
        self.mru.first().copied()
    }

    /// Moves `wid` to the head of the MRU stack (focused).
    pub fn push_focused(&mut self, wid: WindowId) {
        if let Some(pos) = self.mru.iter().position(|&w| w == wid) {
            self.mru.remove(pos);
        }
        self.mru.insert(0, wid);
    }

    /// Removes `wid` from the focus stack.
    pub fn remove(&mut self, wid: WindowId) {
        if let Some(pos) = self.mru.iter().position(|&w| w == wid) {
            self.mru.remove(pos);
        }
    }

    /// Returns the windows in MRU order (index 0 is most recently focused).
    #[must_use]
    pub fn mru_order(&self) -> &[WindowId] {
        &self.mru
    }

    /// Clears the focus stack.
    pub fn clear(&mut self) {
        self.mru.clear();
    }
}

/// Z-order stacking representation from bottom (index 0) to top (last element).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StackingOrder {
    bottom_to_top: Vec<WindowId>,
}

impl StackingOrder {
    /// Creates an empty stacking order.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bottom_to_top: Vec::new(),
        }
    }

    /// Inserts `wid` at the very top of the stack.
    pub fn insert_top(&mut self, wid: WindowId) {
        self.remove(wid);
        self.bottom_to_top.push(wid);
    }

    /// Raises `wid` to the top of the stack.
    pub fn raise(&mut self, wid: WindowId) {
        self.insert_top(wid);
    }

    /// Lowers `wid` to the bottom of the stack (index 0).
    pub fn lower(&mut self, wid: WindowId) {
        self.remove(wid);
        self.bottom_to_top.insert(0, wid);
    }

    /// Removes `wid` from the stacking order.
    pub fn remove(&mut self, wid: WindowId) {
        if let Some(pos) = self.bottom_to_top.iter().position(|&w| w == wid) {
            self.bottom_to_top.remove(pos);
        }
    }

    /// Returns the bottom-to-top slice of windows.
    #[must_use]
    pub fn windows(&self) -> &[WindowId] {
        &self.bottom_to_top
    }

    /// Returns the topmost window, if any.
    #[must_use]
    pub fn topmost(&self) -> Option<WindowId> {
        self.bottom_to_top.last().copied()
    }
}

/// Ongoing Alt+Tab interactive switching session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AltTabSession {
    candidates: Vec<WindowId>,
    selected_index: usize,
}

impl AltTabSession {
    /// Starts a new Alt+Tab session with the given candidates in MRU order.
    ///
    /// If there are at least two candidates, the initial selection defaults to
    /// index 1 (the previously focused window), matching standard desktop behavior.
    #[must_use]
    pub fn new(candidates: Vec<WindowId>) -> Option<Self> {
        if candidates.is_empty() {
            None
        } else {
            let initial_index = usize::from(candidates.len() > 1);
            Some(Self {
                candidates,
                selected_index: initial_index,
            })
        }
    }

    /// Cycles forward (towards older windows).
    pub fn cycle_forward(&mut self) {
        if !self.candidates.is_empty() {
            self.selected_index = (self.selected_index.saturating_add(1)) % self.candidates.len();
        }
    }

    /// Cycles backward (towards newer windows).
    pub fn cycle_backward(&mut self) {
        if !self.candidates.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.candidates.len().saturating_sub(1)
            } else {
                self.selected_index.saturating_sub(1)
            };
        }
    }

    /// Returns the currently selected window.
    #[must_use]
    pub fn selected(&self) -> Option<WindowId> {
        self.candidates.get(self.selected_index).copied()
    }

    /// Returns the full candidate list in order.
    #[must_use]
    pub fn candidates(&self) -> &[WindowId] {
        &self.candidates
    }

    /// Returns the current selection index.
    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }
}

/// Evaluates whether a newly mapped window should automatically receive focus.
///
/// When `focus_new_windows_cfg` is false, new windows only take focus if no window
/// is currently focused (preventing focus stealing).
#[must_use]
pub fn should_focus_new_window(
    currently_focused: Option<WindowId>,
    focus_new_windows_cfg: bool,
) -> bool {
    if focus_new_windows_cfg {
        true
    } else {
        currently_focused.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::SlotMap;

    #[test]
    fn test_focus_stack_mru() {
        let mut map = SlotMap::with_key();
        let w1: WindowId = map.insert(());
        let w2: WindowId = map.insert(());
        let w3: WindowId = map.insert(());

        let mut stack = FocusStack::new();
        assert_eq!(stack.focused(), None);

        stack.push_focused(w1);
        assert_eq!(stack.focused(), Some(w1));
        assert_eq!(stack.mru_order(), &[w1]);

        stack.push_focused(w2);
        assert_eq!(stack.focused(), Some(w2));
        assert_eq!(stack.mru_order(), &[w2, w1]);

        stack.push_focused(w3);
        assert_eq!(stack.focused(), Some(w3));
        assert_eq!(stack.mru_order(), &[w3, w2, w1]);

        // Re-focus w1 -> moves to front
        stack.push_focused(w1);
        assert_eq!(stack.focused(), Some(w1));
        assert_eq!(stack.mru_order(), &[w1, w3, w2]);

        // Remove w3
        stack.remove(w3);
        assert_eq!(stack.mru_order(), &[w1, w2]);

        // Clear
        stack.clear();
        assert_eq!(stack.focused(), None);
    }

    #[test]
    fn test_stacking_order_raise_and_lower() {
        let mut map = SlotMap::with_key();
        let w1: WindowId = map.insert(());
        let w2: WindowId = map.insert(());
        let w3: WindowId = map.insert(());

        let mut stack = StackingOrder::new();
        stack.insert_top(w1);
        stack.insert_top(w2);
        stack.insert_top(w3);
        assert_eq!(stack.windows(), &[w1, w2, w3]);
        assert_eq!(stack.topmost(), Some(w3));

        // Raise w1 to top
        stack.raise(w1);
        assert_eq!(stack.windows(), &[w2, w3, w1]);
        assert_eq!(stack.topmost(), Some(w1));

        // Lower w1 to bottom
        stack.lower(w1);
        assert_eq!(stack.windows(), &[w1, w2, w3]);

        // Remove w2
        stack.remove(w2);
        assert_eq!(stack.windows(), &[w1, w3]);
    }

    #[test]
    fn test_alttab_cycling() {
        let mut map = SlotMap::with_key();
        let w1: WindowId = map.insert(());
        let w2: WindowId = map.insert(());
        let w3: WindowId = map.insert(());

        // Empty session
        assert!(AltTabSession::new(Vec::new()).is_none());

        // Single window session
        let mut session_single = AltTabSession::new(vec![w1]).expect("valid session");
        assert_eq!(session_single.selected(), Some(w1));
        session_single.cycle_forward();
        assert_eq!(session_single.selected(), Some(w1));

        // 3 windows session: starts at index 1 (w2)
        let mut session = AltTabSession::new(vec![w1, w2, w3]).expect("valid session");
        assert_eq!(session.selected_index(), 1);
        assert_eq!(session.selected(), Some(w2));

        session.cycle_forward();
        assert_eq!(session.selected_index(), 2);
        assert_eq!(session.selected(), Some(w3));

        // Wraps around to 0
        session.cycle_forward();
        assert_eq!(session.selected_index(), 0);
        assert_eq!(session.selected(), Some(w1));

        // Backward wrap to 2
        session.cycle_backward();
        assert_eq!(session.selected_index(), 2);
        assert_eq!(session.selected(), Some(w3));

        session.cycle_backward();
        assert_eq!(session.selected_index(), 1);
        assert_eq!(session.selected(), Some(w2));
    }

    #[test]
    fn test_focus_new_windows_policy() {
        let mut map = SlotMap::with_key();
        let w1: WindowId = map.insert(());

        // With focus_new_windows = true: always focuses new window
        assert!(should_focus_new_window(None, true));
        assert!(should_focus_new_window(Some(w1), true));

        // With focus_new_windows = false: only focuses if no current focus
        assert!(should_focus_new_window(None, false));
        assert!(!should_focus_new_window(Some(w1), false));
    }
}

// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Key modifiers, keysyms, bindings, and shortcut matching logic.

use crate::action::CompositorAction;
use bitflags::bitflags;

bitflags! {
    /// Active modifier keys held during a key event.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct KeyModifiers: u32 {
        /// Shift modifier key.
        const SHIFT = 1 << 0;
        /// Control modifier key.
        const CTRL  = 1 << 1;
        /// Alt / Option modifier key.
        const ALT   = 1 << 2;
        /// Super / Windows / Command / Logo modifier key.
        const LOGO  = 1 << 3;
    }
}

/// Raw XKB keysym numeric value wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeySym(pub u32);

impl KeySym {
    /// Keysym for the Escape key (`0xff1b`).
    pub const ESCAPE: Self = Self(0xff1b);
    /// Keysym for the `BackSpace` key (`0xff08`).
    pub const BACKSPACE: Self = Self(0xff08);
    /// Keysym for the Return / Enter key (`0xff0d`).
    pub const RETURN: Self = Self(0xff0d);
    /// Keysym for lowercase 'q' (`0x0071`).
    pub const Q: Self = Self(0x0071);
    /// Keysym for uppercase 'Q' (`0x0051`).
    pub const Q_UPPER: Self = Self(0x0051);

    /// Construct a new `KeySym` from a raw numeric value.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Access the underlying raw numeric value.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// A registered keyboard shortcut mapped to an action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    /// Required modifier keys.
    pub modifiers: KeyModifiers,
    /// Target keysym.
    pub keysym: KeySym,
    /// Action triggered on match.
    pub action: CompositorAction,
}

impl KeyBinding {
    /// Construct a new key binding.
    #[must_use]
    pub const fn new(modifiers: KeyModifiers, keysym: KeySym, action: CompositorAction) -> Self {
        Self {
            modifiers,
            keysym,
            action,
        }
    }
}

/// Table of active compositor key bindings.
#[derive(Debug, Default, Clone)]
pub struct BindingMatcher {
    bindings: Vec<KeyBinding>,
}

impl BindingMatcher {
    /// Create an empty binding matcher with no registered shortcuts.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// Create a matcher pre-populated with standard emergency exit shortcuts:
    /// - `Ctrl + Shift + Escape` -> `Quit`
    /// - `Ctrl + Alt + BackSpace` -> `Quit`
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut matcher = Self::new();

        // Emergency exit 1: Ctrl + Shift + Escape
        matcher.add_binding(KeyBinding::new(
            KeyModifiers::CTRL | KeyModifiers::SHIFT,
            KeySym::ESCAPE,
            CompositorAction::Quit,
        ));

        // Emergency exit 2: Ctrl + Alt + Backspace
        matcher.add_binding(KeyBinding::new(
            KeyModifiers::CTRL | KeyModifiers::ALT,
            KeySym::BACKSPACE,
            CompositorAction::Quit,
        ));

        matcher
    }

    /// Register a new key binding.
    pub fn add_binding(&mut self, binding: KeyBinding) {
        self.bindings.push(binding);
    }

    /// Match incoming modifier state and keysym against registered bindings.
    ///
    /// Returns the matched action, or `None` if no binding matched.
    #[must_use]
    pub fn matches(&self, modifiers: KeyModifiers, keysym: KeySym) -> Option<&CompositorAction> {
        for binding in &self.bindings {
            if binding.modifiers == modifiers && binding.keysym == keysym {
                return Some(&binding.action);
            }
        }
        None
    }

    /// Check if the given modifier and keysym combination represents an emergency exit.
    #[must_use]
    pub fn is_emergency_exit(&self, modifiers: KeyModifiers, keysym: KeySym) -> bool {
        matches!(
            self.matches(modifiers, keysym),
            Some(CompositorAction::Quit)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emergency_exit_bindings() {
        let matcher = BindingMatcher::with_defaults();

        // Ctrl + Shift + Escape -> Quit
        assert_eq!(
            matcher.matches(KeyModifiers::CTRL | KeyModifiers::SHIFT, KeySym::ESCAPE),
            Some(&CompositorAction::Quit)
        );
        assert!(
            matcher.is_emergency_exit(KeyModifiers::CTRL | KeyModifiers::SHIFT, KeySym::ESCAPE)
        );

        // Ctrl + Alt + BackSpace -> Quit
        assert_eq!(
            matcher.matches(KeyModifiers::CTRL | KeyModifiers::ALT, KeySym::BACKSPACE),
            Some(&CompositorAction::Quit)
        );
        assert!(
            matcher.is_emergency_exit(KeyModifiers::CTRL | KeyModifiers::ALT, KeySym::BACKSPACE)
        );

        // No match for plain Escape
        assert_eq!(matcher.matches(KeyModifiers::empty(), KeySym::ESCAPE), None);
        // No match for Ctrl + Escape
        assert_eq!(matcher.matches(KeyModifiers::CTRL, KeySym::ESCAPE), None);
    }

    #[test]
    fn test_custom_binding() {
        let mut matcher = BindingMatcher::new();
        matcher.add_binding(KeyBinding::new(
            KeyModifiers::LOGO,
            KeySym::Q,
            CompositorAction::CloseWindow,
        ));

        assert_eq!(
            matcher.matches(KeyModifiers::LOGO, KeySym::Q),
            Some(&CompositorAction::CloseWindow)
        );
        assert_eq!(matcher.matches(KeyModifiers::CTRL, KeySym::Q), None);
    }
}

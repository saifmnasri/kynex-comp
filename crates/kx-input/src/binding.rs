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
    /// Keysym for the Tab key (`0xff09`).
    pub const TAB: Self = Self(0xff09);
    /// Keysym for the Return / Enter key (`0xff0d`).
    pub const RETURN: Self = Self(0xff0d);
    /// Keysym for the F4 function key (`0xffbf`).
    pub const F4: Self = Self(0xffbf);
    /// Keysym for lowercase 'q' (`0x0071`).
    pub const Q: Self = Self(0x0071);
    /// Keysym for uppercase 'Q' (`0x0051`).
    pub const Q_UPPER: Self = Self(0x0051);
    /// Keysym for lowercase 'w' (`0x0077`).
    pub const W: Self = Self(0x0077);
    /// Keysym for uppercase 'W' (`0x0057`).
    pub const W_UPPER: Self = Self(0x0057);
    /// Keysym for lowercase 'f' (`0x0066`).
    pub const F: Self = Self(0x0066);
    /// Keysym for uppercase 'F' (`0x0046`).
    pub const F_UPPER: Self = Self(0x0046);
    /// Keysym for lowercase 'm' (`0x006d`).
    pub const M: Self = Self(0x006d);
    /// Keysym for uppercase 'M' (`0x004d`).
    pub const M_UPPER: Self = Self(0x004d);
    /// Keysym for lowercase 'c' (`0x0063`).
    pub const C: Self = Self(0x0063);
    /// Keysym for uppercase 'C' (`0x0043`).
    pub const C_UPPER: Self = Self(0x0043);
    /// Keysym for digit '1' (`0x0031`).
    pub const KEY_1: Self = Self(0x0031);
    /// Keysym for digit '2' (`0x0032`).
    pub const KEY_2: Self = Self(0x0032);
    /// Keysym for digit '3' (`0x0033`).
    pub const KEY_3: Self = Self(0x0033);
    /// Keysym for digit '4' (`0x0034`).
    pub const KEY_4: Self = Self(0x0034);

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

/// Primary modifier key configuration for window management shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PrimaryModifier {
    /// Super / Windows / Command / Logo key (default for DRM mode).
    #[default]
    Super,
    /// Ctrl + Alt combined modifier (dev mode for nested compositor under host desktop).
    CtrlAlt,
}

impl PrimaryModifier {
    /// Parse from a string identifier ("Super" or "`CtrlAlt`", case-insensitive).
    #[must_use]
    pub fn from_str_name(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("super") {
            Some(Self::Super)
        } else if name.eq_ignore_ascii_case("ctrlalt") || name.eq_ignore_ascii_case("ctrl_alt") {
            Some(Self::CtrlAlt)
        } else {
            None
        }
    }

    /// The key modifier combination corresponding to this primary modifier.
    #[must_use]
    pub fn modifiers(self) -> KeyModifiers {
        match self {
            Self::Super => KeyModifiers::LOGO,
            Self::CtrlAlt => KeyModifiers::CTRL | KeyModifiers::ALT,
        }
    }

    /// Check if this primary modifier is active in the provided modifier set.
    #[must_use]
    pub fn is_active(self, mods: KeyModifiers) -> bool {
        mods.contains(self.modifiers())
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
    primary_modifier: PrimaryModifier,
}

impl BindingMatcher {
    /// Create an empty binding matcher with no registered shortcuts.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bindings: Vec::new(),
            primary_modifier: PrimaryModifier::Super,
        }
    }

    /// Create a matcher pre-populated with emergency exit shortcuts and window management
    /// shortcuts configured for the given primary modifier.
    #[must_use]
    pub fn with_primary_modifier(primary_mod: PrimaryModifier) -> Self {
        let mut matcher = Self {
            bindings: Vec::new(),
            primary_modifier: primary_mod,
        };
        matcher.add_emergency_bindings();
        match primary_mod {
            PrimaryModifier::Super => matcher.add_standard_bindings(),
            PrimaryModifier::CtrlAlt => matcher.add_dev_bindings(),
        }
        matcher
    }

    /// Access the primary modifier configured on this matcher.
    #[must_use]
    pub const fn primary_modifier(&self) -> PrimaryModifier {
        self.primary_modifier
    }

    /// Create a matcher pre-populated with standard emergency exit and default Super shortcuts.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::with_primary_modifier(PrimaryModifier::Super)
    }

    fn add_emergency_bindings(&mut self) {
        let cs = KeyModifiers::CTRL | KeyModifiers::SHIFT;
        let ca = KeyModifiers::CTRL | KeyModifiers::ALT;
        let cas = KeyModifiers::CTRL | KeyModifiers::ALT | KeyModifiers::SHIFT;

        let emergency_keys = [
            (cs, KeySym::ESCAPE),
            (ca, KeySym::BACKSPACE),
            (ca, KeySym::Q),
            (ca, KeySym::Q_UPPER),
            (cs, KeySym::Q),
            (cs, KeySym::Q_UPPER),
            (cas, KeySym::Q),
            (cas, KeySym::Q_UPPER),
        ];
        for (mods, sym) in emergency_keys {
            self.add_binding(KeyBinding::new(mods, sym, CompositorAction::Quit));
        }
    }

    fn add_standard_bindings(&mut self) {
        let logo = KeyModifiers::LOGO;
        let alt = KeyModifiers::ALT;

        self.add_binding(KeyBinding::new(
            alt,
            KeySym::TAB,
            CompositorAction::CycleFocus,
        ));
        self.add_binding(KeyBinding::new(
            alt | KeyModifiers::SHIFT,
            KeySym::TAB,
            CompositorAction::CycleFocusBackward,
        ));
        self.add_binding(KeyBinding::new(
            alt,
            KeySym::F4,
            CompositorAction::CloseWindow,
        ));

        let logo_actions = [
            (KeySym::Q, CompositorAction::CloseWindow),
            (KeySym::Q_UPPER, CompositorAction::CloseWindow),
            (KeySym::W, CompositorAction::CloseWindow),
            (KeySym::W_UPPER, CompositorAction::CloseWindow),
            (KeySym::F, CompositorAction::ToggleFullscreen),
            (KeySym::F_UPPER, CompositorAction::ToggleFullscreen),
            (KeySym::M, CompositorAction::ToggleMaximize),
            (KeySym::M_UPPER, CompositorAction::ToggleMaximize),
            (KeySym::C, CompositorAction::RecoverTopmostWindow),
            (KeySym::C_UPPER, CompositorAction::RecoverTopmostWindow),
        ];
        for (sym, action) in logo_actions {
            self.add_binding(KeyBinding::new(logo, sym, action));
        }

        let num_keys = [
            (KeySym::KEY_1, 1),
            (KeySym::KEY_2, 2),
            (KeySym::KEY_3, 3),
            (KeySym::KEY_4, 4),
        ];
        for (sym, ws) in num_keys {
            self.add_binding(KeyBinding::new(
                logo,
                sym,
                CompositorAction::SwitchWorkspace(ws),
            ));
            self.add_binding(KeyBinding::new(
                logo | KeyModifiers::SHIFT,
                sym,
                CompositorAction::MoveToWorkspace(ws),
            ));
        }
    }

    fn add_dev_bindings(&mut self) {
        let ca = KeyModifiers::CTRL | KeyModifiers::ALT;
        let cas = ca | KeyModifiers::SHIFT;

        self.add_binding(KeyBinding::new(
            ca,
            KeySym::TAB,
            CompositorAction::CycleFocus,
        ));
        self.add_binding(KeyBinding::new(
            cas,
            KeySym::TAB,
            CompositorAction::CycleFocusBackward,
        ));

        let ca_actions = [
            (KeySym::W, CompositorAction::CloseWindow),
            (KeySym::W_UPPER, CompositorAction::CloseWindow),
            (KeySym::F, CompositorAction::ToggleFullscreen),
            (KeySym::F_UPPER, CompositorAction::ToggleFullscreen),
            (KeySym::M, CompositorAction::ToggleMaximize),
            (KeySym::M_UPPER, CompositorAction::ToggleMaximize),
            (KeySym::C, CompositorAction::RecoverTopmostWindow),
            (KeySym::C_UPPER, CompositorAction::RecoverTopmostWindow),
        ];
        for (sym, action) in ca_actions {
            self.add_binding(KeyBinding::new(ca, sym, action));
        }

        let num_keys = [
            (KeySym::KEY_1, 1),
            (KeySym::KEY_2, 2),
            (KeySym::KEY_3, 3),
            (KeySym::KEY_4, 4),
        ];
        for (sym, ws) in num_keys {
            self.add_binding(KeyBinding::new(
                ca,
                sym,
                CompositorAction::SwitchWorkspace(ws),
            ));
            self.add_binding(KeyBinding::new(
                cas,
                sym,
                CompositorAction::MoveToWorkspace(ws),
            ));
        }
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

        // Ctrl + Alt + Shift + Q -> Quit
        assert_eq!(
            matcher.matches(
                KeyModifiers::CTRL | KeyModifiers::ALT | KeyModifiers::SHIFT,
                KeySym::Q
            ),
            Some(&CompositorAction::Quit)
        );

        // No match for plain Escape
        assert_eq!(matcher.matches(KeyModifiers::empty(), KeySym::ESCAPE), None);
        // No match for Ctrl + Escape
        assert_eq!(matcher.matches(KeyModifiers::CTRL, KeySym::ESCAPE), None);
    }

    #[test]
    fn test_standard_wm_bindings() {
        let matcher = BindingMatcher::with_primary_modifier(PrimaryModifier::Super);

        assert_eq!(
            matcher.matches(KeyModifiers::ALT, KeySym::TAB),
            Some(&CompositorAction::CycleFocus)
        );
        assert_eq!(
            matcher.matches(KeyModifiers::ALT | KeyModifiers::SHIFT, KeySym::TAB),
            Some(&CompositorAction::CycleFocusBackward)
        );
        assert_eq!(
            matcher.matches(KeyModifiers::LOGO, KeySym::M),
            Some(&CompositorAction::ToggleMaximize)
        );
        assert_eq!(
            matcher.matches(KeyModifiers::LOGO, KeySym::F),
            Some(&CompositorAction::ToggleFullscreen)
        );
        assert_eq!(
            matcher.matches(KeyModifiers::LOGO, KeySym::C),
            Some(&CompositorAction::RecoverTopmostWindow)
        );
        assert_eq!(
            matcher.matches(KeyModifiers::LOGO, KeySym::KEY_2),
            Some(&CompositorAction::SwitchWorkspace(2))
        );
        assert_eq!(
            matcher.matches(KeyModifiers::LOGO | KeyModifiers::SHIFT, KeySym::KEY_3),
            Some(&CompositorAction::MoveToWorkspace(3))
        );
    }

    #[test]
    fn test_dev_wm_bindings() {
        let matcher = BindingMatcher::with_primary_modifier(PrimaryModifier::CtrlAlt);
        let ca = KeyModifiers::CTRL | KeyModifiers::ALT;
        let cas = KeyModifiers::CTRL | KeyModifiers::ALT | KeyModifiers::SHIFT;

        assert_eq!(
            matcher.matches(ca, KeySym::TAB),
            Some(&CompositorAction::CycleFocus)
        );
        assert_eq!(
            matcher.matches(cas, KeySym::TAB),
            Some(&CompositorAction::CycleFocusBackward)
        );
        assert_eq!(
            matcher.matches(ca, KeySym::W),
            Some(&CompositorAction::CloseWindow)
        );
        assert_eq!(
            matcher.matches(ca, KeySym::M),
            Some(&CompositorAction::ToggleMaximize)
        );
        assert_eq!(
            matcher.matches(ca, KeySym::F),
            Some(&CompositorAction::ToggleFullscreen)
        );
        assert_eq!(
            matcher.matches(ca, KeySym::C),
            Some(&CompositorAction::RecoverTopmostWindow)
        );
        assert_eq!(
            matcher.matches(ca, KeySym::KEY_1),
            Some(&CompositorAction::SwitchWorkspace(1))
        );
        assert_eq!(
            matcher.matches(cas, KeySym::KEY_4),
            Some(&CompositorAction::MoveToWorkspace(4))
        );
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

    #[test]
    fn test_primary_modifier_helpers() {
        let sup = PrimaryModifier::Super;
        assert_eq!(sup.modifiers(), KeyModifiers::LOGO);
        assert!(sup.is_active(KeyModifiers::LOGO));
        assert!(sup.is_active(KeyModifiers::LOGO | KeyModifiers::SHIFT));
        assert!(!sup.is_active(KeyModifiers::CTRL));

        let ca = PrimaryModifier::CtrlAlt;
        assert_eq!(ca.modifiers(), KeyModifiers::CTRL | KeyModifiers::ALT);
        assert!(ca.is_active(KeyModifiers::CTRL | KeyModifiers::ALT));
        assert!(ca.is_active(KeyModifiers::CTRL | KeyModifiers::ALT | KeyModifiers::SHIFT));
        assert!(!ca.is_active(KeyModifiers::CTRL));

        let matcher = BindingMatcher::with_primary_modifier(ca);
        assert_eq!(matcher.primary_modifier(), PrimaryModifier::CtrlAlt);
    }
}

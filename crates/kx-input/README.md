# kx-input

Pure logic key bindings, modifier handling and action dispatch for KynexComp.

## Purpose

Provides data structures and matcher algorithms for compositor shortcuts (such as emergency exit,
window switching, workspace navigation, etc.) without coupling to any Wayland or graphics libraries.

## Public API

- `KeyModifiers`: Bitflags representing Shift, Ctrl, Alt, and Super/Logo modifiers.
- `KeySym`: Newtype wrapping an XKB / keysym numeric value (`u32`).
- `CompositorAction`: Typed actions triggered by keyboard shortcuts.
- `KeyBinding`: Combination of modifier flags, keysym, and corresponding `CompositorAction`.
- `BindingMatcher`: Table of active bindings, matching incoming key events before client dispatch.

## Forbidden Dependencies

This crate is pure logic. It must NEVER depend on:
- `smithay`
- `wayland-server` or `wayland-client`
- `calloop`
- GPU or rendering crates
- IO or filesystem crates

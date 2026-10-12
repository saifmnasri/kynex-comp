// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Pure logic key bindings, modifier handling and action dispatch for `KynexComp`.
//!
//! This crate must not depend on Smithay, Wayland crates, Calloop, or any GPU crate.

pub mod action;
pub mod binding;

pub use action::CompositorAction;
pub use binding::{BindingMatcher, KeyBinding, KeyModifiers, KeySym};

// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Shell core for `KynexComp`.
//!
//! Provides the primary compositor state, output tracking, window management,
//! rendering loop, and protocol dispatch.

pub mod client;
pub mod error;
pub mod nested;
pub mod state;

pub use client::{ClientLimits, ClientState};
pub use error::ShellError;
pub use nested::{DEFAULT_BG_COLOR, NestedConfig, NestedOptions, run_nested};
pub use state::KynexState;

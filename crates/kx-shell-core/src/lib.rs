// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Shell core for `KynexComp`.
//!
//! Provides the primary compositor state, output tracking, window management,
//! rendering loop, and protocol dispatch.

pub mod client;
pub mod error;
pub mod frame_clock;
pub mod nested;
pub mod state;

#[cfg(feature = "fault-injection")]
pub mod fault;

pub use client::{ClientLimits, ClientState};
pub use error::ShellError;
#[cfg(feature = "fault-injection")]
pub use fault::{FaultController, FaultError, FaultSpec, MAX_HANG_SECS, MAX_TRIGGER_FRAME};
pub use frame_clock::{FrameClock, FrameClockError, FrameState, QueueResult};
pub use nested::{DEFAULT_BG_COLOR, NestedConfig, NestedOptions, run_nested};
pub use state::KynexState;

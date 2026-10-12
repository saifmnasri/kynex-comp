// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Fault-injection test harness for verifying watchdog, crash hooks, and error recovery.
//!
//! # Safety Invariant
//!
//! This module is only active when both the `fault-injection` cargo feature and
//! `debug_assertions` are present. It is strictly compiled out of release builds.

#[cfg(all(feature = "fault-injection", not(debug_assertions)))]
compile_error!(
    "Feature 'fault-injection' is strictly forbidden in release builds (requires debug_assertions)"
);

use std::time::{Duration, Instant};
use thiserror::Error;

/// Maximum allowable frame number for scheduled fault triggers (`DoS` guard).
pub const MAX_TRIGGER_FRAME: u64 = 1_000_000;

/// Maximum duration in seconds for simulated hangs (1 hour).
pub const MAX_HANG_SECS: u64 = 3_600;

/// Errors arising from fault specification parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum FaultError {
    /// The specification string is empty or whitespace.
    #[error("fault specification is empty")]
    EmptySpec,

    /// Unrecognized fault type.
    #[error("unknown fault type: {0}")]
    UnknownFaultType(String),

    /// Missing required parameters in spec.
    #[error("missing parameter in fault spec: {0}")]
    MissingParameter(&'static str),

    /// Too many parameters provided in spec.
    #[error("too many parameters in fault spec: {0}")]
    TooManyParameters(String),

    /// Invalid integer parameter.
    #[error("invalid integer value for {field}: {raw}")]
    InvalidInteger {
        /// Field name.
        field: &'static str,
        /// Raw string that failed to parse.
        raw: String,
    },

    /// Value exceeded safety limit.
    #[error("value {val} exceeds safety limit {max} for {field}")]
    ExceedsLimit {
        /// Field name.
        field: &'static str,
        /// Parsed value.
        val: u64,
        /// Upper bound.
        max: u64,
    },
}

/// Parsed fault injection specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultSpec {
    /// Panic at start of rendering frame N.
    PanicOnFrame(u64),
    /// Block/hang the event loop at start of rendering frame N for `duration_secs`.
    HangOnFrame {
        /// Target frame count.
        frame: u64,
        /// Duration of the hang in seconds.
        duration_secs: u64,
    },
    /// Inject GPU context loss during render on frame N.
    GpuLossOnFrame(u64),
}

/// Parses a strict fault specification string.
///
/// Accepted formats:
/// - `panic:<frame>` where 1 <= frame <= `1_000_000`
/// - `hang:<frame>:<duration_secs>` where 1 <= frame <= `1_000_000` and 1 <= `duration_secs` <= 3600
/// - `gpu_loss:<frame>` where 1 <= frame <= `1_000_000`
///
/// # Errors
///
/// Returns [`FaultError`] if the format is invalid, unknown, or exceeds safety bounds.
pub fn parse_fault_spec(input: &str) -> Result<FaultSpec, FaultError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(FaultError::EmptySpec);
    }

    let tokens: Vec<&str> = trimmed.split(':').collect();
    match tokens.as_slice() {
        ["panic", frame_str] => {
            let frame = parse_bounded_u64(frame_str, "frame", 1, MAX_TRIGGER_FRAME)?;
            Ok(FaultSpec::PanicOnFrame(frame))
        }
        ["hang", frame_str, dur_str] => {
            let frame = parse_bounded_u64(frame_str, "frame", 1, MAX_TRIGGER_FRAME)?;
            let dur = parse_bounded_u64(dur_str, "duration_secs", 1, MAX_HANG_SECS)?;
            Ok(FaultSpec::HangOnFrame {
                frame,
                duration_secs: dur,
            })
        }
        ["gpu_loss", frame_str] => {
            let frame = parse_bounded_u64(frame_str, "frame", 1, MAX_TRIGGER_FRAME)?;
            Ok(FaultSpec::GpuLossOnFrame(frame))
        }
        [other, ..] => {
            if *other != "panic" && *other != "hang" && *other != "gpu_loss" {
                Err(FaultError::UnknownFaultType((*other).to_string()))
            } else if tokens.len() < 2 {
                Err(FaultError::MissingParameter("frame"))
            } else {
                Err(FaultError::TooManyParameters(trimmed.to_string()))
            }
        }
        [] => Err(FaultError::EmptySpec),
    }
}

fn parse_bounded_u64(
    raw: &str,
    field: &'static str,
    min: u64,
    max: u64,
) -> Result<u64, FaultError> {
    let val = raw.parse::<u64>().map_err(|_| FaultError::InvalidInteger {
        field,
        raw: raw.to_string(),
    })?;

    if val < min || val > max {
        return Err(FaultError::ExceedsLimit { field, val, max });
    }

    Ok(val)
}

/// Active fault-injection controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FaultController {
    spec: Option<FaultSpec>,
}

impl FaultController {
    /// Creates a new fault controller from an optional spec.
    #[must_use]
    pub fn new(spec: Option<FaultSpec>) -> Self {
        if let Some(s) = spec {
            tracing::warn!(
                spec = ?s,
                "FAULT INJECTION ACTIVE: deliberately simulating abnormal compositor conditions"
            );
        }
        Self { spec }
    }

    /// Loads fault configuration from environment variable `KYNEX_FAULT` if set.
    ///
    /// # Errors
    ///
    /// Returns [`FaultError`] if `KYNEX_FAULT` is set but invalid.
    pub fn from_env() -> Result<Self, FaultError> {
        match std::env::var("KYNEX_FAULT") {
            Ok(val) if !val.trim().is_empty() => {
                let spec = parse_fault_spec(&val)?;
                Ok(Self::new(Some(spec)))
            }
            _ => Ok(Self::new(None)),
        }
    }

    /// Evaluates if a pre-render fault should trigger on the current frame.
    pub fn check_pre_render(&self, frame: u64) {
        let Some(spec) = self.spec else { return };
        match spec {
            FaultSpec::PanicOnFrame(target) if frame == target => {
                trigger_panic(frame);
            }
            FaultSpec::HangOnFrame {
                frame: target,
                duration_secs,
            } if frame == target => {
                trigger_hang(frame, duration_secs);
            }
            _ => {}
        }
    }

    /// Checks if a simulated GPU loss should be returned on this frame.
    #[must_use]
    pub fn should_fail_gpu(&self, frame: u64) -> bool {
        matches!(self.spec, Some(FaultSpec::GpuLossOnFrame(target)) if frame == target)
    }
}

/// Triggers a simulated hang using `std::thread::park_timeout` in a loop.
///
/// Complies with Rule 00 / Rule 06: no `thread::sleep`, no disallowed methods suppression.
fn trigger_hang(frame: u64, duration_secs: u64) {
    tracing::warn!(
        frame,
        duration_secs,
        "Fault-injection: starting deliberate loop stall via park_timeout"
    );
    let deadline = Instant::now() + Duration::from_secs(duration_secs);
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::park_timeout(remaining);
    }
    tracing::warn!(frame, "Fault-injection: deliberate loop stall ended");
}

/// Triggers a deliberate panic for crash-hook validation.
///
/// Explicitly suppressed with a single scoped `expect` per approved condition 2.
#[expect(
    clippy::panic,
    reason = "Deliberate fault injection for crash hook and recovery validation"
)]
fn trigger_panic(frame: u64) -> ! {
    panic!("Fault injection triggered on frame {frame}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_panic_spec() {
        assert_eq!(
            parse_fault_spec("panic:5").unwrap(),
            FaultSpec::PanicOnFrame(5)
        );
        assert_eq!(
            parse_fault_spec("  panic:1000000  ").unwrap(),
            FaultSpec::PanicOnFrame(1_000_000)
        );
        assert!(parse_fault_spec("panic:0").is_err());
        assert!(parse_fault_spec("panic:1000001").is_err());
        assert!(parse_fault_spec("panic:abc").is_err());
    }

    #[test]
    fn test_parse_hang_spec() {
        assert_eq!(
            parse_fault_spec("hang:10:5").unwrap(),
            FaultSpec::HangOnFrame {
                frame: 10,
                duration_secs: 5
            }
        );
        assert!(parse_fault_spec("hang:10:0").is_err());
        assert!(parse_fault_spec("hang:10:3601").is_err());
        assert!(parse_fault_spec("hang:10").is_err());
        assert!(parse_fault_spec("hang:10:5:extra").is_err());
    }

    #[test]
    fn test_parse_gpu_loss_spec() {
        assert_eq!(
            parse_fault_spec("gpu_loss:1").unwrap(),
            FaultSpec::GpuLossOnFrame(1)
        );
        assert!(parse_fault_spec("gpu_loss:0").is_err());
    }

    #[test]
    fn test_parse_invalid_specs() {
        assert!(parse_fault_spec("").is_err());
        assert!(parse_fault_spec("   ").is_err());
        assert!(parse_fault_spec("corrupt:1").is_err());
        assert!(parse_fault_spec("hang:not_a_num:1").is_err());
    }
}

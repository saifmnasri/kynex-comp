// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Typed configuration for `KynexComp`.
//!
//! This crate provides parsing, schema definition, and validation for
//! the TOML configuration files of `KynexComp`.

pub mod error;
pub mod load;
pub mod types;

pub use error::ConfigError;
pub use load::{MAX_CONFIG_SIZE, SUPPORTED_CONFIG_VERSION, load_from_str};
pub use types::{
    AppearanceConfig, Config, DEFAULT_LOG_LEVEL, DEFAULT_REPEAT_DELAY_MS, DEFAULT_REPEAT_RATE_HZ,
    GeneralConfig, InputConfig, MAX_REPEAT_DELAY_MS, MAX_REPEAT_RATE_HZ, MIN_REPEAT_DELAY_MS,
    MIN_REPEAT_RATE_HZ,
};

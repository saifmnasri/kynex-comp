// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Configuration loading and semantic validation.

use serde::Deserialize;

use crate::error::ConfigError;
use crate::types::{
    Config, GeneralConfig, InputConfig, MAX_REPEAT_DELAY_MS, MAX_REPEAT_RATE_HZ,
    MIN_REPEAT_DELAY_MS, MIN_REPEAT_RATE_HZ,
};

/// Maximum allowed configuration file size in bytes (256 KiB).
pub const MAX_CONFIG_SIZE: usize = 262_144;

/// The expected configuration schema version.
pub const SUPPORTED_CONFIG_VERSION: u32 = 1;

/// Permitted values for `general.log_level`.
const VALID_LOG_LEVELS: [&str; 5] = ["error", "warn", "info", "debug", "trace"];

#[derive(Deserialize)]
struct VersionHeader {
    config_version: u32,
}

/// Parses and validates a configuration string.
///
/// # Errors
///
/// Returns [`ConfigError::Oversized`] if `input` exceeds 256 KiB.
/// Returns [`ConfigError::UnsupportedVersion`] if `config_version` is present but is not 1.
/// Returns [`ConfigError::Parse`] if TOML syntax is invalid or unknown fields are present.
/// Returns [`ConfigError::Validation`] if values fail semantic boundary checks.
pub fn load_from_str(input: &str) -> Result<Config, ConfigError> {
    if input.len() > MAX_CONFIG_SIZE {
        return Err(ConfigError::Oversized {
            size: input.len(),
            max: MAX_CONFIG_SIZE,
        });
    }

    // Two-pass parsing: first check version without deny_unknown_fields
    // to return UnsupportedVersion before strict schema validation.
    if let Ok(header) = toml::from_str::<VersionHeader>(input) {
        if header.config_version != SUPPORTED_CONFIG_VERSION {
            return Err(ConfigError::UnsupportedVersion {
                found: header.config_version,
            });
        }
    }

    let config: Config =
        toml::from_str(input).map_err(|err| ConfigError::Parse(err.to_string()))?;

    if config.config_version != SUPPORTED_CONFIG_VERSION {
        return Err(ConfigError::UnsupportedVersion {
            found: config.config_version,
        });
    }

    validate_general(&config.general)?;
    validate_input(&config.input)?;

    Ok(config)
}

fn validate_general(general: &GeneralConfig) -> Result<(), ConfigError> {
    let level = general.log_level.to_ascii_lowercase();
    if !VALID_LOG_LEVELS.contains(&level.as_str()) {
        return Err(ConfigError::Validation(format!(
            "invalid log_level '{}', must be one of: error, warn, info, debug, trace",
            general.log_level
        )));
    }
    Ok(())
}

fn validate_input(input: &InputConfig) -> Result<(), ConfigError> {
    if input.repeat_delay_ms < MIN_REPEAT_DELAY_MS || input.repeat_delay_ms > MAX_REPEAT_DELAY_MS {
        return Err(ConfigError::Validation(format!(
            "repeat_delay_ms {} out of range [{}, {}]",
            input.repeat_delay_ms, MIN_REPEAT_DELAY_MS, MAX_REPEAT_DELAY_MS
        )));
    }

    if input.repeat_rate_hz < MIN_REPEAT_RATE_HZ || input.repeat_rate_hz > MAX_REPEAT_RATE_HZ {
        return Err(ConfigError::Validation(format!(
            "repeat_rate_hz {} out of range [{}, {}]",
            input.repeat_rate_hz, MIN_REPEAT_RATE_HZ, MAX_REPEAT_RATE_HZ
        )));
    }

    Ok(())
}

// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Error types for configuration parsing and validation.

use thiserror::Error;

/// Errors that can occur when parsing and validating configuration.
#[non_exhaustive]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    /// Configuration input exceeds the maximum permitted byte size.
    #[error("configuration input is oversized: {size} bytes exceeds maximum {max} bytes")]
    Oversized {
        /// Actual size in bytes of the supplied input.
        size: usize,
        /// Maximum allowed size in bytes.
        max: usize,
    },

    /// The configuration could not be parsed as valid TOML or deserialize into the schema.
    #[error("configuration parse error: {0}")]
    Parse(String),

    /// The configuration version is not supported by this build.
    #[error("unsupported config_version: {found} (expected version 1)")]
    UnsupportedVersion {
        /// The configuration version found in the input.
        found: u32,
    },

    /// Semantic validation failed for one or more configuration fields.
    #[error("configuration validation failed: {0}")]
    Validation(String),
}

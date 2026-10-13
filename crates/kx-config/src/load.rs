// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Configuration loading and semantic validation.

use serde::Deserialize;

use crate::error::ConfigError;
use crate::types::{
    Config, GeneralConfig, InputConfig, KeybindingConfig, MAX_REPEAT_DELAY_MS, MAX_REPEAT_RATE_HZ,
    MIN_REPEAT_DELAY_MS, MIN_REPEAT_RATE_HZ,
};

/// Maximum allowed configuration file size in bytes (256 KiB).
pub const MAX_CONFIG_SIZE: usize = 262_144;

/// The expected configuration schema version.
pub const SUPPORTED_CONFIG_VERSION: u32 = 1;

/// Permitted values for `general.log_level`.
const VALID_LOG_LEVELS: [&str; 5] = ["error", "warn", "info", "debug", "trace"];

/// Permitted values for `keybindings.primary_mod`.
const VALID_PRIMARY_MODS: [&str; 2] = ["super", "ctrlalt"];

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
    validate_keybindings(&config.keybindings)?;
    validate_privilege(&config.privilege)?;

    Ok(config)
}

fn validate_privilege(privilege: &crate::types::PrivilegeConfig) -> Result<(), ConfigError> {
    for path in &privilege.layer_shell_allow {
        if path.is_empty() || !path.starts_with('/') || path.contains("..") {
            return Err(ConfigError::Validation(format!(
                "invalid layer_shell_allow path '{path}', must be an absolute path without '..'"
            )));
        }
    }
    Ok(())
}

fn validate_keybindings(keybindings: &KeybindingConfig) -> Result<(), ConfigError> {
    let pm = keybindings.primary_mod.to_ascii_lowercase();
    if !VALID_PRIMARY_MODS.contains(&pm.as_str()) {
        return Err(ConfigError::Validation(format!(
            "invalid primary_mod '{}', must be one of: Super, CtrlAlt",
            keybindings.primary_mod
        )));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DEFAULT_LOG_LEVEL, DEFAULT_REPEAT_DELAY_MS, DEFAULT_REPEAT_RATE_HZ};

    #[test]
    fn valid_minimal_config() {
        let toml_str = "config_version = 1\n";
        let config = load_from_str(toml_str).unwrap();
        assert_eq!(config.config_version, 1);
        assert_eq!(config.general.log_level, DEFAULT_LOG_LEVEL);
        assert_eq!(config.input.repeat_delay_ms, DEFAULT_REPEAT_DELAY_MS);
        assert_eq!(config.input.repeat_rate_hz, DEFAULT_REPEAT_RATE_HZ);
        assert!(!config.appearance.enable_animations);
    }

    #[test]
    fn valid_full_config() {
        let toml_str = r#"
            config_version = 1

            [general]
            log_level = "debug"

            [input]
            repeat_delay_ms = 300
            repeat_rate_hz = 50

            [appearance]
            enable_animations = true
        "#;
        let config = load_from_str(toml_str).unwrap();
        assert_eq!(config.config_version, 1);
        assert_eq!(config.general.log_level, "debug");
        assert_eq!(config.input.repeat_delay_ms, 300);
        assert_eq!(config.input.repeat_rate_hz, 50);
        assert!(config.appearance.enable_animations);
    }

    #[test]
    fn unknown_field_rejected() {
        let root_unknown = "config_version = 1\nunknown_key = 42\n";
        assert!(matches!(
            load_from_str(root_unknown),
            Err(ConfigError::Parse(_))
        ));

        let section_unknown = "config_version = 1\n[general]\nlog_level = \"info\"\nextra = true\n";
        assert!(matches!(
            load_from_str(section_unknown),
            Err(ConfigError::Parse(_))
        ));
    }

    #[test]
    fn oversized_rejected_at_boundary() {
        let prefix = "config_version = 1\n# ";
        let pad_len = MAX_CONFIG_SIZE - prefix.len();
        let mut boundary_str = String::with_capacity(MAX_CONFIG_SIZE + 1);
        boundary_str.push_str(prefix);
        boundary_str.extend(std::iter::repeat_n('a', pad_len));
        assert_eq!(boundary_str.len(), MAX_CONFIG_SIZE);

        assert!(load_from_str(&boundary_str).is_ok());

        boundary_str.push('b');
        assert_eq!(boundary_str.len(), MAX_CONFIG_SIZE + 1);
        assert_eq!(
            load_from_str(&boundary_str),
            Err(ConfigError::Oversized {
                size: MAX_CONFIG_SIZE + 1,
                max: MAX_CONFIG_SIZE,
            })
        );
    }

    #[test]
    fn wrong_version_rejected() {
        let v0 = "config_version = 0\n";
        assert_eq!(
            load_from_str(v0),
            Err(ConfigError::UnsupportedVersion { found: 0 })
        );

        let v99 = "config_version = 99\n";
        assert_eq!(
            load_from_str(v99),
            Err(ConfigError::UnsupportedVersion { found: 99 })
        );

        let v2_with_unknown = "config_version = 2\nunknown_future_field = 123\n";
        assert_eq!(
            load_from_str(v2_with_unknown),
            Err(ConfigError::UnsupportedVersion { found: 2 })
        );
    }

    #[test]
    fn defaults_applied() {
        let toml_str = "config_version = 1\n";
        let config = load_from_str(toml_str).unwrap();
        assert_eq!(config.general.log_level, "info");
        assert_eq!(config.input.repeat_delay_ms, 400);
        assert_eq!(config.input.repeat_rate_hz, 25);
        assert!(!config.appearance.enable_animations);
    }

    #[test]
    fn empty_string_rejected() {
        assert!(matches!(load_from_str(""), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn deeply_nested_input_handled_gracefully() {
        let deep = "[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]]";
        assert!(matches!(load_from_str(deep), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn semantic_validation() {
        let invalid_log = "config_version = 1\n[general]\nlog_level = \"verbose\"\n";
        assert!(matches!(
            load_from_str(invalid_log),
            Err(ConfigError::Validation(_))
        ));

        let delay_too_low = "config_version = 1\n[input]\nrepeat_delay_ms = 10\n";
        assert!(matches!(
            load_from_str(delay_too_low),
            Err(ConfigError::Validation(_))
        ));

        let delay_too_high = "config_version = 1\n[input]\nrepeat_delay_ms = 10000\n";
        assert!(matches!(
            load_from_str(delay_too_high),
            Err(ConfigError::Validation(_))
        ));

        let rate_too_low = "config_version = 1\n[input]\nrepeat_rate_hz = 0\n";
        assert!(matches!(
            load_from_str(rate_too_low),
            Err(ConfigError::Validation(_))
        ));

        let rate_too_high = "config_version = 1\n[input]\nrepeat_rate_hz = 500\n";
        assert!(matches!(
            load_from_str(rate_too_high),
            Err(ConfigError::Validation(_))
        ));

        let valid_super = "config_version = 1\n[keybindings]\nprimary_mod = \"Super\"\n";
        let cfg = load_from_str(valid_super).unwrap();
        assert_eq!(cfg.keybindings.primary_mod, "Super");

        let valid_ctrlalt = "config_version = 1\n[keybindings]\nprimary_mod = \"CtrlAlt\"\n";
        let cfg = load_from_str(valid_ctrlalt).unwrap();
        assert_eq!(cfg.keybindings.primary_mod, "CtrlAlt");

        let invalid_mod = "config_version = 1\n[keybindings]\nprimary_mod = \"Alt\"\n";
        assert!(matches!(
            load_from_str(invalid_mod),
            Err(ConfigError::Validation(_))
        ));

        let valid_privilege = r#"
            config_version = 1
            [privilege]
            layer_shell_allow = ["/usr/bin/waybar", "/usr/libexec/kynex-panel"]
        "#;
        let cfg = load_from_str(valid_privilege).unwrap();
        assert_eq!(cfg.privilege.layer_shell_allow.len(), 2);

        let relative_privilege = r#"
            config_version = 1
            [privilege]
            layer_shell_allow = ["bin/waybar"]
        "#;
        assert!(matches!(
            load_from_str(relative_privilege),
            Err(ConfigError::Validation(_))
        ));

        let traversal_privilege = r#"
            config_version = 1
            [privilege]
            layer_shell_allow = ["/usr/bin/../bin/waybar"]
        "#;
        assert!(matches!(
            load_from_str(traversal_privilege),
            Err(ConfigError::Validation(_))
        ));
    }
}

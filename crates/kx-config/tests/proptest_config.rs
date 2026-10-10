// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Property-based testing for configuration parsing and validation.

use std::fmt::Write;

use proptest::prelude::*;

use kx_config::{ConfigError, load_from_str};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Verifies that load_from_str never panics on arbitrary byte sequences.
    #[test]
    fn arbitrary_bytes_never_panic(bytes in any::<Vec<u8>>()) {
        let text = String::from_utf8_lossy(&bytes);
        let _ = load_from_str(&text);
    }

    /// Verifies that load_from_str never panics on arbitrary string inputs.
    #[test]
    fn arbitrary_strings_never_panic(s in "\\PC*") {
        let _ = load_from_str(&s);
    }

    /// Verifies TOML-shaped inputs with generated version numbers and fields.
    #[test]
    fn toml_shaped_inputs_handle_cleanly(
        version in 0u32..10,
        log_level in prop_oneof![
            Just("info".to_string()),
            Just("debug".to_string()),
            Just("warn".to_string()),
            Just("error".to_string()),
            Just("trace".to_string()),
            "[a-z]{1,8}"
        ],
        repeat_delay in 0u32..6000,
        repeat_rate in 0u32..300,
        animations in any::<bool>(),
        include_unknown in any::<bool>()
    ) {
        let mut toml = format!("config_version = {version}\n");
        let _ = write!(
            toml,
            "[general]\nlog_level = \"{log_level}\"\n\n\
             [input]\nrepeat_delay_ms = {repeat_delay}\nrepeat_rate_hz = {repeat_rate}\n\n\
             [appearance]\nenable_animations = {animations}\n"
        );
        if include_unknown {
            toml.push_str("extra_field = 123\n");
        }

        let result = load_from_str(&toml);

        if version != 1 {
            prop_assert_eq!(
                result,
                Err(ConfigError::UnsupportedVersion { found: version })
            );
        } else if include_unknown {
            prop_assert!(matches!(result, Err(ConfigError::Parse(_))));
        } else {
            let valid_log = matches!(log_level.to_ascii_lowercase().as_str(), "error" | "warn" | "info" | "debug" | "trace");
            let valid_delay = (50..=5000).contains(&repeat_delay);
            let valid_rate = (1..=200).contains(&repeat_rate);

            if valid_log && valid_delay && valid_rate {
                prop_assert!(result.is_ok());
            } else {
                prop_assert!(matches!(result, Err(ConfigError::Validation(_))));
            }
        }
    }
}

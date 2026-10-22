// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Typed configuration structures and defaults.

use serde::Deserialize;

/// Default logging verbosity level.
pub const DEFAULT_LOG_LEVEL: &str = "info";

/// Default keyboard repeat delay in milliseconds.
pub const DEFAULT_REPEAT_DELAY_MS: u32 = 400;

/// Minimum allowed keyboard repeat delay in milliseconds.
pub const MIN_REPEAT_DELAY_MS: u32 = 50;

/// Maximum allowed keyboard repeat delay in milliseconds (5 seconds).
pub const MAX_REPEAT_DELAY_MS: u32 = 5000;

/// Default keyboard repeat rate in Hertz.
pub const DEFAULT_REPEAT_RATE_HZ: u32 = 25;

/// Minimum allowed keyboard repeat rate in Hertz.
pub const MIN_REPEAT_RATE_HZ: u32 = 1;

/// Maximum allowed keyboard repeat rate in Hertz.
pub const MAX_REPEAT_RATE_HZ: u32 = 200;

/// Root compositor configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Schema version for the configuration file (must be 1).
    pub config_version: u32,

    /// General runtime options.
    #[serde(default)]
    pub general: GeneralConfig,

    /// Input device configuration.
    #[serde(default)]
    pub input: InputConfig,

    /// Appearance and visual preferences.
    #[serde(default)]
    pub appearance: AppearanceConfig,

    /// Keybinding configuration.
    #[serde(default)]
    pub keybindings: KeybindingConfig,

    /// Privilege and security policy configuration.
    #[serde(default)]
    pub privilege: PrivilegeConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: 1,
            general: GeneralConfig::default(),
            input: InputConfig::default(),
            appearance: AppearanceConfig::default(),
            keybindings: KeybindingConfig::default(),
            privilege: PrivilegeConfig::default(),
        }
    }
}

/// Keybinding and modifier configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeybindingConfig {
    /// Primary modifier key: "Super" (default for DRM) or "`CtrlAlt`" (dev mode for nested).
    #[serde(default = "default_primary_mod")]
    pub primary_mod: String,
}

impl Default for KeybindingConfig {
    fn default() -> Self {
        Self {
            primary_mod: default_primary_mod(),
        }
    }
}

fn default_primary_mod() -> String {
    "Super".to_owned()
}

/// General compositor options.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneralConfig {
    /// Logging level ("error", "warn", "info", "debug", or "trace").
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
        }
    }
}

fn default_log_level() -> String {
    DEFAULT_LOG_LEVEL.to_owned()
}

/// Keyboard and pointer input configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputConfig {
    /// Delay in milliseconds before repeating a held key.
    #[serde(default = "default_repeat_delay")]
    pub repeat_delay_ms: u32,

    /// Key repeat frequency in Hertz.
    #[serde(default = "default_repeat_rate")]
    pub repeat_rate_hz: u32,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            repeat_delay_ms: default_repeat_delay(),
            repeat_rate_hz: default_repeat_rate(),
        }
    }
}

const fn default_repeat_delay() -> u32 {
    DEFAULT_REPEAT_DELAY_MS
}

const fn default_repeat_rate() -> u32 {
    DEFAULT_REPEAT_RATE_HZ
}

/// Appearance and animation preferences.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AppearanceConfig {
    /// Whether window transition animations are enabled.
    #[serde(default)]
    pub enable_animations: bool,
}

/// Privilege and security policy configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PrivilegeConfig {
    /// Allow-listed absolute executable paths permitted to bind `zwlr_layer_shell_v1`.
    #[serde(default)]
    pub layer_shell_allow: Vec<String>,

    /// Allow-listed absolute executable paths permitted to bind `ext_foreign_toplevel_list_v1`.
    #[serde(default)]
    pub foreign_toplevel_allow: Vec<String>,

    /// Allow-listed absolute executable paths permitted to bind `ext_idle_notifier_v1`.
    #[serde(default)]
    pub idle_notify_allow: Vec<String>,

    /// Allow-listed absolute executable paths permitted to bind `ext_session_lock_manager_v1`.
    #[serde(default)]
    pub session_lock_allow: Vec<String>,

    /// Allow-listed absolute executable paths permitted to bind `zwp_input_method_manager_v2`.
    #[serde(default)]
    pub input_method_allow: Vec<String>,
}

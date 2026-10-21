// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Binary entry point for the `KynexComp` Wayland compositor.

#[cfg(all(feature = "fault-injection", not(debug_assertions)))]
compile_error!(
    "Feature 'fault-injection' is strictly forbidden in release builds (requires debug_assertions)"
);

use std::env;
use std::path::PathBuf;
use std::time::Duration;

/// Parsed command line options for `kynex-comp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    /// Backend kind requested by the user.
    pub backend: String,
    /// Optional path to an alternative configuration file.
    pub config_path: Option<PathBuf>,
    /// Optional frame count limit (useful for automated testing).
    pub max_frames: Option<u64>,
    /// Optional timeout before clean exit (useful for automated testing).
    pub exit_after: Option<Duration>,
    /// When true, only query and output hardware diagnostics, then exit immediately.
    pub diagnostic: bool,
}

fn print_help() {
    tracing::info!("Usage: kynex-comp [OPTIONS]");
    tracing::info!("Options:");
    tracing::info!("  --backend <winit|drm|headless>  Select backend (default: winit)");
    tracing::info!("  --config <path>                 Path to configuration file");
    tracing::info!("  --max-frames <count>            Limit rendered frames before exit");
    tracing::info!(
        "  --exit-after <duration>         Exit cleanly after specified duration (e.g. 5s, 500ms)"
    );
    tracing::info!(
        "  --diagnostic                    Inspect DRM devices and KMS resources, then exit"
    );
    tracing::info!("  --help                          Display this help message");
    tracing::info!("  --version                       Display version information");
}

/// Parses human-readable duration strings like "500ms", "5s", "2m", or raw seconds "10".
fn parse_duration(val: &str) -> Result<Duration, String> {
    let trimmed = val.trim();
    if trimmed.is_empty() {
        return Err(String::from("duration string is empty"));
    }
    if let Some(s) = trimmed.strip_suffix("ms") {
        let ms = s
            .trim()
            .parse::<u64>()
            .map_err(|e| format!("invalid milliseconds in duration: {e}"))?;
        Ok(Duration::from_millis(ms))
    } else if let Some(s) = trimmed.strip_suffix('s') {
        let sec = s
            .trim()
            .parse::<u64>()
            .map_err(|e| format!("invalid seconds in duration: {e}"))?;
        Ok(Duration::from_secs(sec))
    } else if let Some(s) = trimmed.strip_suffix('m') {
        let min = s
            .trim()
            .parse::<u64>()
            .map_err(|e| format!("invalid minutes in duration: {e}"))?;
        let sec = min
            .checked_mul(60)
            .ok_or_else(|| String::from("duration overflow in minutes"))?;
        Ok(Duration::from_secs(sec))
    } else {
        let sec = trimmed.parse::<u64>().map_err(|e| {
            format!("invalid duration (expected e.g. 5s, 500ms, or integer seconds): {e}")
        })?;
        Ok(Duration::from_secs(sec))
    }
}

/// Prepares a child process command by stripping compositor-internal environment variables
/// such as `KYNEX_FAULT`.
pub fn prepare_child_command(cmd: &mut std::process::Command) {
    cmd.env_remove("KYNEX_FAULT");
}

fn print_version() {
    #[cfg(feature = "fault-injection")]
    tracing::info!("kynex-comp {}+fault-injection", env!("CARGO_PKG_VERSION"));
    #[cfg(not(feature = "fault-injection"))]
    tracing::info!("kynex-comp {}", env!("CARGO_PKG_VERSION"));
}

fn parse_option(
    arg: &str,
    args: &[String],
    idx: &mut usize,
    cli: &mut CliArgs,
) -> Result<(), String> {
    match arg {
        "--backend" => {
            *idx = idx.saturating_add(1);
            let val = args
                .get(*idx)
                .ok_or_else(|| String::from("missing argument to --backend"))?;
            cli.backend.clone_from(val);
        }
        "--config" => {
            *idx = idx.saturating_add(1);
            let val = args
                .get(*idx)
                .ok_or_else(|| String::from("missing argument to --config"))?;
            cli.config_path = Some(PathBuf::from(val));
        }
        "--max-frames" => {
            *idx = idx.saturating_add(1);
            let val = args
                .get(*idx)
                .ok_or_else(|| String::from("missing argument to --max-frames"))?;
            let frames = val
                .parse::<u64>()
                .map_err(|e| format!("invalid number for --max-frames: {e}"))?;
            cli.max_frames = Some(frames);
        }
        "--exit-after" => {
            *idx = idx.saturating_add(1);
            let val = args
                .get(*idx)
                .ok_or_else(|| String::from("missing argument to --exit-after"))?;
            let dur = parse_duration(val)?;
            cli.exit_after = Some(dur);
        }
        "--diagnostic" => {
            cli.diagnostic = true;
        }
        other => return Err(format!("unknown argument: {other}")),
    }
    Ok(())
}

fn parse_args(args: &[String]) -> Result<Option<CliArgs>, String> {
    let mut cli = CliArgs {
        backend: String::from("winit"),
        config_path: None,
        max_frames: None,
        exit_after: None,
        diagnostic: false,
    };
    let mut idx = 1;

    while idx < args.len() {
        let arg = match args.get(idx) {
            Some(a) => a.as_str(),
            None => break,
        };

        match arg {
            "--help" | "-h" => {
                print_help();
                return Ok(None);
            }
            "--version" | "-v" => {
                print_version();
                return Ok(None);
            }
            _ => parse_option(arg, args, &mut idx, &mut cli)?,
        }
        idx = idx.saturating_add(1);
    }

    Ok(Some(cli))
}

fn resolve_crash_dir() -> PathBuf {
    match env::var_os("XDG_STATE_HOME") {
        Some(val) => PathBuf::from(val).join("kynex").join("crash"),
        None => match env::var_os("HOME") {
            Some(val) => PathBuf::from(val)
                .join(".local")
                .join("state")
                .join("kynex")
                .join("crash"),
            None => PathBuf::from("/tmp/kynex-crash"),
        },
    }
}

/// Loads and validates a configuration file from disk if specified.
///
/// # Errors
///
/// Returns an error if the file cannot be read from disk or if configuration parsing fails.
pub fn load_config_file(
    path: Option<&std::path::Path>,
) -> anyhow::Result<Option<kx_config::Config>> {
    match path {
        Some(p) => {
            let content = std::fs::read_to_string(p).map_err(|e| {
                anyhow::anyhow!("failed to read config file '{}': {e}", p.display())
            })?;
            let cfg = kx_config::load_from_str(&content).map_err(|e| {
                anyhow::anyhow!("failed to parse config file '{}': {e}", p.display())
            })?;
            Ok(Some(cfg))
        }
        None => Ok(None),
    }
}

fn dispatch_backend(parsed: &CliArgs) -> anyhow::Result<()> {
    let app_config = load_config_file(parsed.config_path.as_deref())?;

    if parsed.backend == "winit" || parsed.backend == "nested" {
        let options = kx_shell_core::NestedOptions {
            config: kx_shell_core::NestedConfig::default(),
            app_config,
            max_frames: parsed.max_frames,
            exit_after: parsed.exit_after,
        };
        kx_shell_core::run_nested(&options)?;
    } else if parsed.backend == "drm" {
        let exit_after = if parsed.diagnostic {
            parsed.exit_after
        } else {
            parsed.exit_after.or(Some(Duration::from_secs(10)))
        };
        let options = kx_shell_core::DrmOptions {
            exit_after,
            max_frames: parsed.max_frames,
            diagnostic_only: parsed.diagnostic,
        };
        kx_shell_core::run_drm(&options)?;
    } else {
        anyhow::bail!(
            "Unsupported backend '{}' (supported: 'winit', 'nested', 'drm')",
            parsed.backend
        );
    }
    Ok(())
}

fn run() -> anyhow::Result<()> {
    if let Err(err) = kx_common::init_logging() {
        anyhow::bail!("failed to initialize logging: {err}");
    }

    let crash_dir = resolve_crash_dir();

    #[cfg(feature = "fault-injection")]
    let version_str = concat!(env!("CARGO_PKG_VERSION"), "+fault-injection");
    #[cfg(not(feature = "fault-injection"))]
    let version_str = env!("CARGO_PKG_VERSION");

    kx_common::install_panic_hook(crash_dir, version_str);

    let raw_args: Vec<String> = env::args().collect();
    let parsed = match parse_args(&raw_args) {
        Ok(Some(args)) => args,
        Ok(None) => return Ok(()),
        Err(err) => {
            tracing::error!("{err}");
            anyhow::bail!("{err}");
        }
    };

    tracing::info!(backend = %parsed.backend, "Starting KynexComp");
    dispatch_backend(&parsed)
}

fn main() -> anyhow::Result<()> {
    run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_args() {
        let args = vec![String::from("kynex-comp")];
        let parsed = parse_args(&args).unwrap().unwrap();
        assert_eq!(parsed.backend, "winit");
        assert_eq!(parsed.config_path, None);
    }

    #[test]
    fn test_custom_backend_and_config() {
        let args = vec![
            String::from("kynex-comp"),
            String::from("--backend"),
            String::from("winit"),
            String::from("--config"),
            String::from("/path/to/kynex.toml"),
        ];
        let parsed = parse_args(&args).unwrap().unwrap();
        assert_eq!(parsed.backend, "winit");
        assert_eq!(
            parsed.config_path,
            Some(PathBuf::from("/path/to/kynex.toml"))
        );
    }

    #[test]
    fn test_help_and_version_flags() {
        assert!(
            parse_args(&[String::from("kynex-comp"), String::from("--help")])
                .unwrap()
                .is_none()
        );
        assert!(
            parse_args(&[String::from("kynex-comp"), String::from("-h")])
                .unwrap()
                .is_none()
        );
        assert!(
            parse_args(&[String::from("kynex-comp"), String::from("--version")])
                .unwrap()
                .is_none()
        );
        assert!(
            parse_args(&[String::from("kynex-comp"), String::from("-v")])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn test_missing_and_unknown_flags() {
        assert!(parse_args(&[String::from("kynex-comp"), String::from("--backend")]).is_err());
        assert!(parse_args(&[String::from("kynex-comp"), String::from("--config")]).is_err());
        assert!(parse_args(&[String::from("kynex-comp"), String::from("--unknown")]).is_err());
        assert!(parse_args(&[String::from("kynex-comp"), String::from("--max-frames")]).is_err());
        assert!(
            parse_args(&[
                String::from("kynex-comp"),
                String::from("--max-frames"),
                String::from("invalid")
            ])
            .is_err()
        );
    }

    #[test]
    fn test_max_frames_flag() {
        let args = vec![
            String::from("kynex-comp"),
            String::from("--max-frames"),
            String::from("42"),
        ];
        let parsed = parse_args(&args).unwrap().unwrap();
        assert_eq!(parsed.max_frames, Some(42));
    }

    #[test]
    fn test_exit_after_flag() {
        let args = vec![
            String::from("kynex-comp"),
            String::from("--exit-after"),
            String::from("500ms"),
        ];
        let parsed = parse_args(&args).unwrap().unwrap();
        assert_eq!(parsed.exit_after, Some(Duration::from_millis(500)));

        let args_sec = vec![
            String::from("kynex-comp"),
            String::from("--exit-after"),
            String::from("5s"),
        ];
        let parsed_sec = parse_args(&args_sec).unwrap().unwrap();
        assert_eq!(parsed_sec.exit_after, Some(Duration::from_secs(5)));
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("5s").unwrap(), Duration::from_secs(5));
        assert_eq!(parse_duration("2m").unwrap(), Duration::from_secs(120));
        assert_eq!(parse_duration("10").unwrap(), Duration::from_secs(10));
        assert!(parse_duration("").is_err());
        assert!(parse_duration("abc").is_err());
    }

    #[test]
    fn test_prepare_child_command() {
        let mut cmd = std::process::Command::new("true");
        cmd.env("KYNEX_FAULT", "panic:1");
        cmd.env("ANOTHER_VAR", "1");
        prepare_child_command(&mut cmd);
        // Verify command executes cleanly with environment prepared
        assert!(
            cmd.get_envs()
                .any(|(k, v)| k == "KYNEX_FAULT" && v.is_none())
        );
    }

    #[test]
    fn test_diagnostic_flag() {
        let args = vec![String::from("kynex-comp"), String::from("--diagnostic")];
        let parsed = parse_args(&args).unwrap().unwrap();
        assert_eq!(parsed.backend, "winit");
        assert!(parsed.diagnostic);

        let args_drm = vec![
            String::from("kynex-comp"),
            String::from("--backend"),
            String::from("drm"),
            String::from("--diagnostic"),
        ];
        let parsed_drm = parse_args(&args_drm).unwrap().unwrap();
        assert_eq!(parsed_drm.backend, "drm");
        assert!(parsed_drm.diagnostic);
    }

    #[test]
    fn test_drm_backend_flag() {
        let args = vec![
            String::from("kynex-comp"),
            String::from("--backend"),
            String::from("drm"),
        ];
        let parsed = parse_args(&args).unwrap().unwrap();
        assert_eq!(parsed.backend, "drm");
        assert!(!parsed.diagnostic);
    }

    #[test]
    fn test_load_config_file() {
        assert!(load_config_file(None).unwrap().is_none());
        assert!(load_config_file(Some(std::path::Path::new("/nonexistent/kynex.toml"))).is_err());
    }
}

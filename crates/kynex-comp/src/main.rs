// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Binary entry point for the `KynexComp` Wayland compositor.

use std::env;
use std::path::PathBuf;

/// Parsed command line options for `kynex-comp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    /// Backend kind requested by the user.
    pub backend: String,
    /// Optional path to an alternative configuration file.
    pub config_path: Option<PathBuf>,
    /// Optional frame count limit (useful for automated testing).
    pub max_frames: Option<u64>,
}

fn print_help() {
    tracing::info!("Usage: kynex-comp [OPTIONS]");
    tracing::info!("Options:");
    tracing::info!("  --backend <winit|drm|headless>  Select backend (default: winit)");
    tracing::info!("  --config <path>                 Path to configuration file");
    tracing::info!("  --max-frames <count>            Limit rendered frames before exit");
    tracing::info!("  --help                          Display this help message");
    tracing::info!("  --version                       Display version information");
}

fn parse_args(args: &[String]) -> Result<Option<CliArgs>, String> {
    let mut backend = String::from("winit");
    let mut config_path = None;
    let mut max_frames = None;
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
                tracing::info!("kynex-comp {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--backend" => {
                idx = idx.saturating_add(1);
                let val = args
                    .get(idx)
                    .ok_or_else(|| String::from("missing argument to --backend"))?;
                backend.clone_from(val);
            }
            "--config" => {
                idx = idx.saturating_add(1);
                let val = args
                    .get(idx)
                    .ok_or_else(|| String::from("missing argument to --config"))?;
                config_path = Some(PathBuf::from(val));
            }
            "--max-frames" => {
                idx = idx.saturating_add(1);
                let val = args
                    .get(idx)
                    .ok_or_else(|| String::from("missing argument to --max-frames"))?;
                let frames = val
                    .parse::<u64>()
                    .map_err(|e| format!("invalid number for --max-frames: {e}"))?;
                max_frames = Some(frames);
            }
            other => {
                return Err(format!("unknown argument: {other}"));
            }
        }
        idx = idx.saturating_add(1);
    }

    Ok(Some(CliArgs {
        backend,
        config_path,
        max_frames,
    }))
}

fn run() -> anyhow::Result<()> {
    if let Err(err) = kx_common::init_logging() {
        anyhow::bail!("failed to initialize logging: {err}");
    }

    let crash_dir = match env::var_os("XDG_STATE_HOME") {
        Some(val) => PathBuf::from(val).join("kynex").join("crash"),
        None => match env::var_os("HOME") {
            Some(val) => PathBuf::from(val)
                .join(".local")
                .join("state")
                .join("kynex")
                .join("crash"),
            None => PathBuf::from("/tmp/kynex-crash"),
        },
    };
    kx_common::install_panic_hook(crash_dir, env!("CARGO_PKG_VERSION"));

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

    if parsed.backend != "winit" {
        anyhow::bail!(
            "Only the 'winit' nested backend is supported in Phase 1 (requested: {})",
            parsed.backend
        );
    }

    let options = kx_shell_core::NestedOptions {
        config: kx_shell_core::NestedConfig::default(),
        max_frames: parsed.max_frames,
    };
    kx_shell_core::run_nested(&options)?;

    Ok(())
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
}

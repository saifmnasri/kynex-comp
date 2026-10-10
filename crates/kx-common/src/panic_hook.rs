// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Panic hook and structured crash-report generation for `KynexComp`.

use std::backtrace::Backtrace;
use std::fs::DirBuilder;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

/// Structured crash-report payload.
#[derive(Debug, Clone)]
pub(crate) struct CrashReport {
    pub(crate) message: String,
    pub(crate) location: String,
    pub(crate) backtrace: String,
    pub(crate) version: &'static str,
    pub(crate) timestamp: String,
    pub(crate) pid: u32,
}

/// Writes a structured crash report to a file in `crash_dir`.
///
/// On Unix, directories are created with mode `0o700` and files with mode `0o600`
/// at creation time without post-creation permission modifications.
///
/// # Errors
///
/// Returns an [`std::io::Error`] if directory creation or file writing fails.
pub(crate) fn write_crash_report(
    crash_dir: &Path,
    report: &CrashReport,
) -> std::io::Result<PathBuf> {
    let mut dir_builder = DirBuilder::new();
    dir_builder.recursive(true);
    #[cfg(unix)]
    dir_builder.mode(0o700);
    dir_builder.create(crash_dir)?;

    let base_name = format!("crash-{}-{}", report.pid, report.timestamp);
    let mut file_path = crash_dir.join(format!("{base_name}.log"));
    let mut counter = 0u32;
    while file_path.exists() && counter < 100 {
        counter += 1;
        file_path = crash_dir.join(format!("{base_name}-{counter}.log"));
    }

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&file_path)?;

    writeln!(file, "KynexComp Crash Report")?;
    writeln!(file, "======================")?;
    writeln!(file, "version: {}", report.version)?;
    writeln!(file, "pid: {}", report.pid)?;
    writeln!(file, "timestamp: {}", report.timestamp)?;
    writeln!(file, "location: {}", report.location)?;
    writeln!(file, "message: {}", report.message)?;
    writeln!(file)?;
    writeln!(file, "Backtrace:")?;
    writeln!(file, "{}", report.backtrace)?;
    file.flush()?;

    Ok(file_path)
}

/// Installs a panic hook that logs structured crash reports to `crash_dir`.
///
/// When a panic occurs, this hook captures the panic payload, source location,
/// process ID, timestamp, and a backtrace using [`Backtrace::force_capture`].
/// A crash report is written to `crash_dir`, and the previously installed hook
/// is then invoked to preserve standard diagnostic output.
///
/// # Panics
///
/// This installation function and the resulting hook closure never panic.
pub fn install_panic_hook(crash_dir: PathBuf, version: &'static str) {
    let prev_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Box<dyn Any> (unknown panic payload)".to_string()
        };

        let location = match info.location() {
            Some(loc) => format!("{}:{}:{}", loc.file(), loc.line(), loc.column()),
            None => "unknown".to_string(),
        };

        let backtrace = Backtrace::force_capture().to_string();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or_else(|_| "0".to_string(), |d| d.as_secs().to_string());

        let report = CrashReport {
            message,
            location,
            backtrace,
            version,
            timestamp,
            pid: std::process::id(),
        };

        // NOTE: We intentionally ignore write errors here because the panic hook
        // must never panic, and stderr/diagnostic reporting is handled by chaining
        // the previous panic hook below.
        let _ = write_crash_report(&crash_dir, &report);

        prev_hook(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_crash_report() {
        let tmp = tempfile::tempdir().unwrap();
        let crash_dir = tmp.path().join("crashes");

        let report = CrashReport {
            message: "unexpected state".to_string(),
            location: "src/main.rs:42:5".to_string(),
            backtrace: "stack frame 1\nstack frame 2".to_string(),
            version: "0.0.1",
            timestamp: "1700000000".to_string(),
            pid: 12345,
        };

        let written_path = write_crash_report(&crash_dir, &report).unwrap();
        assert!(written_path.exists());

        let content = std::fs::read_to_string(&written_path).unwrap();
        assert!(content.contains("KynexComp Crash Report"));
        assert!(content.contains("version: 0.0.1"));
        assert!(content.contains("pid: 12345"));
        assert!(content.contains("timestamp: 1700000000"));
        assert!(content.contains("location: src/main.rs:42:5"));
        assert!(content.contains("message: unexpected state"));
        assert!(content.contains("stack frame 1"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&written_path).unwrap();
            let mode = meta.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}

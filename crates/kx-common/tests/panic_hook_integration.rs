// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for the panic hook and crash report logging.

use std::sync::atomic::{AtomicBool, Ordering};

use kx_common::install_panic_hook;

static PREV_HOOK_INVOKED: AtomicBool = AtomicBool::new(false);

#[test]
fn test_panic_hook_chains_and_writes_report() {
    let tmp = tempfile::tempdir().unwrap();
    let crash_dir = tmp.path().join("crashes");

    // Install a flag-setting previous hook first.
    std::panic::set_hook(Box::new(|_| {
        PREV_HOOK_INVOKED.store(true, Ordering::SeqCst);
    }));

    install_panic_hook(crash_dir.clone(), "0.0.1");

    let result = std::panic::catch_unwind(|| {
        panic!("synthetic test panic");
    });
    assert!(result.is_err());

    // Assert that the chained previous hook was called.
    assert!(PREV_HOOK_INVOKED.load(Ordering::SeqCst));

    // Assert that the crash report directory and log file exist.
    assert!(crash_dir.exists());
    let entries: Vec<_> = std::fs::read_dir(&crash_dir)
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert_eq!(entries.len(), 1);

    let file_path = entries[0].path();
    let file_name = file_path.file_name().unwrap().to_str().unwrap();
    assert!(file_name.starts_with("crash-"));
    assert!(
        file_path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
    );

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("KynexComp Crash Report"));
    assert!(content.contains("version: 0.0.1"));
    assert!(content.contains("message: synthetic test panic"));
    assert!(content.contains("Backtrace:"));
}

// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for the logging subsystem.

use kx_common::{LoggingError, init_logging};

#[test]
fn test_init_logging_idempotence() {
    let first = init_logging();
    assert!(first.is_ok());

    let second = init_logging();
    assert!(matches!(second, Err(LoggingError::AlreadyInitialised)));
}

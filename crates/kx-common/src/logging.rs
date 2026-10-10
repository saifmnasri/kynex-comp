// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Tracing and logging initialisation for `KynexComp`.

use tracing_subscriber::EnvFilter;

/// Errors that can occur during logging initialisation.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    /// The global subscriber has already been configured.
    #[error("logging subsystem has already been initialised")]
    AlreadyInitialised,

    /// An error occurred while initialising the subscriber.
    #[error("failed to initialise logging subscriber: {0}")]
    InitFailed(String),
}

/// Initialises the global `tracing` subscriber for the process.
///
/// Uses `tracing_subscriber::fmt` formatted to standard output/journald with
/// an `EnvFilter` defaulting to `info` if `RUST_LOG` is unset.
///
/// # Errors
///
/// Returns [`LoggingError::AlreadyInitialised`] if a global default subscriber
/// has already been set in this process.
///
/// # Panics
///
/// This function never panics.
pub fn init_logging() -> Result<(), LoggingError> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = tracing_subscriber::fmt().with_env_filter(filter).finish();

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|_| LoggingError::AlreadyInitialised)
}

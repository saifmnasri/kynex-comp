// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Error types for `kx-shell-core`.

use thiserror::Error;

/// Primary error type for shell-core operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ShellError {
    /// Failed to initialize the winit backend.
    #[error("failed to initialize winit backend: {0}")]
    WinitInit(String),

    /// Calloop event loop error.
    #[error("event loop error: {0}")]
    Calloop(#[from] calloop::Error),

    /// Calloop source insertion error.
    #[error("failed to insert event source into loop: {0}")]
    SourceInsertion(String),

    /// Rendering error during frame composition.
    #[error("rendering error: {0}")]
    Render(String),

    /// Buffer swap/submit error.
    #[error("buffer swap error: {0}")]
    SwapBuffers(#[from] smithay::backend::SwapBuffersError),

    /// Signal handling registration error.
    #[error("signal error: {0}")]
    Signal(#[from] calloop::signals::SignalError),

    /// Wayland display initialization error.
    #[error("failed to initialize Wayland display: {0}")]
    DisplayInit(String),

    /// Wayland socket binding error.
    #[error("failed to bind Wayland listening socket: {0}")]
    SocketBind(String),

    /// Wayland client dispatch error.
    #[error("client dispatch error: {0}")]
    ClientDispatch(String),

    /// Seat keyboard initialization error.
    #[error("keyboard initialization error: {0}")]
    Keyboard(#[from] smithay::input::keyboard::Error),

    /// Underlying I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<smithay::backend::winit::Error> for ShellError {
    fn from(err: smithay::backend::winit::Error) -> Self {
        Self::WinitInit(err.to_string())
    }
}

impl From<smithay::reexports::wayland_server::backend::InitError> for ShellError {
    fn from(err: smithay::reexports::wayland_server::backend::InitError) -> Self {
        Self::DisplayInit(err.to_string())
    }
}

impl From<smithay::reexports::wayland_server::BindError> for ShellError {
    fn from(err: smithay::reexports::wayland_server::BindError) -> Self {
        Self::SocketBind(err.to_string())
    }
}

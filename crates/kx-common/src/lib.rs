// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Shared leaf types for `KynexComp`.
//!
//! This crate must not depend on any other `kx-*` crate, on Smithay, or on any Wayland crate.

mod logging;
mod panic_hook;
mod systemd;

pub use logging::{LoggingError, init_logging};
pub use panic_hook::install_panic_hook;
pub use systemd::{SystemdNotifyError, notify_state, sd_notify, sd_notify_to};

/// Human-readable product name.
pub const PRODUCT_NAME: &str = "KynexComp";

/// Name of the compositor binary.
pub const BINARY_NAME: &str = "kynex-comp";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_stable() {
        assert_eq!(PRODUCT_NAME, "KynexComp");
        assert_eq!(BINARY_NAME, "kynex-comp");
    }
}

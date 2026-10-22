// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Wayland Idle Notifier protocol handler (`ext_idle_notifier_v1`).
//!
//! Allows clients to monitor user idle state by creating idle notification objects
//! for specific seats with configurable idle timeout thresholds.
//!
//! Classified as a Public global per `docs/SECURITY.md`.

use smithay::reexports::calloop;
use smithay::reexports::wayland_server::DisplayHandle;
pub use smithay::wayland::idle_notify::{IdleNotifierHandler, IdleNotifierState};

use crate::error::ShellError;
use crate::state::KynexState;

smithay::delegate_idle_notify!(KynexState);

impl IdleNotifierHandler for KynexState {
    fn idle_notifier_state(&mut self) -> &mut IdleNotifierState<Self> {
        &mut self.idle_notifier_state
    }
}

/// Initializes and registers the `ext_idle_notifier_v1` global.
///
/// Creates a dedicated internal event loop for idle timers and registers
/// the protocol global with the display handle.
///
/// # Errors
///
/// Returns a [`ShellError`] if initialization of the internal event loop fails.
pub fn init_idle_notify_global(
    dh: &DisplayHandle,
) -> Result<
    (
        IdleNotifierState<KynexState>,
        calloop::EventLoop<'static, KynexState>,
    ),
    ShellError,
> {
    tracing::info!("Initializing ext_idle_notifier_v1 global");
    let idle_loop = calloop::EventLoop::<KynexState>::try_new().map_err(ShellError::from)?;
    let state = IdleNotifierState::new(dh, idle_loop.handle());
    Ok((state, idle_loop))
}

impl KynexState {
    /// Dispatches any pending idle timers with the specified timeout.
    ///
    /// # Errors
    ///
    /// Returns a [`ShellError`] if dispatching the idle event loop fails.
    pub fn dispatch_idle_timers(
        &mut self,
        timeout: Option<std::time::Duration>,
    ) -> Result<(), ShellError> {
        if let Some(mut idle_loop) = self.idle_loop.take() {
            let res = idle_loop.dispatch(timeout, self);
            self.idle_loop = Some(idle_loop);
            res.map_err(ShellError::from)?;
        }
        Ok(())
    }

    /// Notifies the idle notifier of user activity on the specified seat.
    pub fn notify_idle_activity(&mut self, seat: &smithay::input::Seat<Self>) {
        self.idle_notifier_state.notify_activity(seat);
    }

    /// Notifies the idle notifier of user activity on the primary compositor seat.
    pub fn process_input_activity(&mut self) {
        let seat = self.seat.clone();
        self.notify_idle_activity(&seat);
    }
}

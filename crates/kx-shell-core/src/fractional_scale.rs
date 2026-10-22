// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Fractional scale protocol handler (`wp_fractional_scale_v1`).
//!
//! Provides support for the `wp_fractional_scale_manager_v1` interface, enabling
//! clients to receive preferred fractional scale factors (in 120ths) matching
//! their current display outputs (e.g. 1.25x = 150/120, 1.5x = 180/120).

use smithay::delegate_fractional_scale;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::with_states;
pub use smithay::wayland::fractional_scale::{
    FractionalScaleHandler, FractionalScaleManagerState, FractionalScaleState,
    with_fractional_scale,
};

use crate::state::KynexState;

/// Initializes the `wp_fractional_scale_manager_v1` global on the Wayland display.
///
/// Registers version 1 of the global and enables fractional scale reporting.
#[must_use]
pub fn init_fractional_scale_global(dh: &DisplayHandle) -> FractionalScaleManagerState {
    tracing::info!("Initializing wp_fractional_scale_manager_v1 global (version 1)");
    FractionalScaleManagerState::new::<KynexState>(dh)
}

/// Updates the preferred fractional scale for a surface.
///
/// Sets the scale on the surface's fractional scale state and, if a fractional
/// scale object has been instantiated by the client, dispatches the preferred scale
/// event in units of 120ths per the `wp_fractional_scale_v1` specification.
pub fn update_surface_fractional_scale(surface: &WlSurface, scale: f64) {
    with_states(surface, |states| {
        with_fractional_scale(states, |fs| {
            fs.set_preferred_scale(scale);
        });
    });
}

impl KynexState {
    /// Determines the preferred fractional scale factor for a given surface.
    ///
    /// Resolves the output that the surface belongs to (either via window space mapping
    /// or layer surface mapping) and returns its current scale. If unmapped, falls back
    /// to the primary output's scale.
    #[must_use]
    pub fn surface_scale(&self, surface: &WlSurface) -> f64 {
        if let Some(&wid) = self.surface_to_window_id.get(surface) {
            if let Some(output) = crate::window::primary_output_for_window(self, wid) {
                return output.current_scale().fractional_scale();
            }
        }

        for output in self.space.outputs() {
            let layer_map = smithay::desktop::layer_map_for_output(output);
            if layer_map
                .layer_for_surface(surface, smithay::desktop::WindowSurfaceType::all())
                .is_some()
            {
                return output.current_scale().fractional_scale();
            }
        }

        self.output.current_scale().fractional_scale()
    }
}

impl FractionalScaleHandler for KynexState {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        let scale = self.surface_scale(&surface);
        update_surface_fractional_scale(&surface, scale);
    }
}

delegate_fractional_scale!(KynexState);

#[cfg(test)]
mod tests {
    use smithay::reexports::wayland_server::Display;

    use super::*;

    #[test]
    fn test_init_fractional_scale_global_success() {
        let display = Display::<KynexState>::new().expect("failed to create test wayland display");
        let dh = display.handle();
        let _state = init_fractional_scale_global(&dh);
    }
}

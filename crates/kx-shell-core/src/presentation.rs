// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Presentation time protocol integration (`wp_presentation`).
//!
//! Provides accurate vblank timing feedback to clients for audio/video synchronization,
//! smooth animations, and frame scheduling.

use std::time::Duration;

use smithay::desktop::layer_map_for_output;
use smithay::desktop::utils::OutputPresentationFeedback;
use smithay::desktop::{Space, Window};
use smithay::output::Output;
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Monotonic, Time};
use smithay::wayland::presentation::{PresentationState, Refresh};

use crate::state::KynexState;

smithay::delegate_presentation!(KynexState);

/// Monotonic clock ID (1 on POSIX / Linux systems).
pub const MONOTONIC_CLOCK_ID: u32 = 1;

/// Initializes the `wp_presentation` global.
#[must_use]
pub fn init_presentation_global(display: &DisplayHandle) -> PresentationState {
    PresentationState::new::<KynexState>(display, MONOTONIC_CLOCK_ID)
}

/// Drains presentation feedback callbacks from all mapped windows and layers,
/// and delivers presentation feedback timestamps for the given output.
pub fn post_presentation_feedback(
    space: &Space<Window>,
    output: &Output,
    time: Duration,
    sequence: u64,
) {
    let mut feedback = OutputPresentationFeedback::new(output);

    for window in space.elements() {
        window.take_presentation_feedback(
            &mut feedback,
            |_, _| Some(output.clone()),
            |_, _| wp_presentation_feedback::Kind::Vsync,
        );
    }

    for layer in layer_map_for_output(output).layers() {
        layer.take_presentation_feedback(
            &mut feedback,
            |_, _| Some(output.clone()),
            |_, _| wp_presentation_feedback::Kind::Vsync,
        );
    }

    let refresh = output
        .current_mode()
        .and_then(|m| {
            let refresh_rate = u64::try_from(m.refresh).ok()?;
            if refresh_rate > 0 {
                let nanos = 1_000_000_000_000u64.checked_div(refresh_rate)?;
                Some(Refresh::fixed(Duration::from_nanos(nanos)))
            } else {
                None
            }
        })
        .unwrap_or(Refresh::fixed(Duration::from_millis(16)));

    let monotonic_time = Time::<Monotonic>::from(time);
    feedback.presented(
        monotonic_time,
        refresh,
        sequence,
        wp_presentation_feedback::Kind::Vsync,
    );
}

// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Pure-logic frame clock state machine and vblank pacing.
//!
//! Follows Rule 06:
//! - Event-driven: `Idle -> Queued -> WaitingForVBlank -> Idle`.
//! - Never queue a second frame while one is pending.
//! - Tracks render times and estimates moving maximum for low-latency scheduling.

use std::time::{Duration, Instant};

/// Current state of an output's frame cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameState {
    /// No frame is pending; waiting for client damage or animation tick.
    Idle,
    /// Damage occurred; a frame is queued to render in the current or upcoming cycle.
    Queued,
    /// Rendered frame was submitted to hardware; waiting for page flip / vblank presentation event.
    WaitingForVBlank,
}

/// Result of attempting to queue a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueResult {
    /// Transitioned from `Idle` to `Queued`. Caller must schedule rendering.
    Queued,
    /// Frame cycle is already active (`Queued` or `WaitingForVBlank`); coalesced into pending cycle.
    AlreadyPending,
}

/// Errors from invalid state transitions in the frame clock.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrameClockError {
    /// Render started while clock was not in `Queued` state.
    #[error("cannot begin render in state {0:?}; must be Queued")]
    InvalidRenderStart(FrameState),

    /// Page flip / vblank received while clock was not in `WaitingForVBlank` state.
    #[error("unexpected vblank in state {0:?}; expected WaitingForVBlank")]
    UnexpectedVBlank(FrameState),
}

/// Number of samples retained for moving maximum render time estimation.
const RENDER_SAMPLES_CAPACITY: usize = 16;

/// Pure-logic frame clock managing output pacing and presentation deadlines.
#[derive(Debug, Clone)]
pub struct FrameClock {
    state: FrameState,
    refresh_rate_mhz: u32,
    refresh_interval: Duration,
    render_start: Option<Instant>,
    render_durations: [Duration; RENDER_SAMPLES_CAPACITY],
    sample_index: usize,
    sample_count: usize,
    missed_vblanks: u64,
    frame_count: u64,
    has_pending_damage_while_waiting: bool,
    last_vblank: Option<Instant>,
}

impl FrameClock {
    /// Creates a new frame clock with the specified refresh rate in millihertz (e.g., `60_000` for 60 Hz).
    #[must_use]
    pub fn new(refresh_rate_mhz: u32) -> Self {
        let mhz = refresh_rate_mhz.max(1_000);
        let interval_nanos = (1_000_000_000_000u64)
            .checked_div(u64::from(mhz))
            .unwrap_or(16_666_666);
        let refresh_interval = Duration::from_nanos(interval_nanos);

        Self {
            state: FrameState::Idle,
            refresh_rate_mhz: mhz,
            refresh_interval,
            render_start: None,
            render_durations: [Duration::ZERO; RENDER_SAMPLES_CAPACITY],
            sample_index: 0,
            sample_count: 0,
            missed_vblanks: 0,
            frame_count: 0,
            has_pending_damage_while_waiting: false,
            last_vblank: None,
        }
    }

    /// Returns the current state of the frame cycle.
    #[must_use]
    pub fn state(&self) -> FrameState {
        self.state
    }

    /// Refresh rate of this output in millihertz.
    #[must_use]
    pub fn refresh_rate_mhz(&self) -> u32 {
        self.refresh_rate_mhz
    }

    /// Nominal frame interval derived from the refresh rate.
    #[must_use]
    pub fn refresh_interval(&self) -> Duration {
        self.refresh_interval
    }

    /// Total number of presented frames.
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Total number of detected missed vblanks.
    #[must_use]
    pub fn missed_vblanks(&self) -> u64 {
        self.missed_vblanks
    }

    /// Requests that a frame be scheduled.
    ///
    /// If `Idle`, transitions to `Queued` and returns [`QueueResult::Queued`].
    /// If already `Queued` or `WaitingForVBlank`, coalesces damage and returns [`QueueResult::AlreadyPending`].
    pub fn schedule_frame(&mut self) -> QueueResult {
        match self.state {
            FrameState::Idle => {
                self.state = FrameState::Queued;
                QueueResult::Queued
            }
            FrameState::Queued => QueueResult::AlreadyPending,
            FrameState::WaitingForVBlank => {
                self.has_pending_damage_while_waiting = true;
                QueueResult::AlreadyPending
            }
        }
    }

    /// Marks the start of frame rendering.
    ///
    /// # Errors
    /// Returns [`FrameClockError::InvalidRenderStart`] if the clock is not currently in `Queued` state.
    pub fn on_render_begin(&mut self, now: Instant) -> Result<(), FrameClockError> {
        if self.state != FrameState::Queued {
            return Err(FrameClockError::InvalidRenderStart(self.state));
        }
        self.render_start = Some(now);
        Ok(())
    }

    /// Marks completion of frame submission to the GPU/hardware.
    ///
    /// Transitions from `Queued` to `WaitingForVBlank`.
    ///
    /// # Errors
    /// Returns [`FrameClockError::InvalidRenderStart`] if not currently in `Queued` state.
    pub fn on_render_submitted(&mut self, now: Instant) -> Result<(), FrameClockError> {
        if self.state != FrameState::Queued {
            return Err(FrameClockError::InvalidRenderStart(self.state));
        }

        if let Some(start) = self.render_start.take() {
            let duration = now.saturating_duration_since(start);
            self.record_render_duration(duration);
        }

        self.state = FrameState::WaitingForVBlank;
        Ok(())
    }

    /// Called when the hardware confirms page-flip presentation (vblank).
    ///
    /// Checks elapsed presentation time against the nominal refresh interval and records
    /// any missed vblank intervals.
    ///
    /// # Returns
    /// Returns `Ok(true)` if another frame is already queued due to damage arriving while waiting,
    /// or `Ok(false)` if the clock transitions back to `Idle`.
    ///
    /// # Errors
    /// Returns [`FrameClockError::UnexpectedVBlank`] if a vblank was received when no frame was waiting.
    pub fn on_vblank(&mut self, presentation_time: Instant) -> Result<bool, FrameClockError> {
        if self.state != FrameState::WaitingForVBlank {
            return Err(FrameClockError::UnexpectedVBlank(self.state));
        }

        self.frame_count = self.frame_count.saturating_add(1);

        if let Some(prev) = self.last_vblank {
            let elapsed = presentation_time.saturating_duration_since(prev);
            let threshold = self
                .refresh_interval
                .saturating_add(self.refresh_interval / 2);
            if elapsed > threshold {
                let interval_nanos = self.refresh_interval.as_nanos().max(1);
                let missed_ratio = elapsed.as_nanos() / interval_nanos;
                let missed_count = missed_ratio.saturating_sub(1);
                if missed_count > 0 {
                    let missed_u64 = u64::try_from(missed_count).unwrap_or(u64::MAX);
                    self.missed_vblanks = self.missed_vblanks.saturating_add(missed_u64);
                }
            }
        }
        self.last_vblank = Some(presentation_time);

        if self.has_pending_damage_while_waiting {
            self.has_pending_damage_while_waiting = false;
            self.state = FrameState::Queued;
            Ok(true)
        } else {
            self.state = FrameState::Idle;
            Ok(false)
        }
    }

    /// Returns the most recent recorded frame render duration, if any.
    #[must_use]
    pub fn last_render_duration(&self) -> Duration {
        if self.sample_count == 0 {
            return Duration::ZERO;
        }
        let last_idx = if self.sample_index == 0 {
            RENDER_SAMPLES_CAPACITY.saturating_sub(1)
        } else {
            self.sample_index.saturating_sub(1)
        };
        self.render_durations
            .get(last_idx)
            .copied()
            .unwrap_or(Duration::ZERO)
    }

    /// Computes the 99th percentile render duration over the recent sample window.
    #[must_use]
    pub fn p99_render_duration(&self) -> Duration {
        if self.sample_count == 0 {
            return Duration::ZERO;
        }
        let count = self.sample_count.min(RENDER_SAMPLES_CAPACITY);
        let mut sorted = [Duration::ZERO; RENDER_SAMPLES_CAPACITY];
        if let Some(src_slice) = self.render_durations.get(..count) {
            for (dst, src) in sorted.iter_mut().take(count).zip(src_slice) {
                *dst = *src;
            }
        }
        if let Some(active_slice) = sorted.get_mut(..count) {
            active_slice.sort_unstable();
            let p99_idx = count.saturating_sub(1);
            active_slice.get(p99_idx).copied().unwrap_or(Duration::ZERO)
        } else {
            Duration::ZERO
        }
    }

    /// Computes the moving maximum render duration over the recent sample window.
    #[must_use]
    pub fn moving_max_render_duration(&self) -> Duration {
        if self.sample_count == 0 {
            return Duration::ZERO;
        }
        let count = self.sample_count.min(RENDER_SAMPLES_CAPACITY);
        let mut max = Duration::ZERO;
        for i in 0..count {
            if let Some(d) = self.render_durations.get(i) {
                if *d > max {
                    max = *d;
                }
            }
        }
        max
    }

    /// Called when frame rendering resulted in no damage or state changes, so no page flip was queued.
    ///
    /// Resets the clock to `Idle` (or keeps it `Queued` if damage arrived during rendering).
    pub fn on_render_skipped(&mut self) {
        self.render_start = None;
        if self.has_pending_damage_while_waiting {
            self.has_pending_damage_while_waiting = false;
            self.state = FrameState::Queued;
        } else {
            self.state = FrameState::Idle;
        }
    }

    fn record_render_duration(&mut self, duration: Duration) {
        if let Some(slot) = self.render_durations.get_mut(self.sample_index) {
            *slot = duration;
        }
        self.sample_index = (self.sample_index.saturating_add(1)) % RENDER_SAMPLES_CAPACITY;
        self.sample_count = self.sample_count.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_idle() {
        let clock = FrameClock::new(60_000);
        assert_eq!(clock.state(), FrameState::Idle);
        assert_eq!(clock.refresh_rate_mhz(), 60_000);
        assert_eq!(clock.frame_count(), 0);
        assert_eq!(clock.missed_vblanks(), 0);
        assert_eq!(clock.moving_max_render_duration(), Duration::ZERO);
    }

    #[test]
    fn test_render_skipped_returns_to_idle() {
        let mut clock = FrameClock::new(60_000);
        let now = Instant::now();

        clock.schedule_frame();
        clock.on_render_begin(now).unwrap();
        assert_eq!(clock.state(), FrameState::Queued);

        clock.on_render_skipped();
        assert_eq!(clock.state(), FrameState::Idle);
    }

    #[test]
    fn test_normal_cycle_idle_to_queued_to_vblank_to_idle() {
        let mut clock = FrameClock::new(60_000);
        let now = Instant::now();

        assert_eq!(clock.schedule_frame(), QueueResult::Queued);
        assert_eq!(clock.state(), FrameState::Queued);

        clock.on_render_begin(now).unwrap();
        let submit_time = now + Duration::from_millis(2);
        clock.on_render_submitted(submit_time).unwrap();
        assert_eq!(clock.state(), FrameState::WaitingForVBlank);

        let vblank_time = submit_time + Duration::from_millis(14);
        let next_queued = clock.on_vblank(vblank_time).unwrap();
        assert!(!next_queued);
        assert_eq!(clock.state(), FrameState::Idle);
        assert_eq!(clock.frame_count(), 1);
        assert_eq!(clock.moving_max_render_duration(), Duration::from_millis(2));
    }

    #[test]
    fn test_coalesced_scheduling_while_waiting_for_vblank() {
        let mut clock = FrameClock::new(60_000);
        let now = Instant::now();

        assert_eq!(clock.schedule_frame(), QueueResult::Queued);
        assert_eq!(clock.schedule_frame(), QueueResult::AlreadyPending);

        clock.on_render_begin(now).unwrap();
        clock
            .on_render_submitted(now + Duration::from_millis(1))
            .unwrap();

        // Damage occurs while waiting for page flip
        assert_eq!(clock.schedule_frame(), QueueResult::AlreadyPending);

        // VBlank arrives -> should immediately enter Queued for next frame
        let next_queued = clock.on_vblank(now + Duration::from_millis(16)).unwrap();
        assert!(next_queued);
        assert_eq!(clock.state(), FrameState::Queued);
    }

    #[test]
    fn test_invalid_transitions_return_typed_errors() {
        let mut clock = FrameClock::new(60_000);
        let now = Instant::now();

        // Render begin while Idle
        let err1 = clock.on_render_begin(now).unwrap_err();
        assert_eq!(err1, FrameClockError::InvalidRenderStart(FrameState::Idle));

        // VBlank while Idle
        let err2 = clock.on_vblank(now).unwrap_err();
        assert_eq!(err2, FrameClockError::UnexpectedVBlank(FrameState::Idle));
    }

    #[test]
    fn test_moving_maximum_estimation() {
        let mut clock = FrameClock::new(60_000);
        let now = Instant::now();

        let durations = [
            Duration::from_millis(1),
            Duration::from_millis(4),
            Duration::from_millis(2),
            Duration::from_millis(3),
        ];

        for &d in &durations {
            clock.schedule_frame();
            clock.on_render_begin(now).unwrap();
            clock.on_render_submitted(now + d).unwrap();
            clock.on_vblank(now + Duration::from_millis(16)).unwrap();
        }

        assert_eq!(clock.moving_max_render_duration(), Duration::from_millis(4));
    }

    #[test]
    fn test_missed_vblanks_detection() {
        let mut clock = FrameClock::new(60_000); // 16.666 ms nominal
        let t0 = Instant::now();

        clock.schedule_frame();
        clock.on_render_begin(t0).unwrap();
        clock
            .on_render_submitted(t0 + Duration::from_millis(2))
            .unwrap();
        clock.on_vblank(t0 + Duration::from_millis(16)).unwrap();
        assert_eq!(clock.missed_vblanks(), 0);

        // Frame 2: delayed vblank arriving after ~50ms (missed 2 vblanks)
        clock.schedule_frame();
        clock
            .on_render_begin(t0 + Duration::from_millis(20))
            .unwrap();
        clock
            .on_render_submitted(t0 + Duration::from_millis(22))
            .unwrap();
        clock.on_vblank(t0 + Duration::from_millis(66)).unwrap();
        assert_eq!(clock.missed_vblanks(), 2);
    }

    #[test]
    fn test_percentile_and_last_render_duration() {
        let mut clock = FrameClock::new(60_000);
        let t0 = Instant::now();

        assert_eq!(clock.last_render_duration(), Duration::ZERO);
        assert_eq!(clock.p99_render_duration(), Duration::ZERO);

        for ms in [1, 5, 2, 8, 3] {
            clock.schedule_frame();
            clock.on_render_begin(t0).unwrap();
            clock
                .on_render_submitted(t0 + Duration::from_millis(ms))
                .unwrap();
            clock.on_vblank(t0 + Duration::from_millis(16)).unwrap();
        }

        assert_eq!(clock.last_render_duration(), Duration::from_millis(3));
        assert_eq!(clock.p99_render_duration(), Duration::from_millis(8));
    }
}

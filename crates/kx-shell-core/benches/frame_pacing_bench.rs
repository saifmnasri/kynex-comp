// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Benchmarks for `kx-shell-core` frame clock state machine and vblank pacing telemetry.

#![expect(
    missing_docs,
    reason = "criterion macro generates undocumented harness items"
)]

use std::time::{Duration, Instant};

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use kx_shell_core::frame_clock::FrameClock;

fn bench_frame_pacing(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_clock_pacing");

    group.bench_function("full_frame_cycle_transition", |b| {
        let mut clock = FrameClock::new(60_000);
        let start = Instant::now();
        let mut offset_nanos = 0u64;

        b.iter(|| {
            offset_nanos = offset_nanos.wrapping_add(16_666_666);
            let t1 = start + Duration::from_nanos(offset_nanos);
            let t2 = t1 + Duration::from_millis(2);
            let t3 = t1 + Duration::from_millis(16);

            let _ = clock.schedule_frame();
            let _ = clock.on_render_begin(black_box(t1));
            let _ = clock.on_render_submitted(black_box(t2));
            let _ = clock.on_vblank(black_box(t3));
        });
    });

    group.bench_function("percentile_and_timing_retrieval", |b| {
        let mut clock = FrameClock::new(60_000);
        let start = Instant::now();
        for i in 0..16 {
            let t1 = start + Duration::from_millis(i * 16);
            let t2 = t1 + Duration::from_millis(2 + (i % 3));
            let t3 = t1 + Duration::from_millis(16);
            let _ = clock.schedule_frame();
            let _ = clock.on_render_begin(t1);
            let _ = clock.on_render_submitted(t2);
            let _ = clock.on_vblank(t3);
        }

        b.iter(|| {
            let last = clock.last_render_duration();
            let p99 = clock.p99_render_duration();
            black_box((last, p99));
        });
    });

    group.finish();
}

criterion_group!(benches, bench_frame_pacing);
criterion_main!(benches);

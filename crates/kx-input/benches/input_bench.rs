// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Benchmarks for `kx-input` key binding lookup and shortcut matching.

#![expect(
    missing_docs,
    reason = "criterion macro generates undocumented harness items"
)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use kx_input::{BindingMatcher, CompositorAction, KeyBinding, KeyModifiers, KeySym};

fn bench_input_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("input_binding_matching");
    let default_matcher = BindingMatcher::with_defaults();

    group.bench_function("default_emergency_exit_match", |b| {
        b.iter(|| {
            let res = default_matcher.is_emergency_exit(
                black_box(KeyModifiers::CTRL | KeyModifiers::SHIFT),
                black_box(KeySym::ESCAPE),
            );
            assert!(res);
        });
    });

    group.bench_function("default_non_matching_key", |b| {
        b.iter(|| {
            let res = default_matcher.matches(
                black_box(KeyModifiers::empty()),
                black_box(KeySym::new(0x0061)), // 'a'
            );
            assert!(res.is_none());
        });
    });

    // Matcher with 30 registered custom bindings
    let mut large_matcher = BindingMatcher::with_defaults();
    for i in 1..=30 {
        large_matcher.add_binding(KeyBinding::new(
            KeyModifiers::LOGO,
            KeySym::new(0x0030 + i),
            CompositorAction::Custom(i.to_string()),
        ));
    }

    group.bench_function("thirty_bindings_hit_last", |b| {
        b.iter(|| {
            let res = large_matcher.matches(
                black_box(KeyModifiers::LOGO),
                black_box(KeySym::new(0x0030 + 30)),
            );
            assert!(res.is_some());
        });
    });

    group.finish();
}

criterion_group!(benches, bench_input_matching);
criterion_main!(benches);

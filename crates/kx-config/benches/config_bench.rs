// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Benchmarks for `kx-config` parsing and validation.

#![expect(
    missing_docs,
    reason = "criterion macro generates undocumented harness items"
)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use kx_config::load_from_str;

const MINIMAL_CONFIG: &str = r"
config_version = 1
";

const FULL_CONFIG: &str = r#"
config_version = 1

[general]
log_level = "info"

[input]
repeat_delay_ms = 300
repeat_rate_hz = 30

[appearance]
enable_animations = true
"#;

fn bench_config_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_parsing");

    group.bench_function("minimal_config", |b| {
        b.iter(|| {
            let res = load_from_str(black_box(MINIMAL_CONFIG));
            assert!(res.is_ok());
        });
    });

    group.bench_function("full_config", |b| {
        b.iter(|| {
            let res = load_from_str(black_box(FULL_CONFIG));
            assert!(res.is_ok());
        });
    });

    group.finish();
}

criterion_group!(benches, bench_config_parse);
criterion_main!(benches);

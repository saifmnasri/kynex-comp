// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Benchmarks for `kx-layout` window management and layout operations.

#![expect(
    missing_docs,
    reason = "criterion macro generates undocumented harness items"
)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use kx_layout::{MapWindowParams, SizeConstraints, WindowGeometry, WindowManager};

fn create_params<'a>(x: i32, y: i32, usable: WindowGeometry) -> MapWindowParams<'a> {
    MapWindowParams {
        geometry: WindowGeometry::new(x, y, 800, 600),
        constraints: SizeConstraints::default(),
        title: "Benchmark Window",
        app_id: "org.kynex.bench",
        usable_area: usable,
        target_workspace: None,
        focus_new_windows_cfg: true,
    }
}

fn bench_layout_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_operations");
    let usable = WindowGeometry::new(0, 0, 1920, 1080);

    // 1. Single window map/unmap cycle
    group.bench_function("map_unmap_single_window", |b| {
        let mut wm = WindowManager::new(9);
        b.iter(|| {
            let params = create_params(100, 100, usable);
            if let Ok(wid) = wm.map_window(&params) {
                black_box(&wid);
                let removed = wm.unmap_window(wid);
                black_box(removed);
            }
        });
    });

    // 2. Map 20 windows in workspace
    group.bench_function("map_20_windows_batch", |b| {
        b.iter(|| {
            let mut wm = WindowManager::new(9);
            for i in 0..20_usize {
                let offset = i32::try_from(50 + i * 10).unwrap_or(50);
                let params = create_params(offset, offset, usable);
                if let Ok(wid) = wm.map_window(&params) {
                    black_box(&wid);
                }
            }
        });
    });

    // 3. Map 200 windows across workspaces
    group.bench_function("map_200_windows_distributed", |b| {
        b.iter(|| {
            let mut wm = WindowManager::new(9);
            for i in 0..200_usize {
                let ws_idx = (i % 9) + 1;
                wm.switch_workspace(ws_idx);
                let offset = i32::try_from(50 + (i % 30) * 10).unwrap_or(50);
                let params = create_params(offset, offset, usable);
                if let Ok(wid) = wm.map_window(&params) {
                    black_box(&wid);
                }
            }
        });
    });

    // 4. Switch workspace with 20 windows populated
    let mut populated_wm = WindowManager::new(9);
    for i in 0..20_usize {
        let offset = i32::try_from(50 + i * 10).unwrap_or(50);
        let params = create_params(offset, offset, usable);
        let _ = populated_wm.map_window(&params);
    }
    group.bench_function("switch_workspace_with_20_windows", |b| {
        let mut current = 1_usize;
        b.iter(|| {
            current = (current % 9) + 1;
            black_box(populated_wm.switch_workspace(black_box(current)));
        });
    });

    group.finish();
}

criterion_group!(benches, bench_layout_operations);
criterion_main!(benches);

// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Property-based tests for kx-layout reachability, margins, constraints, and invariants.

use proptest::prelude::*;

use kx_layout::{
    ExclusiveMargins, MIN_ONSCREEN_HEIGHT, MIN_ONSCREEN_WIDTH, MapWindowParams, SizeConstraints,
    WindowGeometry, WindowId, WindowManager, clamp_to_reachability,
};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    /// Verifies that clamp_to_reachability ensures windows remain grabable and visible
    /// under arbitrary window and usable bounds, with zero panics or overflows.
    #[test]
    fn reachability_clamping_property(
        wx in -10_000i32..10_000i32,
        wy in -10_000i32..10_000i32,
        ww in 10i32..5_000i32,
        wh in 10i32..5_000i32,
        ux in 0i32..1_000i32,
        uy in 0i32..1_000i32,
        uw in 300i32..4_000i32,
        uh in 300i32..4_000i32,
    ) {
        let win = WindowGeometry::new(wx, wy, ww, wh);
        let usable = WindowGeometry::new(ux, uy, uw, uh);

        let clamped = clamp_to_reachability(win, usable);

        // Top edge cannot be hidden above the top panel
        prop_assert!(clamped.y >= usable.y);

        // Window must have at least MIN_ONSCREEN_WIDTH inside horizontally
        let min_w = MIN_ONSCREEN_WIDTH.min(usable.width.max(1));
        prop_assert!(clamped.x + clamped.width >= usable.x + min_w);
        prop_assert!(clamped.x <= usable.x + usable.width - min_w);

        // Window must have at least MIN_ONSCREEN_HEIGHT inside vertically
        let min_h = MIN_ONSCREEN_HEIGHT.min(usable.height.max(1));
        prop_assert!(clamped.y <= usable.y + usable.height - min_h);
    }

    /// Verifies that exclusive margin subtraction never underflows or produces inverted rects.
    #[test]
    fn exclusive_margins_property(
        top in 0i32..500i32,
        bottom in 0i32..500i32,
        left in 0i32..500i32,
        right in 0i32..500i32,
        ow in 1i32..4000i32,
        oh in 1i32..4000i32,
    ) {
        let margins = ExclusiveMargins { top, bottom, left, right };
        let output = WindowGeometry::new(0, 0, ow, oh);
        let usable = margins.compute_usable_area(output);

        prop_assert!(usable.width >= 0);
        prop_assert!(usable.height >= 0);
        prop_assert!(usable.x >= 0);
        prop_assert!(usable.y >= 0);
        prop_assert!(usable.width <= ow);
        prop_assert!(usable.height <= oh);
    }

    /// Verifies that size constraint clamping produces dimensions >= 1 for any inputs.
    #[test]
    fn size_constraints_clamping_property(
        min_w in 0i32..2000i32,
        max_w in 0i32..2000i32,
        min_h in 0i32..2000i32,
        max_h in 0i32..2000i32,
        has_max_w in any::<bool>(),
        has_max_h in any::<bool>(),
        target_w in -100i32..5000i32,
        target_h in -100i32..5000i32,
    ) {
        let constraints = SizeConstraints {
            min_width: min_w,
            max_width: if has_max_w { Some(max_w) } else { None },
            min_height: min_h,
            max_height: if has_max_h { Some(max_h) } else { None },
        };

        let (cw, ch) = constraints.constrain(target_w, target_h);
        prop_assert!(cw >= 1);
        prop_assert!(ch >= 1);
    }

    /// Verifies that WindowManager maintains all internal invariants across sequences of ops.
    #[test]
    fn window_manager_invariants_hold(
        win_count in 1usize..15,
        target_ws in 0usize..4,
    ) {
        let mut wm = WindowManager::new(4);
        let usable = WindowGeometry::new(0, 32, 1920, 1048);
        let mut created = Vec::new();

        for i in 0..win_count {
            let title = format!("Window {i}");
            let app_id = format!("app_{i}");
            let ws_id = wm.workspaces().workspace_ids().get(target_ws % 4).copied();

            let wid = wm.map_window(&MapWindowParams {
                geometry: WindowGeometry::new(0, 0, 400, 300),
                constraints: SizeConstraints::default(),
                title: &title,
                app_id: &app_id,
                target_workspace: ws_id,
                usable_area: usable,
                focus_new_windows_cfg: true,
            });

            if let Ok(wid) = wid {
                created.push(wid);
            }
            prop_assert!(wm.check_invariants());
        }

        // Apply operations to created windows
        for (idx, &wid) in created.iter().enumerate() {
            match idx % 4 {
                0 => wm.maximize_window(wid, usable),
                1 => wm.minimize_window(wid),
                2 => wm.fullscreen_window(wid, WindowGeometry::new(0, 0, 1920, 1080)),
                _ => wm.center_window(wid, usable),
            }
            prop_assert!(wm.check_invariants());
        }

        // Unmap all created windows
        for wid in created {
            wm.unmap_window(wid);
            prop_assert!(wm.check_invariants());
        }
    }

    /// Verifies that arbitrary byte sequences exercising window manager operations
    /// never cause panics, arithmetic overflows, or invariant violations.
    #[test]
    fn arbitrary_bytes_fuzz_never_panics(data in any::<Vec<u8>>()) {
        if data.is_empty() {
            return Ok(());
        }

        let mut wm = WindowManager::new(4);
        let usable = WindowGeometry::new(0, 32, 1920, 1048);
        let output = WindowGeometry::new(0, 0, 1920, 1080);
        let mut mapped_windows: Vec<WindowId> = Vec::new();

        let chunks = data.chunks_exact(8);
        for chunk in chunks {
            let op = chunk.first().copied().unwrap_or(0) % 9;
            let x = i32::from(chunk.get(1).copied().unwrap_or(0)) * 10;
            let y = i32::from(chunk.get(2).copied().unwrap_or(0)) * 10;
            let w = (i32::from(chunk.get(3).copied().unwrap_or(1)) * 10).max(1);
            let h = (i32::from(chunk.get(4).copied().unwrap_or(1)) * 10).max(1);
            let target_ws_idx = usize::from(chunk.get(5).copied().unwrap_or(0)) % 4;

            match op {
                0 => {
                    if mapped_windows.len() < 16 {
                        let target_ws = wm.workspaces().workspace_ids().get(target_ws_idx).copied();
                        let params = MapWindowParams {
                            geometry: WindowGeometry::new(x, y, w, h),
                            constraints: SizeConstraints::default(),
                            title: "fuzz_win",
                            app_id: "fuzz_app",
                            target_workspace: target_ws,
                            usable_area: usable,
                            focus_new_windows_cfg: true,
                        };
                        if let Ok(wid) = wm.map_window(&params) {
                            mapped_windows.push(wid);
                        }
                    }
                }
                1 => {
                    if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                        wm.maximize_window(wid, usable);
                    }
                }
                2 => {
                    if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                        wm.unmaximize_window(wid, usable);
                    }
                }
                3 => {
                    if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                        wm.fullscreen_window(wid, output);
                    }
                }
                4 => {
                    if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                        wm.minimize_window(wid);
                    }
                }
                5 => {
                    if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                        wm.unminimize_window(wid, usable);
                    }
                }
                6 => {
                    if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                        wm.center_window(wid, usable);
                    }
                }
                7 => {
                    if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                        if let Some(&ws_id) = wm.workspaces().workspace_ids().get(target_ws_idx) {
                            let _ = wm.move_to_workspace(wid, ws_id);
                        }
                    }
                }
                8 if !mapped_windows.is_empty() => {
                    let remove_idx = target_ws_idx % mapped_windows.len();
                    let wid = mapped_windows.remove(remove_idx);
                    wm.unmap_window(wid);
                }
                _ => {}
            }

            prop_assert!(wm.check_invariants());
        }
    }
}

// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Fuzz target harness for `kx-layout` WindowManager operations.
//!
//! Interprets arbitrary byte streams as a sequence of window manager operations
//! (map, unmap, minimize, maximize, fullscreen, move workspace, center) and
//! asserts that internal invariants never break and no arithmetic overflows occur.
//!
//! Nightly execution:
//! ```sh
//! cargo fuzz run layout_ops
//! ```

use kx_layout::{
    MapWindowParams, SizeConstraints, WindowGeometry, WindowId, WindowManager,
};

/// Runs a sequence of window management operations derived from raw fuzz input bytes.
pub fn run_layout_fuzz(data: &[u8]) {
    if data.is_empty() {
        return;
    }

    let mut wm = WindowManager::new(4);
    let usable = WindowGeometry::new(0, 32, 1920, 1048);
    let output = WindowGeometry::new(0, 0, 1920, 1080);
    let mut mapped_windows: Vec<WindowId> = Vec::new();

    let mut chunks = data.chunks_exact(16);
    while let Some(chunk) = chunks.next() {
        let op = chunk.first().copied().unwrap_or(0) % 9;
        let x = i32::from(chunk.get(1).copied().unwrap_or(0)) * 10;
        let y = i32::from(chunk.get(2).copied().unwrap_or(0)) * 10;
        let w = (i32::from(chunk.get(3).copied().unwrap_or(1)) * 10).max(1);
        let h = (i32::from(chunk.get(4).copied().unwrap_or(1)) * 10).max(1);
        let target_ws_idx = usize::from(chunk.get(5).copied().unwrap_or(0)) % 4;

        match op {
            0 => {
                // Map window
                if mapped_windows.len() < 64 {
                    let target_ws = wm.workspaces().workspace_ids().get(target_ws_idx).copied();
                    let params = MapWindowParams {
                        geometry: WindowGeometry::new(x, y, w, h),
                        constraints: SizeConstraints::default(),
                        title: "fuzz_window",
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
                // Maximize
                if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                    wm.maximize_window(wid, usable);
                }
            }
            2 => {
                // Unmaximize
                if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                    wm.unmaximize_window(wid, usable);
                }
            }
            3 => {
                // Fullscreen
                if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                    wm.fullscreen_window(wid, output);
                }
            }
            4 => {
                // Minimize
                if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                    wm.minimize_window(wid);
                }
            }
            5 => {
                // Unminimize
                if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                    wm.unminimize_window(wid, usable);
                }
            }
            6 => {
                // Center
                if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                    wm.center_window(wid, usable);
                }
            }
            7 => {
                // Move workspace
                if let Some(&wid) = mapped_windows.get(target_ws_idx % mapped_windows.len().max(1)) {
                    if let Some(&ws_id) = wm.workspaces().workspace_ids().get(target_ws_idx) {
                        let _ = wm.move_to_workspace(wid, ws_id);
                    }
                }
            }
            8 => {
                // Unmap
                if !mapped_windows.is_empty() {
                    let remove_idx = target_ws_idx % mapped_windows.len();
                    let wid = mapped_windows.remove(remove_idx);
                    wm.unmap_window(wid);
                }
            }
            _ => {}
        }

        assert!(wm.check_invariants());
    }
}

#[cfg(fuzzing)]
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    run_layout_fuzz(data);
});

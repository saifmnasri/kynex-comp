// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for `WindowManager` lifecycle, workspace switching, and focus.

use kx_layout::manager::{MapWindowParams, WindowManager};
use kx_layout::window::{SizeConstraints, WindowGeometry, WindowState};
use kx_layout::workspace::WorkspaceError;

#[test]
fn test_window_manager_lifecycle() {
    let mut wm = WindowManager::new(4);
    let usable = WindowGeometry::new(0, 32, 1920, 1048);
    let output = WindowGeometry::new(0, 0, 1920, 1080);

    // Map window 1
    let w1 = wm
        .map_window(&MapWindowParams {
            geometry: WindowGeometry::new(0, 0, 800, 600),
            constraints: SizeConstraints::default(),
            title: "Window 1",
            app_id: "app1",
            target_workspace: None,
            usable_area: usable,
            focus_new_windows_cfg: true,
        })
        .expect("map succeeds");

    assert_eq!(wm.focus().focused(), Some(w1));
    assert_eq!(wm.stacking().topmost(), Some(w1));
    assert!(wm.check_invariants());

    // Map window 2
    let w2 = wm
        .map_window(&MapWindowParams {
            geometry: WindowGeometry::new(0, 0, 600, 400),
            constraints: SizeConstraints::default(),
            title: "Window 2",
            app_id: "app2",
            target_workspace: None,
            usable_area: usable,
            focus_new_windows_cfg: true,
        })
        .expect("map succeeds");

    assert_eq!(wm.focus().focused(), Some(w2));
    assert_eq!(wm.stacking().topmost(), Some(w2));
    assert!(wm.check_invariants());

    // Maximize w1
    wm.maximize_window(w1, usable);
    assert_eq!(wm.window(w1).map(|w| w.state), Some(WindowState::Maximized));
    assert_eq!(wm.window(w1).map(|w| w.geometry), Some(usable));

    // Unmaximize w1
    wm.unmaximize_window(w1, usable);
    assert_eq!(wm.window(w1).map(|w| w.state), Some(WindowState::Normal));

    // Minimize w2 -> focus reverts to w1
    wm.minimize_window(w2);
    assert_eq!(wm.window(w2).map(|w| w.state), Some(WindowState::Minimized));
    assert_eq!(wm.focus().focused(), Some(w1));

    // Restore w2
    wm.unminimize_window(w2, usable);
    assert_eq!(wm.window(w2).map(|w| w.state), Some(WindowState::Normal));
    assert_eq!(wm.focus().focused(), Some(w2));

    // Fullscreen w1
    wm.fullscreen_window(w1, output);
    assert_eq!(
        wm.window(w1).map(|w| w.state),
        Some(WindowState::Fullscreen)
    );
    assert_eq!(wm.window(w1).map(|w| w.geometry), Some(output));

    // Center w2
    wm.center_window(w2, usable);
    assert!(wm.check_invariants());

    // Unmap w1
    assert!(wm.unmap_window(w1).is_some());
    assert_eq!(wm.focus().focused(), Some(w2));
    assert!(wm.check_invariants());
}

#[test]
fn test_workspace_switching_and_window_moving() {
    let mut wm = WindowManager::new(4);
    let usable = WindowGeometry::new(0, 32, 1920, 1048);

    let w1 = wm
        .map_window(&MapWindowParams {
            geometry: WindowGeometry::new(100, 100, 800, 600),
            constraints: SizeConstraints::default(),
            title: "Window 1",
            app_id: "app1",
            target_workspace: None,
            usable_area: usable,
            focus_new_windows_cfg: true,
        })
        .expect("map succeeds");

    assert_eq!(
        wm.workspaces().active_workspace(),
        wm.workspaces().workspace_ids()[0]
    );
    assert_eq!(wm.focus().focused(), Some(w1));

    // Move w1 to workspace 2
    assert!(wm.move_window_to_workspace_index(w1, 2).is_ok());
    assert_eq!(wm.focus().focused(), None);
    assert_eq!(wm.visible_windows().len(), 0);

    // Switch to workspace 2
    assert!(wm.switch_workspace(2));
    assert_eq!(
        wm.workspaces().active_workspace(),
        wm.workspaces().workspace_ids()[1]
    );
    assert_eq!(wm.focus().focused(), Some(w1));
    assert_eq!(wm.visible_windows().len(), 1);

    // Invalid workspace index
    assert!(!wm.switch_workspace(99));
    assert_eq!(
        wm.move_window_to_workspace_index(w1, 99),
        Err(WorkspaceError::NotFound)
    );
}

#[test]
fn test_focus_cycling_and_recovery() {
    let mut wm = WindowManager::new(4);
    let usable = WindowGeometry::new(0, 32, 1920, 1048);

    let w1 = wm
        .map_window(&MapWindowParams {
            geometry: WindowGeometry::new(100, 100, 800, 600),
            constraints: SizeConstraints::default(),
            title: "Window 1",
            app_id: "app1",
            target_workspace: None,
            usable_area: usable,
            focus_new_windows_cfg: true,
        })
        .expect("map succeeds");

    let w2 = wm
        .map_window(&MapWindowParams {
            geometry: WindowGeometry::new(200, 200, 800, 600),
            constraints: SizeConstraints::default(),
            title: "Window 2",
            app_id: "app2",
            target_workspace: None,
            usable_area: usable,
            focus_new_windows_cfg: true,
        })
        .expect("map succeeds");

    // w2 is focused
    assert_eq!(wm.focus().focused(), Some(w2));

    // Cycle focus forward -> w1
    let next = wm.cycle_focus(false, usable);
    assert_eq!(next, Some(w1));
    assert_eq!(wm.focus().focused(), Some(w1));

    // Cycle focus forward -> w2
    let next2 = wm.cycle_focus(false, usable);
    assert_eq!(next2, Some(w2));
    assert_eq!(wm.focus().focused(), Some(w2));

    // Minimize w1
    wm.minimize_window(w1);
    assert_eq!(wm.window(w1).map(|w| w.state), Some(WindowState::Minimized));

    // Alt+Tab to w1 should unminimize it
    let cycled = wm.cycle_focus(false, usable);
    assert_eq!(cycled, Some(w1));
    assert_eq!(wm.window(w1).map(|w| w.state), Some(WindowState::Normal));
    assert_eq!(wm.focus().focused(), Some(w1));

    // Minimize w2, recover topmost window unminimizes w2 and centers it
    wm.minimize_window(w2);
    let recovered = wm.recover_topmost_window(usable);
    assert_eq!(recovered, Some(w2));
    assert_eq!(wm.window(w2).map(|w| w.state), Some(WindowState::Normal));
    assert_eq!(wm.focus().focused(), Some(w2));
}

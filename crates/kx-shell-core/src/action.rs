// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Compositor action dispatch and execution.
//!
//! Maps [`kx_input::CompositorAction`] events to concrete window manager,
//! seat focus, and protocol state modifications.

use kx_input::CompositorAction;
use kx_layout::WindowId;
use kx_layout::window::WindowState;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::Resource;

use crate::state::KynexState;
use crate::window::{
    handle_fullscreen_request, handle_maximize_request, handle_unfullscreen_request,
    handle_unmaximize_request, primary_output_usable_geometry, sync_layout_to_space,
};

/// Activates a window, updates its XDG toplevel state, and sets seat keyboard focus.
pub fn activate_and_focus_window(state: &mut KynexState, wid: WindowId) {
    let prev_focused = state.window_manager.focus().focused();
    if prev_focused != Some(wid) {
        if let Some(prev_wid) = prev_focused {
            if let Some(prev_win) = state.window_map.get(&prev_wid) {
                if let Some(toplevel) = prev_win.toplevel() {
                    toplevel.with_pending_state(|s| {
                        s.states.unset(xdg_toplevel::State::Activated);
                    });
                    toplevel.send_configure();
                }
            }
        }
    }

    state.window_manager.focus_window(wid);

    if let Some(win) = state.window_map.get(&wid) {
        if let Some(toplevel) = win.toplevel() {
            toplevel.with_pending_state(|s| {
                s.states.set(xdg_toplevel::State::Activated);
            });
            toplevel.send_configure();

            if let Some(keyboard) = state.seat.get_keyboard() {
                let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                keyboard.set_focus(state, Some(toplevel.wl_surface().clone()), serial);
            }
        }
    }
}

/// Clears window activation and keyboard focus.
pub fn clear_window_focus(state: &mut KynexState) {
    if let Some(prev_wid) = state.window_manager.focus().focused() {
        if let Some(prev_win) = state.window_map.get(&prev_wid) {
            if let Some(toplevel) = prev_win.toplevel() {
                toplevel.with_pending_state(|s| {
                    s.states.unset(xdg_toplevel::State::Activated);
                });
                toplevel.send_configure();
            }
        }
    }

    if let Some(keyboard) = state.seat.get_keyboard() {
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        keyboard.set_focus(state, None, serial);
    }
}

fn handle_toggle_maximize(state: &mut KynexState) {
    let Some(wid) = state.window_manager.focus().focused() else {
        return;
    };
    let is_maximized = state
        .window_manager
        .window(wid)
        .is_some_and(|w| w.state == WindowState::Maximized);

    let Some(win) = state.window_map.get(&wid).cloned() else {
        return;
    };
    let Some(toplevel) = win.toplevel() else {
        return;
    };

    if is_maximized {
        handle_unmaximize_request(state, toplevel);
    } else {
        handle_maximize_request(state, toplevel);
    }
}

fn handle_toggle_fullscreen(state: &mut KynexState) {
    let Some(wid) = state.window_manager.focus().focused() else {
        return;
    };
    let is_fullscreen = state
        .window_manager
        .window(wid)
        .is_some_and(|w| w.state == WindowState::Fullscreen);

    let Some(win) = state.window_map.get(&wid).cloned() else {
        return;
    };
    let Some(toplevel) = win.toplevel() else {
        return;
    };

    if is_fullscreen {
        handle_unfullscreen_request(state, toplevel);
    } else {
        handle_fullscreen_request(state, toplevel, None);
    }
}

fn handle_minimize_action(state: &mut KynexState) {
    let Some(wid) = state.window_manager.focus().focused() else {
        return;
    };
    if let Some(win) = state.window_map.get(&wid) {
        if let Some(toplevel) = win.toplevel() {
            toplevel.with_pending_state(|s| {
                s.states.unset(xdg_toplevel::State::Activated);
            });
            toplevel.send_configure();
        }
    }
    state.window_manager.minimize_window(wid);
    let next_focus = state.window_manager.focus().focused();
    if let Some(next_wid) = next_focus {
        activate_and_focus_window(state, next_wid);
    } else {
        clear_window_focus(state);
    }
    sync_layout_to_space(state);
}

fn handle_cycle_focus(state: &mut KynexState, backward: bool) {
    let usable = primary_output_usable_geometry(state);
    if let Some(new_wid) = state.window_manager.cycle_focus(backward, usable) {
        activate_and_focus_window(state, new_wid);
        sync_layout_to_space(state);
    }
}

fn handle_recover_topmost(state: &mut KynexState) {
    let usable = primary_output_usable_geometry(state);
    if let Some(recovered_wid) = state.window_manager.recover_topmost_window(usable) {
        activate_and_focus_window(state, recovered_wid);
        sync_layout_to_space(state);
    }
}

fn handle_workspace_switch(state: &mut KynexState, index: u32) {
    let Ok(idx) = usize::try_from(index) else {
        return;
    };
    if state.window_manager.switch_workspace(idx) {
        if let Some(wid) = state.window_manager.focus().focused() {
            activate_and_focus_window(state, wid);
        } else {
            clear_window_focus(state);
        }
        sync_layout_to_space(state);
    }
}

fn handle_move_to_workspace(state: &mut KynexState, index: u32) {
    let Some(wid) = state.window_manager.focus().focused() else {
        return;
    };
    let Ok(idx) = usize::try_from(index) else {
        return;
    };
    if state
        .window_manager
        .move_window_to_workspace_index(wid, idx)
        .is_ok()
    {
        if let Some(next_wid) = state.window_manager.focus().focused() {
            activate_and_focus_window(state, next_wid);
        } else {
            clear_window_focus(state);
        }
        sync_layout_to_space(state);
    }
}

fn handle_close_window(state: &mut KynexState) {
    let Some(wid) = state.window_manager.focus().focused() else {
        return;
    };
    if let Some(win) = state.window_map.get(&wid) {
        if let Some(toplevel) = win.toplevel() {
            toplevel.send_close();
        }
    }
}

fn handle_kill_client(state: &mut KynexState) {
    let Some(wid) = state.window_manager.focus().focused() else {
        return;
    };
    if let Some(win) = state.window_map.get(&wid) {
        if let Some(toplevel) = win.toplevel() {
            tracing::info!(
                ?wid,
                "Terminating client connection via fatal protocol error"
            );
            toplevel
                .wl_surface()
                .post_error(1u32, "Terminated by compositor user action");
        }
    }
}

/// Executes a compositor action triggered by keybindings or compositor policy.
///
/// Returns `true` if the compositor was requested to quit, or `false` otherwise.
pub fn execute_compositor_action(state: &mut KynexState, action: &CompositorAction) -> bool {
    match action {
        CompositorAction::Quit => {
            tracing::info!("Compositor shutdown requested via action");
            true
        }
        CompositorAction::CloseWindow => {
            handle_close_window(state);
            false
        }
        CompositorAction::KillClient => {
            handle_kill_client(state);
            false
        }
        CompositorAction::ToggleMaximize => {
            handle_toggle_maximize(state);
            false
        }
        CompositorAction::ToggleFullscreen => {
            handle_toggle_fullscreen(state);
            false
        }
        CompositorAction::MinimizeWindow => {
            handle_minimize_action(state);
            false
        }
        CompositorAction::CycleFocus => {
            handle_cycle_focus(state, false);
            false
        }
        CompositorAction::CycleFocusBackward => {
            handle_cycle_focus(state, true);
            false
        }
        CompositorAction::RecoverTopmostWindow => {
            handle_recover_topmost(state);
            false
        }
        CompositorAction::SwitchWorkspace(idx) => {
            handle_workspace_switch(state, *idx);
            false
        }
        CompositorAction::MoveToWorkspace(idx) => {
            handle_move_to_workspace(state, *idx);
            false
        }
        CompositorAction::Custom(name) => {
            tracing::debug!(action = %name, "Custom compositor action unhandled");
            false
        }
        _ => false,
    }
}

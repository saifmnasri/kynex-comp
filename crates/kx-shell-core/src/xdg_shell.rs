// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! XDG Shell protocol implementation and delegate.

use smithay::delegate_xdg_shell;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::Serial;
use smithay::wayland::shell::xdg::{
    Configure, PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};

use crate::state::KynexState;

impl XdgShellHandler for KynexState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        crate::window::map_new_toplevel(self, &surface);
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        crate::window::handle_new_popup(self, &surface, positioner);
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: WlSeat, serial: Serial) {
        crate::grab::handle_client_move_request(self, &surface, &seat, serial);
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: WlSeat,
        serial: Serial,
        edges: ResizeEdge,
    ) {
        crate::grab::handle_client_resize_request(self, &surface, &seat, serial, edges);
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        _positioner: PositionerState,
        token: u32,
    ) {
        surface.send_repositioned(token);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        crate::window::unmap_toplevel(self, &surface);
    }

    fn popup_destroyed(&mut self, _surface: PopupSurface) {
        self.popups.cleanup();
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        crate::window::handle_maximize_request(self, &surface);
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        crate::window::handle_unmaximize_request(self, &surface);
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, output: Option<WlOutput>) {
        crate::window::handle_fullscreen_request(self, &surface, output);
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        crate::window::handle_unfullscreen_request(self, &surface);
    }

    fn minimize_request(&mut self, surface: ToplevelSurface) {
        crate::window::handle_minimize_request(self, &surface);
    }

    fn ack_configure(&mut self, surface: WlSurface, configure: Configure) {
        crate::window::handle_ack_configure(self, &surface, configure);
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        crate::window::handle_title_changed(self, &surface);
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        crate::window::handle_app_id_changed(self, &surface);
    }
}

delegate_xdg_shell!(KynexState);

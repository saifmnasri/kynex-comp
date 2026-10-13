// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Root compositor state and Wayland protocol delegates.

use std::collections::HashMap;
use std::sync::OnceLock;

use kx_layout::{DEFAULT_WORKSPACE_COUNT, WindowId, WindowManager};
use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::desktop::{PopupKind, PopupManager, Space, Window};
use smithay::input::keyboard::XkbConfig;
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DisplayHandle, Resource};
use smithay::utils::{Logical, Physical, Point, Serial, Size, Transform};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{CompositorClientState, CompositorHandler, CompositorState};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{
    delegate_compositor, delegate_output, delegate_seat, delegate_shm, delegate_xdg_shell,
};

use crate::client::ClientState;
use crate::error::ShellError;

/// Root state holding core protocol handlers and compositor resources.
pub struct KynexState {
    /// Wayland compositor protocol state (`wl_compositor` and `wl_subcompositor`).
    pub compositor_state: CompositorState,
    /// Shared memory protocol state (`wl_shm`).
    pub shm_state: ShmState,
    /// Seat management protocol state (`wl_seat`).
    pub seat_state: SeatState<Self>,
    /// Primary input seat.
    pub seat: Seat<Self>,
    /// Primary nested output.
    pub output: Output,
    /// Bound socket name (e.g. `wayland-1`).
    pub socket_name: String,
    /// XDG Shell protocol state (`xdg_wm_base`).
    pub xdg_shell_state: XdgShellState,
    /// 2D space managing window placement and stacking.
    pub space: Space<Window>,
    /// Popup manager tracking popup relationships.
    pub popups: PopupManager,
    /// Active compositor key binding matcher.
    pub binding_matcher: kx_input::BindingMatcher,
    /// Authoritative window manager coordinating workspaces, stacking, and layout.
    pub window_manager: WindowManager,
    /// Mapping from layout [`WindowId`] to Smithay desktop [`Window`].
    pub window_map: HashMap<WindowId, Window>,
    /// Reverse lookup mapping from [`WlSurface`] to layout [`WindowId`].
    pub surface_to_window_id: HashMap<WlSurface, WindowId>,
}

impl KynexState {
    /// Creates a new compositor state with core protocol globals and the specified output.
    ///
    /// # Errors
    ///
    /// Returns a [`ShellError`] if seat input device initialization fails.
    pub fn new_with_output(
        dh: &DisplayHandle,
        socket_name: String,
        output: Output,
    ) -> Result<Self, ShellError> {
        let compositor_state = CompositorState::new::<Self>(dh);
        let shm_state = ShmState::new::<Self>(dh, vec![]);
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(dh, "kynex-seat-0");

        // Add standard pointer and keyboard to the default seat
        let _pointer = seat.add_pointer();
        let _keyboard = seat.add_keyboard(XkbConfig::default(), 200, 200)?;

        let _global = output.create_global::<Self>(dh);

        let xdg_shell_state = XdgShellState::new::<Self>(dh);
        let mut space = Space::default();
        space.map_output(&output, (0, 0));
        let popups = PopupManager::default();
        let binding_matcher = kx_input::BindingMatcher::with_defaults();
        let window_manager = WindowManager::new(DEFAULT_WORKSPACE_COUNT);
        let window_map = HashMap::new();
        let surface_to_window_id = HashMap::new();

        Ok(Self {
            compositor_state,
            shm_state,
            seat_state,
            seat,
            output,
            socket_name,
            xdg_shell_state,
            space,
            popups,
            binding_matcher,
            window_manager,
            window_map,
            surface_to_window_id,
        })
    }

    /// Creates a new compositor state with core protocol globals and a default nested output.
    ///
    /// # Errors
    ///
    /// Returns a [`ShellError`] if seat input device initialization fails.
    pub fn new(
        dh: &DisplayHandle,
        socket_name: String,
        initial_size: Size<i32, Physical>,
    ) -> Result<Self, ShellError> {
        let output = Output::new(
            "WL-1".to_string(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "Kynex".to_string(),
                model: "Nested-1".to_string(),
            },
        );

        let mode = Mode {
            size: initial_size,
            refresh: 60_000,
        };
        output.set_preferred(mode);
        output.change_current_state(
            Some(mode),
            // The nested winit backend renders into an EGLSurface window rather than
            // a DRM KMS scanout plane. In Smithay's GlesRenderer, an internal flip180
            // matrix is applied to compensate for DRM hardware scanout conventions.
            // Using Transform::Flipped180 cancels this internal flip so that the nested
            // window displays right-side up.
            Some(Transform::Flipped180),
            Some(Scale::Integer(1)),
            Some(Point::from((0, 0))),
        );

        Self::new_with_output(dh, socket_name, output)
    }

    /// Updates the output geometry when the nested window is resized.
    pub fn update_output_size(&self, new_size: Size<i32, Physical>) {
        let mode = Mode {
            size: new_size,
            refresh: 60_000,
        };
        self.output
            .change_current_state(Some(mode), None, None, None);
    }

    /// Maps an additional display output into the compositor's global space.
    pub fn map_output(
        &mut self,
        dh: &DisplayHandle,
        output: &Output,
        location: impl Into<Point<i32, Logical>>,
    ) {
        let _global = output.create_global::<Self>(dh);
        self.space.map_output(output, location);
    }

    /// Unmaps a display output from the compositor's global space.
    pub fn unmap_output(&mut self, output: &Output) {
        self.space.unmap_output(output);
    }

    /// Migrates windows away from an unplugged output so they remain accessible on surviving outputs.
    pub fn migrate_windows_from_unplugged(
        &mut self,
        unplugged_origin: Point<i32, Logical>,
        unplugged_size: Size<i32, Logical>,
        primary_origin: Point<i32, Logical>,
        primary_size: Size<i32, Logical>,
    ) {
        let windows: Vec<(Window, Point<i32, Logical>)> = self
            .space
            .elements()
            .filter_map(|w| self.space.element_location(w).map(|loc| (w.clone(), loc)))
            .collect();

        for (window, loc) in windows {
            let new_loc = crate::drm::layout::migrate_window_position(
                loc,
                unplugged_origin,
                unplugged_size,
                primary_origin,
                primary_size,
            );
            if new_loc != loc {
                self.space.map_element(window, new_loc, false);
            }
        }
    }
}

impl BufferHandler for KynexState {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl CompositorHandler for KynexState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        if let Some(client_data) = client.get_data::<ClientState>() {
            &client_data.compositor_state
        } else {
            static FALLBACK_CLIENT_STATE: OnceLock<CompositorClientState> = OnceLock::new();
            FALLBACK_CLIENT_STATE.get_or_init(CompositorClientState::default)
        }
    }

    fn new_surface(&mut self, surface: &WlSurface) {
        if let Some(client) = surface.client() {
            if let Some(client_data) = client.get_data::<ClientState>() {
                if !client_data.try_increment_surface() {
                    tracing::warn!(
                        "Client exceeded maximum surface quota ({}); disconnecting",
                        client_data.limits.max_surfaces
                    );
                    // 2 corresponds to wl_display::Error::NoMemory in the Wayland protocol specification
                    surface.post_error(2u32, "Maximum surface quota exceeded");
                }
            }
        }
    }

    fn destroyed(&mut self, surface: &WlSurface) {
        if let Some(client) = surface.client() {
            if let Some(client_data) = client.get_data::<ClientState>() {
                client_data.decrement_surface();
            }
        }
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        self.popups.commit(surface);
        for window in self.space.elements() {
            let mut belongs = false;
            window.with_surfaces(|s, _| {
                if s == surface {
                    belongs = true;
                }
            });
            if belongs {
                window.on_commit();
                break;
            }
        }
        if let Some(&wid) = self.surface_to_window_id.get(surface) {
            if let Some(window) = self.window_map.get(&wid) {
                if let Some(win_data) = self.window_manager.window_mut(wid) {
                    if win_data.state == kx_layout::WindowState::Normal {
                        let geo = window.geometry();
                        if geo.size.w > 0 && geo.size.h > 0 {
                            let (cw, ch) = win_data.constraints.constrain(geo.size.w, geo.size.h);
                            win_data.geometry.width = cw;
                            win_data.geometry.height = ch;
                        }
                    }
                }
            }
        }
        self.space.refresh();
    }
}

impl ShmHandler for KynexState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SeatHandler for KynexState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {}

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: CursorImageStatus) {}
}

impl OutputHandler for KynexState {
    fn output_bound(&mut self, _output: Output, _wl_output: WlOutput) {}
}

impl XdgShellHandler for KynexState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        crate::window::map_new_toplevel(self, &surface);
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        if let Err(err) = self.popups.track_popup(PopupKind::from(surface.clone())) {
            tracing::warn!(?err, "Failed to track popup in popup manager");
            return;
        }
        if let Err(err) = surface.send_configure() {
            tracing::warn!(?err, "Failed to send initial popup configure");
        }
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: WlSeat, serial: Serial) {
        crate::grab::handle_client_move_request(self, &surface, &seat, serial);
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: WlSeat,
        serial: Serial,
        edges: smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge,
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

    fn ack_configure(
        &mut self,
        surface: WlSurface,
        configure: smithay::wayland::shell::xdg::Configure,
    ) {
        crate::window::handle_ack_configure(self, &surface, configure);
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        crate::window::handle_title_changed(self, &surface);
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        crate::window::handle_app_id_changed(self, &surface);
    }
}

delegate_compositor!(KynexState);
delegate_shm!(KynexState);
delegate_seat!(KynexState);
delegate_output!(KynexState);
delegate_xdg_shell!(KynexState);

#[cfg(test)]
mod tests {
    use smithay::reexports::wayland_server::Display;

    use super::*;

    #[test]
    fn test_kynex_state_initialization() {
        let display: Display<KynexState> = Display::new().unwrap();
        let initial_size = Size::from((1280, 720));
        let state = KynexState::new(
            &display.handle(),
            "wayland-test-0".to_string(),
            initial_size,
        )
        .unwrap();

        assert_eq!(state.socket_name, "wayland-test-0");
        let current_mode = state.output.current_mode().unwrap();
        assert_eq!(current_mode.size, initial_size);
        assert_eq!(current_mode.refresh, 60_000);
        assert!(matches!(state.output.current_scale(), Scale::Integer(1)));
        assert_eq!(
            state.output.current_transform(),
            Transform::Flipped180,
            "Nested output must be Flipped180 to cancel GlesRenderer OpenGL scanout inversion"
        );

        let new_size = Size::from((1920, 1080));
        state.update_output_size(new_size);
        let updated_mode = state.output.current_mode().unwrap();
        assert_eq!(updated_mode.size, new_size);
    }

    #[test]
    fn test_binding_matcher_emergency_exit() {
        let display = Display::<KynexState>::new().unwrap();
        let state = KynexState::new(
            &display.handle(),
            "wayland-test-exit".to_string(),
            Size::from((800, 600)),
        )
        .unwrap();

        let action = state.binding_matcher.matches(
            kx_input::KeyModifiers::CTRL | kx_input::KeyModifiers::SHIFT,
            kx_input::KeySym::ESCAPE,
        );
        assert_eq!(action, Some(&kx_input::CompositorAction::Quit));
    }

    #[test]
    fn test_map_unmap_output_and_migrate() {
        use smithay::output::{PhysicalProperties, Subpixel};

        let display = Display::<KynexState>::new().unwrap();
        let mut state = KynexState::new(
            &display.handle(),
            "wayland-test-multi".to_string(),
            Size::from((1280, 720)),
        )
        .unwrap();

        assert_eq!(state.space.outputs().count(), 1);

        let secondary = Output::new(
            "DRM-2".to_string(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "Kynex".to_string(),
                model: "HDMI".to_string(),
            },
        );
        let sec_mode = smithay::output::Mode {
            size: Size::from((1920, 1080)),
            refresh: 60_000,
        };
        secondary.set_preferred(sec_mode);
        secondary.change_current_state(
            Some(sec_mode),
            Some(Transform::Normal),
            Some(Scale::Integer(1)),
            Some(Point::from((1280, 0))),
        );

        state.map_output(&display.handle(), &secondary, Point::from((1280, 0)));
        assert_eq!(state.space.outputs().count(), 2);

        state.unmap_output(&secondary);
        assert_eq!(state.space.outputs().count(), 1);

        state.migrate_windows_from_unplugged(
            Point::from((1280, 0)),
            Size::from((1920, 1080)),
            Point::from((0, 0)),
            Size::from((1280, 720)),
        );
        assert_eq!(state.space.outputs().count(), 1);
    }
}

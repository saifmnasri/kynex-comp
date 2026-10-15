// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Root compositor state and Wayland protocol delegates.

use std::collections::HashMap;
use std::sync::OnceLock;

use kx_layout::{DEFAULT_WORKSPACE_COUNT, WindowId, WindowManager};
use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::desktop::{PopupManager, Space, Window};
use smithay::input::keyboard::XkbConfig;
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DisplayHandle, Resource};
use smithay::utils::{Logical, Physical, Point, Size, Transform};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{CompositorClientState, CompositorHandler, CompositorState};
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufState};
use smithay::wayland::drm_syncobj::DrmSyncobjState;
use smithay::wayland::output::OutputHandler;
use smithay::wayland::shell::wlr_layer::WlrLayerShellState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::wayland::viewporter::ViewporterState;
use smithay::wayland::xdg_activation::XdgActivationState;
use smithay::{
    delegate_compositor, delegate_output, delegate_seat, delegate_shm, delegate_xdg_decoration,
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
    /// Data device protocol state (`wl_data_device_manager`).
    pub data_device_state: smithay::wayland::selection::data_device::DataDeviceState,
    /// Server-side decoration protocol state (`zxdg_decoration_manager_v1`).
    pub xdg_decoration_state: XdgDecorationState,
    /// Current pointer cursor image status (requested by client or compositor).
    pub cursor_status: CursorImageStatus,
    /// Layer shell protocol state (`zwlr_layer_shell_v1`).
    pub layer_shell_state: WlrLayerShellState,
    /// Display handle for Wayland server.
    pub display_handle: DisplayHandle,
    /// XDG activation protocol state (`xdg_activation_v1`).
    pub xdg_activation_state: XdgActivationState,
    /// Linux DMA-BUF protocol state (`zwp_linux_dmabuf_v1`).
    pub dmabuf_state: DmabufState,
    /// Handle to registered dmabuf global.
    pub dmabuf_global: Option<DmabufGlobal>,
    /// DRM syncobj explicit synchronization protocol state (`wp_linux_drm_syncobj_manager_v1`).
    pub drm_syncobj_state: Option<DrmSyncobjState>,
    /// Viewport protocol state (`wp_viewporter`).
    pub viewporter_state: ViewporterState,
    /// Fractional scale protocol state (`wp_fractional_scale_v1`).
    pub fractional_scale_manager_state:
        smithay::wayland::fractional_scale::FractionalScaleManagerState,
    /// Cursor shape protocol state (`wp_cursor_shape_v1`).
    pub cursor_shape_manager_state: smithay::wayland::cursor_shape::CursorShapeManagerState,
    /// Pointer constraints protocol state (`zwp_pointer_constraints_v1`).
    pub pointer_constraints_state: smithay::wayland::pointer_constraints::PointerConstraintsState,
    /// Relative pointer protocol state (`zwp_relative_pointer_v1`).
    pub relative_pointer_state: smithay::wayland::relative_pointer::RelativePointerManagerState,
    /// Compositor configuration.
    pub config: kx_config::Config,
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

        let data_device_state =
            smithay::wayland::selection::data_device::DataDeviceState::new::<Self>(dh);
        let xdg_decoration_state = XdgDecorationState::new::<Self>(dh);
        let cursor_status = CursorImageStatus::default_named();
        let xdg_shell_state = XdgShellState::new::<Self>(dh);
        let mut space = Space::default();
        space.map_output(&output, (0, 0));
        let popups = PopupManager::default();
        let binding_matcher = kx_input::BindingMatcher::with_defaults();
        let window_manager = WindowManager::new(DEFAULT_WORKSPACE_COUNT);
        let window_map = HashMap::new();
        let surface_to_window_id = HashMap::new();
        let config = kx_config::Config::default();
        let layer_shell_state = create_layer_shell_state(dh);
        let display_handle = dh.clone();
        let xdg_activation_state = XdgActivationState::new::<Self>(dh);
        let mut dmabuf_state = DmabufState::new();
        let dmabuf_global = crate::dmabuf::init_dmabuf_global(dh, &mut dmabuf_state, None, None);
        let drm_syncobj_state = crate::syncobj::init_drm_syncobj_global(dh, None);
        let viewporter_state = crate::viewporter::init_viewporter_global(dh);
        let fractional_scale_manager_state =
            crate::fractional_scale::init_fractional_scale_global(dh);
        let cursor_shape_manager_state = crate::cursor_shape::init_cursor_shape_global(dh);
        let pointer_constraints_state =
            crate::pointer_constraints::init_pointer_constraints_global(dh);
        let relative_pointer_state = crate::relative_pointer::init_relative_pointer_global(dh);

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
            data_device_state,
            xdg_decoration_state,
            cursor_status,
            layer_shell_state,
            display_handle,
            xdg_activation_state,
            dmabuf_state,
            dmabuf_global,
            drm_syncobj_state,
            viewporter_state,
            fractional_scale_manager_state,
            cursor_shape_manager_state,
            pointer_constraints_state,
            relative_pointer_state,
            config,
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
            // Flipped180 cancels GlesRenderer's DRM scanout flip so nested EGL window is upright.
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
        self.space.refresh();
    }

    /// Unmaps a display output from the compositor's global space.
    pub fn unmap_output(&mut self, output: &Output) {
        let mut layer_map = smithay::desktop::layer_map_for_output(output);
        let layers: Vec<_> = layer_map.layers().cloned().collect();
        for layer in layers {
            layer_map.unmap_layer(&layer);
        }
        self.space.unmap_output(output);
        self.space.refresh();
    }

    /// Migrates windows away from an unplugged output so they remain accessible on surviving outputs.
    pub fn migrate_windows_from_unplugged(
        &mut self,
        unplugged_origin: Point<i32, Logical>,
        unplugged_size: Size<i32, Logical>,
        primary_origin: Point<i32, Logical>,
        primary_size: Size<i32, Logical>,
    ) {
        let windows: Vec<(WindowId, Point<i32, Logical>)> = self
            .window_map
            .keys()
            .copied()
            .filter_map(|wid| {
                self.window_manager
                    .window(wid)
                    .map(|w| (wid, Point::from((w.geometry.x, w.geometry.y))))
            })
            .collect();

        for (wid, loc) in windows {
            let new_loc = crate::drm::layout::migrate_window_position(
                loc,
                unplugged_origin,
                unplugged_size,
                primary_origin,
                primary_size,
            );
            if new_loc != loc {
                if let Some(win) = self.window_manager.window(wid) {
                    let mut geom = win.geometry;
                    geom.x = new_loc.x;
                    geom.y = new_loc.y;
                    self.window_manager.set_window_geometry(wid, geom);
                }
            }
        }

        crate::window::sync_layout_to_space(self);
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
        for output in self.space.outputs() {
            let mut layer_map = smithay::desktop::layer_map_for_output(output);
            if layer_map
                .layer_for_surface(surface, smithay::desktop::WindowSurfaceType::all())
                .is_some()
            {
                layer_map.arrange();
                break;
            }
        }
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

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.cursor_status = image;
    }
}

impl OutputHandler for KynexState {
    fn output_bound(&mut self, _output: Output, _wl_output: WlOutput) {}
}

delegate_compositor!(KynexState);
delegate_shm!(KynexState);
delegate_seat!(KynexState);
delegate_output!(KynexState);
delegate_xdg_decoration!(KynexState);

fn create_layer_shell_state(dh: &DisplayHandle) -> WlrLayerShellState {
    WlrLayerShellState::new_with_filter::<KynexState, _>(dh, |client| {
        client
            .get_data::<crate::client::ClientState>()
            .is_some_and(crate::client::ClientState::is_layer_shell_allowed)
    })
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;

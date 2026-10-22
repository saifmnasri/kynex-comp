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
use smithay::utils::{Physical, Point, Size, Transform};
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
    /// Text input manager protocol state (`zwp_text_input_v3`).
    pub text_input_state: smithay::wayland::text_input::TextInputManagerState,
    /// Input method manager protocol state (`zwp_input_method_v2`).
    pub input_method_state: smithay::wayland::input_method::InputMethodManagerState,
    /// Wayland session lock protocol state (`ext_session_lock_v1`).
    pub session_lock_state: smithay::wayland::session_lock::SessionLockManagerState,
    /// Whether the session is currently locked.
    pub is_session_locked: bool,
    /// Active lock screen surfaces.
    pub lock_surfaces: Vec<smithay::wayland::session_lock::LockSurface>,
    /// Foreign toplevel list protocol state (`ext_foreign_toplevel_list_v1`).
    pub foreign_toplevel_list_state:
        smithay::wayland::foreign_toplevel_list::ForeignToplevelListState,
    /// Active foreign toplevel handles indexed by window id.
    pub foreign_toplevel_handles: std::collections::HashMap<
        kx_layout::WindowId,
        smithay::wayland::foreign_toplevel_list::ForeignToplevelHandle,
    >,
    /// Presentation time protocol state (`wp_presentation`).
    pub presentation_state: smithay::wayland::presentation::PresentationState,
    /// Idle inhibitor protocol state (`zwp_idle_inhibit_v1`).
    pub idle_inhibit_state: smithay::wayland::idle_inhibit::IdleInhibitManagerState,
    /// Surfaces holding active idle inhibitors.
    pub idle_inhibitors: Vec<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>,
    /// Primary selection protocol state (`zwp_primary_selection_device_manager_v1`).
    pub primary_selection_state:
        smithay::wayland::selection::primary_selection::PrimarySelectionState,
    /// Single-pixel buffer protocol state (`wp_single_pixel_buffer_manager_v1`).
    pub single_pixel_buffer_state: smithay::wayland::single_pixel_buffer::SinglePixelBufferState,
    /// Content type protocol state (`wp_content_type_manager_v1`).
    pub content_type_state: smithay::wayland::content_type::ContentTypeState,
    /// Tearing control protocol state (`wp_tearing_control_manager_v1`).
    pub tearing_control_state: crate::tearing_control::TearingControlState,
    /// Idle notifier protocol state (`ext_idle_notifier_v1`).
    pub idle_notifier_state: smithay::wayland::idle_notify::IdleNotifierState<Self>,
    /// Dedicated internal event loop for idle timers.
    pub idle_loop: Option<smithay::reexports::calloop::EventLoop<'static, Self>>,
    /// Alpha modifier protocol state (`wp_alpha_modifier_v1`).
    pub alpha_modifier_state: smithay::wayland::alpha_modifier::AlphaModifierState,
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
        let (seat_state, seat) = init_seat(dh)?;
        let _global = output.create_global::<Self>(dh);
        let mut space = Space::default();
        space.map_output(&output, (0, 0));
        let proto = init_protocol_globals(dh)?;

        Ok(Self {
            compositor_state: CompositorState::new::<Self>(dh),
            shm_state: ShmState::new::<Self>(dh, vec![]),
            seat_state,
            seat,
            output,
            socket_name,
            xdg_shell_state: XdgShellState::new::<Self>(dh),
            space,
            popups: PopupManager::default(),
            binding_matcher: kx_input::BindingMatcher::with_defaults(),
            window_manager: WindowManager::new(DEFAULT_WORKSPACE_COUNT),
            window_map: HashMap::new(),
            surface_to_window_id: HashMap::new(),
            data_device_state: smithay::wayland::selection::data_device::DataDeviceState::new::<Self>(
                dh,
            ),
            xdg_decoration_state: XdgDecorationState::new::<Self>(dh),
            cursor_status: CursorImageStatus::default_named(),
            layer_shell_state: create_layer_shell_state(dh),
            display_handle: dh.clone(),
            xdg_activation_state: XdgActivationState::new::<Self>(dh),
            dmabuf_state: proto.dmabuf_state,
            dmabuf_global: proto.dmabuf_global,
            drm_syncobj_state: proto.drm_syncobj_state,
            viewporter_state: proto.viewporter_state,
            fractional_scale_manager_state: proto.fractional_scale_manager_state,
            cursor_shape_manager_state: proto.cursor_shape_manager_state,
            pointer_constraints_state: proto.pointer_constraints_state,
            relative_pointer_state: proto.relative_pointer_state,
            text_input_state: proto.text_input_state,
            input_method_state: proto.input_method_state,
            session_lock_state: proto.session_lock_state,
            is_session_locked: false,
            lock_surfaces: Vec::new(),
            foreign_toplevel_list_state: proto.foreign_toplevel_list_state,
            foreign_toplevel_handles: HashMap::new(),
            presentation_state: proto.presentation_state,
            idle_inhibit_state: proto.idle_inhibit_state,
            idle_inhibitors: Vec::new(),
            primary_selection_state: proto.primary_selection_state,
            single_pixel_buffer_state: proto.single_pixel_buffer_state,
            content_type_state: proto.content_type_state,
            tearing_control_state: proto.tearing_control_state,
            idle_notifier_state: proto.idle_notifier_state,
            idle_loop: Some(proto.idle_loop),
            alpha_modifier_state: proto.alpha_modifier_state,
            config: kx_config::Config::default(),
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

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        use smithay::reexports::wayland_server::Resource;
        use smithay::wayland::text_input::TextInputSeat;
        let text_input = seat.text_input();
        text_input.leave();
        text_input.set_focus(focused.cloned());
        text_input.enter();

        let client = focused.and_then(Resource::client);
        smithay::wayland::selection::data_device::set_data_device_focus(
            &self.display_handle,
            seat,
            client.clone(),
        );
        smithay::wayland::selection::primary_selection::set_primary_focus(
            &self.display_handle,
            seat,
            client,
        );

        if let (Some(surface), Some(pointer)) = (focused, seat.get_pointer()) {
            crate::pointer_constraints::maybe_activate_pointer_constraint(surface, &pointer);
        }
    }

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

struct ProtocolGlobals {
    dmabuf_state: DmabufState,
    dmabuf_global: Option<DmabufGlobal>,
    drm_syncobj_state: Option<DrmSyncobjState>,
    viewporter_state: ViewporterState,
    fractional_scale_manager_state: smithay::wayland::fractional_scale::FractionalScaleManagerState,
    cursor_shape_manager_state: smithay::wayland::cursor_shape::CursorShapeManagerState,
    pointer_constraints_state: smithay::wayland::pointer_constraints::PointerConstraintsState,
    relative_pointer_state: smithay::wayland::relative_pointer::RelativePointerManagerState,
    text_input_state: smithay::wayland::text_input::TextInputManagerState,
    input_method_state: smithay::wayland::input_method::InputMethodManagerState,
    session_lock_state: smithay::wayland::session_lock::SessionLockManagerState,
    foreign_toplevel_list_state: smithay::wayland::foreign_toplevel_list::ForeignToplevelListState,
    presentation_state: smithay::wayland::presentation::PresentationState,
    idle_inhibit_state: smithay::wayland::idle_inhibit::IdleInhibitManagerState,
    primary_selection_state: smithay::wayland::selection::primary_selection::PrimarySelectionState,
    single_pixel_buffer_state: smithay::wayland::single_pixel_buffer::SinglePixelBufferState,
    content_type_state: smithay::wayland::content_type::ContentTypeState,
    tearing_control_state: crate::tearing_control::TearingControlState,
    idle_notifier_state: smithay::wayland::idle_notify::IdleNotifierState<KynexState>,
    idle_loop: smithay::reexports::calloop::EventLoop<'static, KynexState>,
    alpha_modifier_state: smithay::wayland::alpha_modifier::AlphaModifierState,
}

fn init_protocol_globals(dh: &DisplayHandle) -> Result<ProtocolGlobals, ShellError> {
    let mut dmabuf_state = DmabufState::new();
    let dmabuf_global = crate::dmabuf::init_dmabuf_global(dh, &mut dmabuf_state, None, None);
    let drm_syncobj_state = crate::syncobj::init_drm_syncobj_global(dh, None);
    let viewporter_state = crate::viewporter::init_viewporter_global(dh);
    let fractional_scale_manager_state = crate::fractional_scale::init_fractional_scale_global(dh);
    let cursor_shape_manager_state = crate::cursor_shape::init_cursor_shape_global(dh);
    let pointer_constraints_state = crate::pointer_constraints::init_pointer_constraints_global(dh);
    let relative_pointer_state = crate::relative_pointer::init_relative_pointer_global(dh);
    let text_input_state = crate::text_input::init_text_input_global(dh);
    let input_method_state = crate::input_method::init_input_method_global(dh);
    let session_lock_state = crate::session_lock::init_session_lock_global(dh);
    let foreign_toplevel_list_state =
        crate::foreign_toplevel::init_foreign_toplevel_list_global(dh);
    let presentation_state = crate::presentation::init_presentation_global(dh);
    let idle_inhibit_state = crate::idle_inhibit::init_idle_inhibit_global(dh);
    let primary_selection_state = crate::primary_selection::init_primary_selection_global(dh);
    let single_pixel_buffer_state = crate::single_pixel_buffer::init_single_pixel_buffer_global(dh);
    let content_type_state = crate::content_type::init_content_type_global(dh);
    let tearing_control_state = crate::tearing_control::init_tearing_control_global(dh);
    let (idle_notifier_state, idle_loop) = crate::idle_notify::init_idle_notify_global(dh)?;
    let alpha_modifier_state = crate::alpha_modifier::init_alpha_modifier_global(dh);

    Ok(ProtocolGlobals {
        dmabuf_state,
        dmabuf_global,
        drm_syncobj_state,
        viewporter_state,
        fractional_scale_manager_state,
        cursor_shape_manager_state,
        pointer_constraints_state,
        relative_pointer_state,
        text_input_state,
        input_method_state,
        session_lock_state,
        foreign_toplevel_list_state,
        presentation_state,
        idle_inhibit_state,
        primary_selection_state,
        single_pixel_buffer_state,
        content_type_state,
        tearing_control_state,
        idle_notifier_state,
        idle_loop,
        alpha_modifier_state,
    })
}

fn init_seat(dh: &DisplayHandle) -> Result<(SeatState<KynexState>, Seat<KynexState>), ShellError> {
    let mut seat_state = SeatState::new();
    let mut seat = seat_state.new_wl_seat(dh, "kynex-seat-0");
    let _pointer = seat.add_pointer();
    let _keyboard = seat.add_keyboard(XkbConfig::default(), 200, 200)?;
    Ok((seat_state, seat))
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod state_tests;

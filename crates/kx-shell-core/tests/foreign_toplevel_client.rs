// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for `ext_foreign_toplevel_list_v1` protocol handler.
//!
//! Tests:
//! - Unauthorized client registry filtering (global hidden from unprivileged clients).
//! - Authorized client global binding and event reception.
//! - Toplevel window creation, `identifier/title/app_id` event propagation, updates, and destruction.

#![forbid(unsafe_code)]

use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use kx_shell_core::client::{ClientLimits, ClientState};
use kx_shell_core::state::KynexState;
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1, ext_foreign_toplevel_list_v1,
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

#[derive(Default, Debug)]
struct ForeignToplevelInfo {
    identifier: String,
    title: String,
    app_id: String,
    done_count: usize,
    closed: bool,
}

#[derive(Default)]
struct PanelClientData {
    foreign_toplevel_list: Option<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1>,
    all_globals: Vec<(String, u32)>,
    toplevels: Vec<(
        ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        ForeignToplevelInfo,
    )>,
    finished: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for PanelClientData {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            state.all_globals.push((interface.clone(), version));
            if interface == "ext_foreign_toplevel_list_v1" && version >= 1 {
                state.foreign_toplevel_list = Some(registry.bind(name, 1, qhandle, ()));
            }
        }
    }
}

impl Dispatch<ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1, ()> for PanelClientData {
    fn event(
        state: &mut Self,
        _proxy: &ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } => {
                state
                    .toplevels
                    .push((toplevel, ForeignToplevelInfo::default()));
            }
            ext_foreign_toplevel_list_v1::Event::Finished => {
                state.finished = true;
            }
            _ => {}
        }
    }

    wayland_client::event_created_child!(
        PanelClientData,
        ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1,
        [
            0 => (ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ()),
        ]
    );
}

impl Dispatch<ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1, ()> for PanelClientData {
    fn event(
        state: &mut Self,
        proxy: &ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let Some((_, info)) = state.toplevels.iter_mut().find(|(h, _)| h == proxy) {
            match event {
                ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
                    info.identifier = identifier;
                }
                ext_foreign_toplevel_handle_v1::Event::Title { title } => {
                    info.title = title;
                }
                ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                    info.app_id = app_id;
                }
                ext_foreign_toplevel_handle_v1::Event::Done => {
                    info.done_count += 1;
                }
                ext_foreign_toplevel_handle_v1::Event::Closed => {
                    info.closed = true;
                }
                _ => {}
            }
        }
    }
}

#[derive(Default)]
struct AppClientData {
    compositor: Option<wl_compositor::WlCompositor>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    configured: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for AppClientData {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "wl_compositor" && version >= 4 {
                state.compositor = Some(registry.bind(name, 4, qhandle, ()));
            } else if interface == "xdg_wm_base" && version >= 1 {
                state.wm_base = Some(registry.bind(name, 1, qhandle, ()));
            }
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for AppClientData {
    fn event(
        _state: &mut Self,
        _proxy: &wl_compositor::WlCompositor,
        _event: wl_compositor::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for AppClientData {
    fn event(
        _state: &mut Self,
        _proxy: &wl_surface::WlSurface,
        _event: wl_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for AppClientData {
    fn event(
        _state: &mut Self,
        proxy: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            proxy.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for AppClientData {
    fn event(
        state: &mut Self,
        proxy: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            proxy.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for AppClientData {
    fn event(
        _state: &mut Self,
        _proxy: &xdg_toplevel::XdgToplevel,
        _event: xdg_toplevel::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

struct TestHarness {
    display: Display<KynexState>,
    state: KynexState,
}

impl TestHarness {
    fn new() -> Result<Self, Box<dyn Error>> {
        let display = Display::new()?;
        let state = KynexState::new(
            &display.handle(),
            "wayland-foreign-toplevel-test".to_string(),
            Size::from((1280, 720)),
        )?;
        Ok(Self { display, state })
    }

    fn connect_panel_client(
        &mut self,
        is_allowed: bool,
    ) -> Result<
        (
            Connection,
            wayland_client::EventQueue<PanelClientData>,
            PanelClientData,
        ),
        Box<dyn Error>,
    > {
        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_state = Arc::new(ClientState {
            limits: ClientLimits::default(),
            is_foreign_toplevel_allowed: AtomicBool::new(is_allowed),
            ..Default::default()
        });
        self.display
            .handle()
            .insert_client(server_stream, client_state)?;

        let conn = Connection::from_socket(client_stream)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let mut client_data = PanelClientData::default();

        let _registry = conn.display().get_registry(&qhandle, ());
        conn.flush()?;
        self.display.dispatch_clients(&mut self.state)?;
        self.display.flush_clients()?;

        let read_guard = conn.prepare_read().ok_or("prepare_read failed")?;
        read_guard.read()?;
        event_queue.dispatch_pending(&mut client_data)?;

        Ok((conn, event_queue, client_data))
    }

    fn sync_panel(
        &mut self,
        conn: &Connection,
        event_queue: &mut wayland_client::EventQueue<PanelClientData>,
        client_data: &mut PanelClientData,
    ) -> Result<(), Box<dyn Error>> {
        conn.flush()?;
        self.display.dispatch_clients(&mut self.state)?;
        self.display.flush_clients()?;

        let read_guard = conn.prepare_read().ok_or("prepare_read failed")?;
        read_guard.read()?;
        event_queue.dispatch_pending(client_data)?;
        Ok(())
    }

    fn connect_app_client(
        &mut self,
    ) -> Result<
        (
            Connection,
            wayland_client::EventQueue<AppClientData>,
            AppClientData,
        ),
        Box<dyn Error>,
    > {
        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_state = Arc::new(ClientState::default());
        self.display
            .handle()
            .insert_client(server_stream, client_state)?;

        let conn = Connection::from_socket(client_stream)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let mut client_data = AppClientData::default();

        let _registry = conn.display().get_registry(&qhandle, ());
        conn.flush()?;
        self.display.dispatch_clients(&mut self.state)?;
        self.display.flush_clients()?;

        let read_guard = conn.prepare_read().ok_or("prepare_read failed")?;
        read_guard.read()?;
        event_queue.dispatch_pending(&mut client_data)?;

        Ok((conn, event_queue, client_data))
    }

    fn sync_app(
        &mut self,
        conn: &Connection,
        event_queue: &mut wayland_client::EventQueue<AppClientData>,
        client_data: &mut AppClientData,
    ) -> Result<(), Box<dyn Error>> {
        conn.flush()?;
        self.display.dispatch_clients(&mut self.state)?;
        self.display.flush_clients()?;

        let read_guard = conn.prepare_read().ok_or("prepare_read failed")?;
        read_guard.read()?;
        event_queue.dispatch_pending(client_data)?;
        Ok(())
    }
}

#[test]
fn test_unauthorized_client_foreign_toplevel_filtered() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (_conn, _queue, client_data) = harness.connect_panel_client(false)?;

    let has_global = client_data
        .all_globals
        .iter()
        .any(|(iface, _)| iface == "ext_foreign_toplevel_list_v1");
    assert!(
        !has_global,
        "Unauthorized client must not see ext_foreign_toplevel_list_v1 in registry"
    );
    assert!(client_data.foreign_toplevel_list.is_none());
    Ok(())
}

#[test]
fn test_authorized_client_discovers_and_binds_foreign_toplevel() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (conn, mut queue, mut client_data) = harness.connect_panel_client(true)?;

    let has_global = client_data
        .all_globals
        .iter()
        .any(|(iface, version)| iface == "ext_foreign_toplevel_list_v1" && *version == 1);
    assert!(
        has_global,
        "Authorized client must see ext_foreign_toplevel_list_v1 version 1"
    );
    assert!(client_data.foreign_toplevel_list.is_some());

    harness.sync_panel(&conn, &mut queue, &mut client_data)?;
    assert!(!client_data.finished);
    Ok(())
}

#[test]
fn test_foreign_toplevel_window_lifecycle_and_updates() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (panel_conn, mut panel_queue, mut panel_data) = harness.connect_panel_client(true)?;
    harness.sync_panel(&panel_conn, &mut panel_queue, &mut panel_data)?;
    assert_eq!(panel_data.toplevels.len(), 0);

    let (app_conn, mut app_queue, mut app_data) = harness.connect_app_client()?;
    let app_qhandle = app_queue.handle();

    let compositor = app_data
        .compositor
        .as_ref()
        .ok_or("Missing wl_compositor")?;
    let wm_base = app_data.wm_base.as_ref().ok_or("Missing xdg_wm_base")?;

    let surface = compositor.create_surface(&app_qhandle, ());
    let xdg_surface = wm_base.get_xdg_surface(&surface, &app_qhandle, ());
    let xdg_toplevel = xdg_surface.get_toplevel(&app_qhandle, ());

    xdg_toplevel.set_title("Editor Window".to_string());
    xdg_toplevel.set_app_id("org.kynex.editor".to_string());
    surface.commit();

    harness.sync_app(&app_conn, &mut app_queue, &mut app_data)?;
    assert!(app_data.configured);

    surface.commit();
    harness.sync_app(&app_conn, &mut app_queue, &mut app_data)?;

    harness.sync_panel(&panel_conn, &mut panel_queue, &mut panel_data)?;

    assert_eq!(
        panel_data.toplevels.len(),
        1,
        "Authorized panel should receive toplevel event for mapped window"
    );
    let info = &panel_data.toplevels[0].1;
    assert_eq!(info.title, "Editor Window");
    assert_eq!(info.app_id, "org.kynex.editor");
    assert_ne!(info.identifier, "");
    assert!(!info.closed);

    xdg_toplevel.set_title("Editor Window - Modified".to_string());
    xdg_toplevel.set_app_id("org.kynex.editor.modified".to_string());
    surface.commit();

    harness.sync_app(&app_conn, &mut app_queue, &mut app_data)?;
    harness.sync_panel(&panel_conn, &mut panel_queue, &mut panel_data)?;

    let updated_info = &panel_data.toplevels[0].1;
    assert_eq!(updated_info.title, "Editor Window - Modified");
    assert_eq!(updated_info.app_id, "org.kynex.editor.modified");

    xdg_toplevel.destroy();
    xdg_surface.destroy();
    surface.destroy();

    harness.sync_app(&app_conn, &mut app_queue, &mut app_data)?;
    harness.sync_panel(&panel_conn, &mut panel_queue, &mut panel_data)?;

    let final_info = &panel_data.toplevels[0].1;
    assert!(
        final_info.closed,
        "Foreign toplevel handle must receive closed event when window is unmapped"
    );

    Ok(())
}

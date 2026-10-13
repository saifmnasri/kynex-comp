// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration test for XDG Shell protocol lifecycle and window management.

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_layout::WindowState;
use kx_shell_core::window::{primary_output_usable_geometry, sync_layout_to_space};
use kx_shell_core::{ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

#[derive(Default)]
struct XdgClientData {
    compositor: Option<wl_compositor::WlCompositor>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    last_configure_serial: Option<u32>,
    configured: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for XdgClientData {
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

impl Dispatch<wl_compositor::WlCompositor, ()> for XdgClientData {
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

impl Dispatch<wl_surface::WlSurface, ()> for XdgClientData {
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

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for XdgClientData {
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

impl Dispatch<xdg_surface::XdgSurface, ()> for XdgClientData {
    fn event(
        state: &mut Self,
        proxy: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            state.last_configure_serial = Some(serial);
            proxy.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for XdgClientData {
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

fn step_client_server(
    conn: &Connection,
    event_queue: &mut wayland_client::EventQueue<XdgClientData>,
    client_state: &mut XdgClientData,
    display: &mut Display<KynexState>,
    state: &mut KynexState,
) -> Result<(), Box<dyn std::error::Error>> {
    conn.flush()?;
    display.dispatch_clients(state)?;
    display.flush_clients()?;

    let read_guard = conn
        .prepare_read()
        .ok_or("Failed to prepare connection read")?;
    read_guard.read()?;
    event_queue.dispatch_pending(client_state)?;
    Ok(())
}

struct TestHarness {
    display: Display<KynexState>,
    state: KynexState,
    conn: Connection,
    event_queue: wayland_client::EventQueue<XdgClientData>,
    client_state: XdgClientData,
    surface: wl_surface::WlSurface,
    xdg_surface: xdg_surface::XdgSurface,
    xdg_toplevel: xdg_toplevel::XdgToplevel,
}

impl TestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut display: Display<KynexState> = Display::new()?;
        let initial_size = Size::from((1280, 720));
        let mut state = KynexState::new(
            &display.handle(),
            "wayland-xdg-test".to_string(),
            initial_size,
        )?;

        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_data = Arc::new(ClientState::default());
        display.handle().insert_client(server_stream, client_data)?;

        let conn = Connection::from_socket(client_stream)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let mut client_state = XdgClientData::default();

        let _registry = conn.display().get_registry(&qhandle, ());
        step_client_server(
            &conn,
            &mut event_queue,
            &mut client_state,
            &mut display,
            &mut state,
        )?;

        let compositor = client_state
            .compositor
            .take()
            .ok_or("wl_compositor must be advertised")?;
        let wm_base = client_state
            .wm_base
            .take()
            .ok_or("xdg_wm_base must be advertised")?;

        let surface = compositor.create_surface(&qhandle, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qhandle, ());
        let xdg_toplevel = xdg_surface.get_toplevel(&qhandle, ());

        Ok(Self {
            display,
            state,
            conn,
            event_queue,
            client_state,
            surface,
            xdg_surface,
            xdg_toplevel,
        })
    }

    fn step(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        step_client_server(
            &self.conn,
            &mut self.event_queue,
            &mut self.client_state,
            &mut self.display,
            &mut self.state,
        )
    }
}

#[test]
fn test_xdg_toplevel_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TestHarness::new()?;
    assert_eq!(harness.state.space.elements().count(), 0);

    harness.xdg_toplevel.set_title("Test Window".to_string());
    harness.surface.commit();
    harness.step()?;

    assert!(harness.client_state.configured);
    assert!(harness.client_state.last_configure_serial.is_some());
    assert_eq!(harness.state.space.elements().count(), 1);

    harness.xdg_toplevel.destroy();
    harness.xdg_surface.destroy();
    harness.surface.destroy();

    harness.conn.flush()?;
    harness.display.dispatch_clients(&mut harness.state)?;
    assert_eq!(harness.state.space.elements().count(), 0);
    Ok(())
}

#[test]
fn test_xdg_toplevel_maximize_and_fullscreen() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TestHarness::new()?;
    harness.xdg_toplevel.set_title("Initial Title".to_string());
    harness
        .xdg_toplevel
        .set_app_id("org.kynex.test".to_string());
    harness.surface.commit();
    harness.step()?;

    let wid = *harness
        .state
        .window_map
        .keys()
        .next()
        .ok_or("No mapped window")?;
    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    assert_eq!(win.state, WindowState::Normal);

    harness.xdg_toplevel.set_maximized();
    harness.step()?;
    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    assert_eq!(win.state, WindowState::Maximized);

    harness.xdg_toplevel.unset_maximized();
    harness.step()?;
    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    assert_eq!(win.state, WindowState::Normal);

    harness.xdg_toplevel.set_fullscreen(None);
    harness.step()?;
    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    assert_eq!(win.state, WindowState::Fullscreen);

    harness.xdg_toplevel.unset_fullscreen();
    harness.step()?;
    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    assert_eq!(win.state, WindowState::Normal);
    Ok(())
}

#[test]
fn test_xdg_toplevel_metadata_and_minimize() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TestHarness::new()?;
    harness.xdg_toplevel.set_title("Initial Title".to_string());
    harness
        .xdg_toplevel
        .set_app_id("org.kynex.test".to_string());
    harness.surface.commit();
    harness.step()?;

    let wid = *harness
        .state
        .window_map
        .keys()
        .next()
        .ok_or("No mapped window")?;

    harness.xdg_toplevel.set_title("Updated Title".to_string());
    harness
        .xdg_toplevel
        .set_app_id("org.kynex.updated".to_string());
    harness.step()?;
    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    assert_eq!(win.title, "Updated Title");
    assert_eq!(win.app_id, "org.kynex.updated");

    harness.xdg_toplevel.set_minimized();
    harness.step()?;
    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    assert_eq!(win.state, WindowState::Minimized);
    assert_eq!(harness.state.space.elements().count(), 0);

    let usable = primary_output_usable_geometry(&harness.state);
    harness.state.window_manager.unminimize_window(wid, usable);
    sync_layout_to_space(&mut harness.state);
    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    assert_eq!(win.state, WindowState::Normal);
    assert_eq!(harness.state.space.elements().count(), 1);
    Ok(())
}

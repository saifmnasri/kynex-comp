// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration test for XDG Shell protocol lifecycle and window management.

use std::os::unix::net::UnixStream;
use std::sync::Arc;

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

#[test]
fn test_xdg_toplevel_lifecycle() {
    let mut display: Display<KynexState> = Display::new().unwrap();
    let initial_size = Size::from((1280, 720));
    let mut state = KynexState::new(
        &display.handle(),
        "wayland-xdg-test".to_string(),
        initial_size,
    )
    .unwrap();

    let (server_stream, client_stream) = UnixStream::pair().unwrap();
    let client_data = Arc::new(ClientState::default());
    display
        .handle()
        .insert_client(server_stream, client_data)
        .unwrap();

    let conn = Connection::from_socket(client_stream).unwrap();
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
    )
    .unwrap();

    let compositor = client_state
        .compositor
        .take()
        .expect("wl_compositor must be advertised");
    let wm_base = client_state
        .wm_base
        .take()
        .expect("xdg_wm_base must be advertised");

    // Initially space has 0 windows
    assert_eq!(state.space.elements().count(), 0);

    // Create surface and xdg_toplevel
    let surface = compositor.create_surface(&qhandle, ());
    let xdg_surface = wm_base.get_xdg_surface(&surface, &qhandle, ());
    let xdg_toplevel = xdg_surface.get_toplevel(&qhandle, ());
    xdg_toplevel.set_title("Test Window".to_string());
    surface.commit();

    step_client_server(
        &conn,
        &mut event_queue,
        &mut client_state,
        &mut display,
        &mut state,
    )
    .unwrap();

    // Client received configure and acked it
    assert!(client_state.configured);
    assert!(client_state.last_configure_serial.is_some());

    // Window should now be mapped in the compositor's Space
    assert_eq!(state.space.elements().count(), 1);

    // Destroy toplevel
    xdg_toplevel.destroy();
    xdg_surface.destroy();
    surface.destroy();

    conn.flush().unwrap();
    display.dispatch_clients(&mut state).unwrap();

    // After destroying the toplevel, the window is unmapped from the space
    assert_eq!(state.space.elements().count(), 0);
}

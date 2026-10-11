// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Hostile and edge-case Wayland client integration tests for resilience and anti-DoS enforcement.

#![forbid(unsafe_code)]

use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientLimits, ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

#[derive(Default)]
struct HostileClientData {
    compositor: Option<wl_compositor::WlCompositor>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    configured_serial: Option<u32>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for HostileClientData {
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

impl Dispatch<wl_compositor::WlCompositor, ()> for HostileClientData {
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

impl Dispatch<wl_surface::WlSurface, ()> for HostileClientData {
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

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for HostileClientData {
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

impl Dispatch<xdg_surface::XdgSurface, ()> for HostileClientData {
    fn event(
        state: &mut Self,
        proxy: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            state.configured_serial = Some(serial);
            proxy.ack_configure(serial);
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for HostileClientData {
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
            "wayland-hostile-test".to_string(),
            Size::from((1280, 720)),
        )?;
        Ok(Self { display, state })
    }

    fn connect_client(
        &mut self,
        limits: ClientLimits,
    ) -> Result<
        (
            Connection,
            wayland_client::EventQueue<HostileClientData>,
            HostileClientData,
        ),
        Box<dyn Error>,
    > {
        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_data = Arc::new(ClientState {
            limits,
            ..Default::default()
        });
        self.display
            .handle()
            .insert_client(server_stream, client_data)?;

        let conn = Connection::from_socket(client_stream)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let mut client_data = HostileClientData::default();

        let _registry = conn.display().get_registry(&qhandle, ());
        conn.flush()?;
        self.display.dispatch_clients(&mut self.state)?;
        self.display.flush_clients()?;

        let read_guard = conn.prepare_read().ok_or("Failed to prepare client read")?;
        read_guard.read()?;
        event_queue.dispatch_pending(&mut client_data)?;

        Ok((conn, event_queue, client_data))
    }
}

#[test]
fn test_surface_quota_exhaustion_disconnects_client() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let limits = ClientLimits {
        max_surfaces: 2,
        ..Default::default()
    };
    let (conn, mut event_queue, mut client_data) = harness.connect_client(limits)?;
    let qhandle = event_queue.handle();
    let comp = client_data
        .compositor
        .take()
        .ok_or("Compositor global missing")?;

    // Creating 2 surfaces succeeds within limits
    let _s1 = comp.create_surface(&qhandle, ());
    let _s2 = comp.create_surface(&qhandle, ());
    conn.flush()?;
    harness.display.dispatch_clients(&mut harness.state)?;

    // 3rd surface exceeds quota
    let _s3 = comp.create_surface(&qhandle, ());
    conn.flush()?;
    harness.display.dispatch_clients(&mut harness.state)?;
    harness.display.flush_clients()?;

    // Client connection should receive protocol error and disconnect
    let has_read_error = match conn.prepare_read() {
        Some(guard) => guard.read().is_err(),
        None => true,
    };
    let dispatch_res = event_queue.dispatch_pending(&mut client_data);
    let has_client_error =
        has_read_error || dispatch_res.is_err() || conn.protocol_error().is_some();
    assert!(has_client_error);
    // Compositor itself must remain completely healthy and undamaged
    assert_eq!(harness.state.space.elements().count(), 0);
    Ok(())
}

#[test]
fn test_surface_quota_reclaimed_on_destroy() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let limits = ClientLimits {
        max_surfaces: 2,
        ..Default::default()
    };
    let (conn, event_queue, mut client_data) = harness.connect_client(limits)?;
    let qhandle = event_queue.handle();
    let comp = client_data
        .compositor
        .take()
        .ok_or("Compositor global missing")?;

    let s1 = comp.create_surface(&qhandle, ());
    let s2 = comp.create_surface(&qhandle, ());
    conn.flush()?;
    harness.display.dispatch_clients(&mut harness.state)?;

    // Destroy one surface
    s2.destroy();
    conn.flush()?;
    harness.display.dispatch_clients(&mut harness.state)?;

    // Reclaimed quota: creating another surface must succeed without error
    let _s3 = comp.create_surface(&qhandle, ());
    conn.flush()?;
    harness.display.dispatch_clients(&mut harness.state)?;
    harness.display.flush_clients()?;

    assert!(conn.protocol_error().is_none());
    s1.destroy();
    Ok(())
}

#[test]
fn test_rapid_client_connect_and_abrupt_disconnect_churn() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;

    for i in 0..15 {
        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_data = Arc::new(ClientState::default());
        harness
            .display
            .handle()
            .insert_client(server_stream, client_data)?;

        let conn = Connection::from_socket(client_stream)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let mut client_state = HostileClientData::default();

        let _registry = conn.display().get_registry(&qhandle, ());
        let _ = conn.flush();
        let _ = harness.display.dispatch_clients(&mut harness.state);
        let _ = harness.display.flush_clients();

        if let Some(read_guard) = conn.prepare_read() {
            let _ = read_guard.read();
        }
        let _ = event_queue.dispatch_pending(&mut client_state);

        if let Some(comp) = client_state.compositor.take() {
            let surface = comp.create_surface(&qhandle, ());
            if i % 2 == 0 {
                surface.commit();
            }
            let _ = conn.flush();
            let _ = harness.display.dispatch_clients(&mut harness.state);
        }

        // Abruptly drop client connection
        drop(conn);
        let _ = harness.display.dispatch_clients(&mut harness.state);
        let _ = harness.display.flush_clients();
    }

    // Compositor state must be clean and no windows leaked
    assert_eq!(harness.state.space.elements().count(), 0);
    Ok(())
}

#[test]
fn test_unconfigured_surface_commit_does_not_panic() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (conn, event_queue, mut client_data) = harness.connect_client(ClientLimits::default())?;
    let qhandle = event_queue.handle();
    let comp = client_data
        .compositor
        .take()
        .ok_or("Compositor global missing")?;
    let wm_base = client_data
        .wm_base
        .take()
        .ok_or("xdg_wm_base global missing")?;

    let surface = comp.create_surface(&qhandle, ());
    let xdg_surface = wm_base.get_xdg_surface(&surface, &qhandle, ());
    let toplevel = xdg_surface.get_toplevel(&qhandle, ());

    // Commit immediately without waiting for configure or acknowledging serial
    surface.commit();
    let _ = conn.flush();
    let _ = harness.display.dispatch_clients(&mut harness.state);

    // Compositor handles unconfigured commit without panic
    assert_eq!(harness.state.space.elements().count(), 1);

    toplevel.destroy();
    xdg_surface.destroy();
    surface.destroy();
    let _ = conn.flush();
    let _ = harness.display.dispatch_clients(&mut harness.state);

    // After destroying, space is cleaned up
    assert_eq!(harness.state.space.elements().count(), 0);
    Ok(())
}

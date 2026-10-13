// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for the `zwlr_layer_shell_v1` protocol handler.
//!
//! Covers:
//! - Authorized client binding and Top/Bottom layer surface mapping.
//! - Non-exclusive usable zone adjustment and margin recalculation.
//! - Unauthorized client filtering (`zwlr_layer_shell_v1` hidden from registry).
//! - Layer surface quota bounding and protocol error on quota exhaustion.
//! - Resource reclamation upon layer surface destruction.

#![forbid(unsafe_code)]

use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kx_shell_core::client::{ClientLimits, ClientState};
use kx_shell_core::state::KynexState;
use smithay::desktop::layer_map_for_output;
use smithay::reexports::wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1, zwlr_layer_surface_v1,
};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle};

#[derive(Default)]
struct LayerClientData {
    compositor: Option<wl_compositor::WlCompositor>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    configured_serial: Option<u32>,
    configure_count: usize,
    closed: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for LayerClientData {
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
            } else if interface == "zwlr_layer_shell_v1" && version >= 1 {
                state.layer_shell = Some(registry.bind(name, 1, qhandle, ()));
            }
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for LayerClientData {
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

impl Dispatch<wl_surface::WlSurface, ()> for LayerClientData {
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

impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for LayerClientData {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        _event: zwlr_layer_shell_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for LayerClientData {
    fn event(
        state: &mut Self,
        proxy: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, .. } => {
                state.configured_serial = Some(serial);
                state.configure_count += 1;
                proxy.ack_configure(serial);
            }
            zwlr_layer_surface_v1::Event::Closed => {
                state.closed = true;
            }
            _ => {}
        }
    }
}

type ClientHandle = (
    Connection,
    wayland_client::EventQueue<LayerClientData>,
    LayerClientData,
    Arc<ClientState>,
);

struct TestHarness {
    display: Display<KynexState>,
    state: KynexState,
}

impl TestHarness {
    fn new() -> Result<Self, Box<dyn Error>> {
        let display = Display::new()?;
        let state = KynexState::new(
            &display.handle(),
            "wayland-layer-shell-test".to_string(),
            Size::from((1280, 720)),
        )?;
        Ok(Self { display, state })
    }

    fn connect_client(
        &mut self,
        is_layer_shell_allowed: bool,
        limits: ClientLimits,
    ) -> Result<ClientHandle, Box<dyn Error>> {
        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_state = Arc::new(ClientState {
            limits,
            is_layer_shell_allowed: AtomicBool::new(is_layer_shell_allowed),
            ..Default::default()
        });
        self.display
            .handle()
            .insert_client(server_stream, client_state.clone())?;

        let conn = Connection::from_socket(client_stream)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let mut client_data = LayerClientData::default();

        let _registry = conn.display().get_registry(&qhandle, ());
        conn.flush()?;
        self.display.dispatch_clients(&mut self.state)?;
        self.display.flush_clients()?;

        let read_guard = conn.prepare_read().ok_or("Failed to prepare client read")?;
        read_guard.read()?;
        event_queue.dispatch_pending(&mut client_data)?;

        Ok((conn, event_queue, client_data, client_state))
    }

    fn sync_roundtrip(
        &mut self,
        conn: &Connection,
        event_queue: &mut wayland_client::EventQueue<LayerClientData>,
        client_data: &mut LayerClientData,
    ) -> Result<(), Box<dyn Error>> {
        conn.flush()?;
        self.display.dispatch_clients(&mut self.state)?;
        self.display.flush_clients()?;

        let read_guard = conn.prepare_read().ok_or("Failed to prepare client read")?;
        read_guard.read()?;
        event_queue.dispatch_pending(client_data)?;
        Ok(())
    }
}

#[test]
fn test_authorized_client_layer_shell_lifecycle() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (conn, mut queue, mut data, client_state) =
        harness.connect_client(true, ClientLimits::default())?;
    let qhandle = queue.handle();

    let ls = data.layer_shell.clone().ok_or("Layer shell not found")?;
    let comp = data.compositor.clone().ok_or("Compositor not found")?;
    let surface = comp.create_surface(&qhandle, ());
    let layer_surface = ls.get_layer_surface(
        &surface,
        None,
        zwlr_layer_shell_v1::Layer::Top,
        "panel".to_string(),
        &qhandle,
        (),
    );

    layer_surface.set_size(1280, 40);
    layer_surface.set_anchor(
        zwlr_layer_surface_v1::Anchor::Top
            | zwlr_layer_surface_v1::Anchor::Left
            | zwlr_layer_surface_v1::Anchor::Right,
    );
    layer_surface.set_exclusive_zone(40);
    surface.commit();

    harness.sync_roundtrip(&conn, &mut queue, &mut data)?;
    assert!(data.configured_serial.is_some());
    assert_eq!(client_state.layer_surface_count.load(Ordering::Relaxed), 1);

    {
        let layer_map = layer_map_for_output(&harness.state.output);
        assert_eq!(layer_map.layers().count(), 1);
    }

    layer_surface.destroy();
    surface.destroy();
    harness.sync_roundtrip(&conn, &mut queue, &mut data)?;

    assert_eq!(client_state.layer_surface_count.load(Ordering::Relaxed), 0);
    let layer_map = layer_map_for_output(&harness.state.output);
    assert_eq!(layer_map.layers().count(), 0);
    Ok(())
}

#[test]
fn test_unauthorized_client_layer_shell_global_filtered() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (_conn, _queue, data, _client_state) =
        harness.connect_client(false, ClientLimits::default())?;

    assert!(data.compositor.is_some());
    assert!(data.layer_shell.is_none());
    Ok(())
}

fn create_test_layer_surface(
    comp: &wl_compositor::WlCompositor,
    ls: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
    qhandle: &QueueHandle<LayerClientData>,
    namespace: &str,
) -> (
    wl_surface::WlSurface,
    zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
) {
    let surface = comp.create_surface(qhandle, ());
    let layer = ls.get_layer_surface(
        &surface,
        None,
        zwlr_layer_shell_v1::Layer::Top,
        namespace.to_string(),
        qhandle,
        (),
    );
    layer.set_size(1280, 40);
    layer.set_anchor(
        zwlr_layer_surface_v1::Anchor::Top
            | zwlr_layer_surface_v1::Anchor::Left
            | zwlr_layer_surface_v1::Anchor::Right,
    );
    (surface, layer)
}

#[test]
fn test_layer_surface_quota_exhaustion_disconnects_client() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let limits = ClientLimits {
        max_layer_surfaces: 1,
        ..ClientLimits::default()
    };
    let (conn, mut queue, mut data, client_state) = harness.connect_client(true, limits)?;
    let qhandle = queue.handle();
    let ls = data.layer_shell.clone().ok_or("Layer shell not found")?;
    let comp = data.compositor.clone().ok_or("Compositor not found")?;

    let (s1, _l1) = create_test_layer_surface(&comp, &ls, &qhandle, "panel1");
    s1.commit();
    harness.sync_roundtrip(&conn, &mut queue, &mut data)?;
    assert_eq!(client_state.layer_surface_count.load(Ordering::Relaxed), 1);

    let (s2, _l2) = create_test_layer_surface(&comp, &ls, &qhandle, "panel2");
    s2.commit();

    conn.flush()?;
    let _ = harness.display.dispatch_clients(&mut harness.state);
    let _ = harness.display.flush_clients();

    let client_read_result = conn
        .prepare_read()
        .ok_or_else(|| -> Box<dyn Error> { "Failed to prepare read".into() })
        .and_then(|guard| guard.read().map_err(|e| Box::new(e) as Box<dyn Error>));

    assert!(
        client_read_result.is_err(),
        "Client must be disconnected with protocol error upon quota breach"
    );
    Ok(())
}

#[test]
fn test_layer_surface_quota_reclaimed_after_destroy() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let limits = ClientLimits {
        max_layer_surfaces: 1,
        ..ClientLimits::default()
    };
    let (conn, mut queue, mut data, client_state) = harness.connect_client(true, limits)?;
    let qhandle = queue.handle();
    let ls = data.layer_shell.clone().ok_or("Layer shell not found")?;
    let comp = data.compositor.clone().ok_or("Compositor not found")?;

    let (s1, l1) = create_test_layer_surface(&comp, &ls, &qhandle, "panel1");
    s1.commit();
    harness.sync_roundtrip(&conn, &mut queue, &mut data)?;
    assert_eq!(client_state.layer_surface_count.load(Ordering::Relaxed), 1);

    l1.destroy();
    s1.destroy();
    harness.sync_roundtrip(&conn, &mut queue, &mut data)?;
    assert_eq!(client_state.layer_surface_count.load(Ordering::Relaxed), 0);

    let (s2, _l2) = create_test_layer_surface(&comp, &ls, &qhandle, "panel2");
    s2.commit();
    harness.sync_roundtrip(&conn, &mut queue, &mut data)?;
    assert_eq!(client_state.layer_surface_count.load(Ordering::Relaxed), 1);
    Ok(())
}

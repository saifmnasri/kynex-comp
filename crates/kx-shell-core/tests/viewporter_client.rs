// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for viewport cropping and scaling protocol (`wp_viewporter`).

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};

#[derive(Default)]
struct ViewporterTestClientState {
    viewporter_global: Option<(u32, u32)>,
    compositor_global: Option<(u32, u32)>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for ViewporterTestClientState {
    fn event(
        state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "wp_viewporter" {
                state.viewporter_global = Some((name, version));
            } else if interface == "wl_compositor" {
                state.compositor_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<wp_viewporter::WpViewporter, ()> for ViewporterTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &wp_viewporter::WpViewporter,
        _event: wp_viewporter::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wp_viewport::WpViewport, ()> for ViewporterTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &wp_viewport::WpViewport,
        _event: wp_viewport::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for ViewporterTestClientState {
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

impl Dispatch<wl_surface::WlSurface, ()> for ViewporterTestClientState {
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

struct ViewporterTestHarness {
    display: Display<KynexState>,
    state: KynexState,
    conn: Connection,
    event_queue: EventQueue<ViewporterTestClientState>,
    client_state: ViewporterTestClientState,
}

impl ViewporterTestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut display: Display<KynexState> = Display::new()?;
        let initial_size = Size::from((1280, 720));
        let mut state = KynexState::new(
            &display.handle(),
            "viewporter-test-sock".to_string(),
            initial_size,
        )?;

        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_data = Arc::new(ClientState::default());
        display.handle().insert_client(server_stream, client_data)?;

        let conn = Connection::from_socket(client_stream)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();

        let display_proxy = conn.display();
        let _registry = display_proxy.get_registry(&qhandle, ());
        conn.flush()?;

        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;

        let read_guard = conn.prepare_read().ok_or("Failed to prepare client read")?;
        read_guard.read()?;

        let mut client_state = ViewporterTestClientState::default();
        event_queue.dispatch_pending(&mut client_state)?;

        Ok(Self {
            display,
            state,
            conn,
            event_queue,
            client_state,
        })
    }

    fn roundtrip(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.conn.flush()?;
        self.display.dispatch_clients(&mut self.state)?;
        self.display.flush_clients()?;

        let read_guard = self
            .conn
            .prepare_read()
            .ok_or("Failed to prepare client read")?;
        read_guard.read()?;
        self.event_queue.dispatch_pending(&mut self.client_state)?;
        Ok(())
    }
}

#[test]
fn test_viewporter_global_advertised_and_bindable() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = ViewporterTestHarness::new()?;

    let (viewporter_name, viewporter_version) = harness
        .client_state
        .viewporter_global
        .ok_or("wp_viewporter global must be advertised")?;

    assert_eq!(viewporter_version, 1, "wp_viewporter version must be 1");

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let _viewporter: wp_viewporter::WpViewporter = registry.bind(viewporter_name, 1, &qhandle, ());

    harness.roundtrip()?;
    Ok(())
}

#[test]
fn test_viewport_normal_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = ViewporterTestHarness::new()?;

    let (viewporter_name, _) = harness
        .client_state
        .viewporter_global
        .ok_or("wp_viewporter global must be advertised")?;
    let (comp_name, comp_version) = harness
        .client_state
        .compositor_global
        .ok_or("wl_compositor global must be advertised")?;

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let compositor: wl_compositor::WlCompositor =
        registry.bind(comp_name, comp_version.min(5), &qhandle, ());
    let viewporter: wp_viewporter::WpViewporter = registry.bind(viewporter_name, 1, &qhandle, ());
    let surface = compositor.create_surface(&qhandle, ());

    // Create viewport for surface
    let viewport = viewporter.get_viewport(&surface, &qhandle, ());

    // Crop source and scale destination
    viewport.set_source(0.0, 0.0, 100.0, 100.0);
    viewport.set_destination(200, 200);
    surface.commit();

    harness.roundtrip()?;

    // Unset source (-1.0) and destination (-1)
    viewport.set_source(-1.0, -1.0, -1.0, -1.0);
    viewport.set_destination(-1, -1);
    surface.commit();

    harness.roundtrip()?;

    // Clean destruction
    viewport.destroy();
    viewporter.destroy();
    surface.destroy();

    harness.roundtrip()?;
    Ok(())
}

#[test]
fn test_viewporter_duplicate_surface_rejection() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = ViewporterTestHarness::new()?;

    let (viewporter_name, _) = harness
        .client_state
        .viewporter_global
        .ok_or("wp_viewporter global must be advertised")?;
    let (comp_name, comp_version) = harness
        .client_state
        .compositor_global
        .ok_or("wl_compositor global must be advertised")?;

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let compositor: wl_compositor::WlCompositor =
        registry.bind(comp_name, comp_version.min(5), &qhandle, ());
    let viewporter: wp_viewporter::WpViewporter = registry.bind(viewporter_name, 1, &qhandle, ());
    let surface = compositor.create_surface(&qhandle, ());

    // First viewport creation succeeds
    let _vp1 = viewporter.get_viewport(&surface, &qhandle, ());

    // Hostile client: attempt duplicate get_viewport on the same surface
    let _vp2 = viewporter.get_viewport(&surface, &qhandle, ());

    harness.conn.flush()?;

    // Server must reject the duplicate viewport request with protocol error (viewport_exists = 0)
    let _ = harness.display.dispatch_clients(&mut harness.state);
    let _ = harness.display.flush_clients();

    // Client connection should encounter protocol error
    let read_err = match harness.conn.prepare_read() {
        Some(guard) => guard.read().is_err(),
        None => true,
    };
    let mut dummy = ViewporterTestClientState::default();
    let dispatch_result = harness.event_queue.dispatch_pending(&mut dummy);

    let had_error = read_err || dispatch_result.is_err();
    assert!(
        had_error,
        "Duplicate get_viewport must cause protocol error and disconnect client"
    );

    Ok(())
}

#[test]
fn test_viewport_invalid_destination_rejection() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = ViewporterTestHarness::new()?;

    let (viewporter_name, _) = harness
        .client_state
        .viewporter_global
        .ok_or("wp_viewporter global must be advertised")?;
    let (comp_name, comp_version) = harness
        .client_state
        .compositor_global
        .ok_or("wl_compositor global must be advertised")?;

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let compositor: wl_compositor::WlCompositor =
        registry.bind(comp_name, comp_version.min(5), &qhandle, ());
    let viewporter: wp_viewporter::WpViewporter = registry.bind(viewporter_name, 1, &qhandle, ());
    let surface = compositor.create_surface(&qhandle, ());
    let viewport = viewporter.get_viewport(&surface, &qhandle, ());

    // Hostile client: set negative destination width
    viewport.set_destination(-5, 100);

    harness.conn.flush()?;

    // Server must post bad_value protocol error
    let _ = harness.display.dispatch_clients(&mut harness.state);
    let _ = harness.display.flush_clients();

    let read_err = match harness.conn.prepare_read() {
        Some(guard) => guard.read().is_err(),
        None => true,
    };
    let mut dummy = ViewporterTestClientState::default();
    let dispatch_result = harness.event_queue.dispatch_pending(&mut dummy);

    let had_error = read_err || dispatch_result.is_err();
    assert!(
        had_error,
        "Invalid destination dimensions must cause protocol error and disconnect client"
    );

    Ok(())
}

#[test]
fn test_viewport_invalid_source_rejection() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = ViewporterTestHarness::new()?;

    let (viewporter_name, _) = harness
        .client_state
        .viewporter_global
        .ok_or("wp_viewporter global must be advertised")?;
    let (comp_name, comp_version) = harness
        .client_state
        .compositor_global
        .ok_or("wl_compositor global must be advertised")?;

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let compositor: wl_compositor::WlCompositor =
        registry.bind(comp_name, comp_version.min(5), &qhandle, ());
    let viewporter: wp_viewporter::WpViewporter = registry.bind(viewporter_name, 1, &qhandle, ());
    let surface = compositor.create_surface(&qhandle, ());
    let viewport = viewporter.get_viewport(&surface, &qhandle, ());

    // Hostile client: set negative source coordinate (not all -1.0)
    viewport.set_source(-10.0, 0.0, 100.0, 100.0);

    harness.conn.flush()?;

    // Server must post bad_value protocol error
    let _ = harness.display.dispatch_clients(&mut harness.state);
    let _ = harness.display.flush_clients();

    let read_err = match harness.conn.prepare_read() {
        Some(guard) => guard.read().is_err(),
        None => true,
    };
    let mut dummy = ViewporterTestClientState::default();
    let dispatch_result = harness.event_queue.dispatch_pending(&mut dummy);

    let had_error = read_err || dispatch_result.is_err();
    assert!(
        had_error,
        "Invalid source coordinates must cause protocol error and disconnect client"
    );

    Ok(())
}

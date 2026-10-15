// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for the input method protocol (`zwp_input_method_v2`).
//!
//! Covers:
//! - Unauthorized/unprivileged client filtering (`zwp_input_method_manager_v2` hidden from registry).
//! - Privileged client registry discovery (`zwp_input_method_manager_v2` advertised with version 1).
//! - Non-panicking parent geometry lookup for input method popups.

#![forbid(unsafe_code)]

use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use kx_shell_core::client::{ClientLimits, ClientState};
use kx_shell_core::input_method::InputMethodHandler;
use kx_shell_core::state::KynexState;
use smithay::reexports::wayland_server::Display;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface as ServerWlSurface;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

#[derive(Default)]
struct ImTestClientData {
    compositor_global: Option<(u32, u32)>,
    input_method_global: Option<(u32, u32)>,
    all_globals: Vec<(String, u32)>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for ImTestClientData {
    fn event(
        state: &mut Self,
        _registry: &wl_registry::WlRegistry,
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
            state.all_globals.push((interface.clone(), version));
            if interface == "wl_compositor" {
                state.compositor_global = Some((name, version));
            } else if interface == "zwp_input_method_manager_v2" {
                state.input_method_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for ImTestClientData {
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

impl Dispatch<wl_surface::WlSurface, ()> for ImTestClientData {
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

type ClientHandle = (
    Connection,
    wayland_client::EventQueue<ImTestClientData>,
    ImTestClientData,
    Arc<ClientState>,
    smithay::reexports::wayland_server::Client,
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
            "wayland-input-method-test".to_string(),
            Size::from((1280, 720)),
        )?;
        Ok(Self { display, state })
    }

    fn connect_client(
        &mut self,
        is_input_method_allowed: bool,
    ) -> Result<ClientHandle, Box<dyn Error>> {
        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_state = Arc::new(ClientState {
            limits: ClientLimits::default(),
            is_input_method_allowed: AtomicBool::new(is_input_method_allowed),
            ..Default::default()
        });
        let client = self
            .display
            .handle()
            .insert_client(server_stream, client_state.clone())?;

        let conn = Connection::from_socket(client_stream)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let mut client_data = ImTestClientData::default();

        let _registry = conn.display().get_registry(&qhandle, ());
        conn.flush()?;
        self.display.dispatch_clients(&mut self.state)?;
        self.display.flush_clients()?;

        let read_guard = conn.prepare_read().ok_or("Failed to prepare client read")?;
        read_guard.read()?;
        event_queue.dispatch_pending(&mut client_data)?;

        Ok((conn, event_queue, client_data, client_state, client))
    }
}

#[test]
fn test_unprivileged_client_input_method_global_hidden() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (_conn, _queue, data, _client_state, _client) = harness.connect_client(false)?;

    assert!(
        data.compositor_global.is_some(),
        "Core compositor global must be advertised"
    );
    assert!(
        data.input_method_global.is_none(),
        "zwp_input_method_manager_v2 must be hidden from unprivileged clients"
    );
    assert!(
        !data
            .all_globals
            .iter()
            .any(|(interface, _)| interface == "zwp_input_method_manager_v2"),
        "Registry event stream must not contain zwp_input_method_manager_v2"
    );
    Ok(())
}

#[test]
fn test_privileged_client_input_method_global_advertised() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (_conn, _queue, data, _client_state, _client) = harness.connect_client(true)?;

    assert!(
        data.compositor_global.is_some(),
        "Core compositor global must be advertised"
    );
    assert!(
        data.input_method_global.is_some(),
        "zwp_input_method_manager_v2 must be advertised to privileged clients"
    );

    let (global_name, global_version) = data.input_method_global.ok_or("Global missing")?;
    assert!(global_name > 0, "Global name must be valid u32 identifier");
    assert_eq!(
        global_version, 1,
        "zwp_input_method_manager_v2 must be advertised at version 1"
    );
    Ok(())
}

#[test]
fn test_parent_geometry_unmapped_surface() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (conn, event_queue, data, _client_state, server_client) = harness.connect_client(true)?;
    let (comp_name, comp_ver) = data.compositor_global.ok_or("compositor global missing")?;
    let qh = event_queue.handle();
    let registry = conn.display().get_registry(&qh, ());
    let compositor: wl_compositor::WlCompositor =
        registry.bind(comp_name, comp_ver.min(5), &qh, ());
    let surface = compositor.create_surface(&qh, ());
    conn.flush()?;

    harness.display.dispatch_clients(&mut harness.state)?;
    harness.display.flush_clients()?;

    let server_surface = server_client
        .object_from_protocol_id::<ServerWlSurface>(
            &harness.display.handle(),
            surface.id().protocol_id(),
        )
        .map_err(|e| format!("Server surface not found: {e:?}"))?;

    let geom = harness.state.parent_geometry(&server_surface);
    assert_eq!(geom.loc.x, 0);
    assert_eq!(geom.loc.y, 0);
    assert_eq!(geom.size.w, 0);
    assert_eq!(geom.size.h, 0);
    Ok(())
}

// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for content-type protocol (`wp_content_type_v1`).

#![forbid(unsafe_code)]

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::wp::content_type::v1::client::wp_content_type_manager_v1::{
    self, WpContentTypeManagerV1,
};
use wayland_protocols::wp::content_type::v1::client::wp_content_type_v1::{
    self, Type, WpContentTypeV1,
};

#[derive(Default)]
struct ContentTypeTestClientState {
    manager_global: Option<(u32, u32)>,
    compositor_global: Option<(u32, u32)>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for ContentTypeTestClientState {
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
            if interface == "wp_content_type_manager_v1" {
                state.manager_global = Some((name, version));
            } else if interface == "wl_compositor" {
                state.compositor_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<WpContentTypeManagerV1, ()> for ContentTypeTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WpContentTypeManagerV1,
        _event: wp_content_type_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpContentTypeV1, ()> for ContentTypeTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WpContentTypeV1,
        _event: wp_content_type_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for ContentTypeTestClientState {
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

impl Dispatch<wl_surface::WlSurface, ()> for ContentTypeTestClientState {
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

struct TestHarness {
    display: Display<KynexState>,
    state: KynexState,
    conn: Connection,
    event_queue: EventQueue<ContentTypeTestClientState>,
    client_state: ContentTypeTestClientState,
}

impl TestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let display = Display::new()?;
        let dh = display.handle();

        let state = KynexState::new(
            &dh,
            "kynex-test-content-type".to_string(),
            Size::from((1280, 800)),
        )?;

        let (server_stream, client_stream) = UnixStream::pair()?;
        server_stream.set_nonblocking(true)?;
        client_stream.set_nonblocking(true)?;

        let client_state = Arc::new(ClientState::default());
        display
            .handle()
            .insert_client(server_stream, client_state)?;

        let conn = Connection::from_socket(client_stream)?;
        let event_queue = conn.new_event_queue();
        let client_state = ContentTypeTestClientState::default();

        let mut harness = Self {
            display,
            state,
            conn,
            event_queue,
            client_state,
        };

        let qh = harness.event_queue.handle();
        let display_proxy = harness.conn.display();
        let _registry = display_proxy.get_registry(&qh, ());
        harness.sync_roundtrip()?;

        Ok(harness)
    }

    fn sync_roundtrip(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for _ in 0..3 {
            self.conn.flush()?;
            self.display.dispatch_clients(&mut self.state)?;
            self.display.flush_clients()?;

            if let Some(read_guard) = self.conn.prepare_read() {
                let _ = read_guard.read();
            }
            self.event_queue.dispatch_pending(&mut self.client_state)?;
        }
        Ok(())
    }
}

#[test]
fn test_content_type_global_advertised_and_bindable() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TestHarness::new()?;

    let (mgr_name, version) = harness
        .client_state
        .manager_global
        .ok_or("wp_content_type_manager_v1 global not advertised")?;
    assert_eq!(
        version, 1,
        "wp_content_type_manager_v1 should advertise version 1"
    );

    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let registry = display_proxy.get_registry(&qh, ());
    let _manager: WpContentTypeManagerV1 = registry.bind(mgr_name, 1, &qh, ());

    harness.sync_roundtrip()?;
    Ok(())
}

#[test]
fn test_content_type_set_and_surface_commit() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TestHarness::new()?;

    let (mgr_name, _) = harness
        .client_state
        .manager_global
        .ok_or("wp_content_type_manager_v1 global not advertised")?;
    let (comp_name, comp_ver) = harness
        .client_state
        .compositor_global
        .ok_or("wl_compositor global not advertised")?;

    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let registry = display_proxy.get_registry(&qh, ());

    let manager: WpContentTypeManagerV1 = registry.bind(mgr_name, 1, &qh, ());
    let compositor: wl_compositor::WlCompositor = registry.bind(comp_name, comp_ver, &qh, ());
    let surface = compositor.create_surface(&qh, ());

    let content_type = manager.get_surface_content_type(&surface, &qh, ());
    content_type.set_content_type(Type::Game);
    surface.commit();

    harness.sync_roundtrip()?;

    content_type.destroy();
    surface.destroy();
    manager.destroy();

    harness.sync_roundtrip()?;
    Ok(())
}

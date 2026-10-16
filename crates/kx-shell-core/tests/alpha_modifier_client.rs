// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for alpha modifier protocol (`wp_alpha_modifier_v1`).

#![forbid(unsafe_code)]

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::wp::alpha_modifier::v1::client::wp_alpha_modifier_surface_v1::{
    self, WpAlphaModifierSurfaceV1,
};
use wayland_protocols::wp::alpha_modifier::v1::client::wp_alpha_modifier_v1::{
    self, WpAlphaModifierV1,
};

#[derive(Default)]
struct AlphaModifierTestClientState {
    modifier_global: Option<(u32, u32)>,
    compositor_global: Option<(u32, u32)>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for AlphaModifierTestClientState {
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
            if interface == "wp_alpha_modifier_v1" {
                state.modifier_global = Some((name, version));
            } else if interface == "wl_compositor" {
                state.compositor_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<WpAlphaModifierV1, ()> for AlphaModifierTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WpAlphaModifierV1,
        _event: wp_alpha_modifier_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpAlphaModifierSurfaceV1, ()> for AlphaModifierTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WpAlphaModifierSurfaceV1,
        _event: wp_alpha_modifier_surface_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for AlphaModifierTestClientState {
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

impl Dispatch<wl_surface::WlSurface, ()> for AlphaModifierTestClientState {
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
    event_queue: EventQueue<AlphaModifierTestClientState>,
    client_state: AlphaModifierTestClientState,
}

impl TestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let display = Display::new()?;
        let dh = display.handle();

        let state = KynexState::new(
            &dh,
            "kynex-test-alpha-modifier".to_string(),
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
        let client_state = AlphaModifierTestClientState::default();

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
fn test_wp_alpha_modifier_global_advertised_and_bindable() -> Result<(), Box<dyn std::error::Error>>
{
    let mut harness = TestHarness::new()?;

    let (modifier_name, modifier_version) = harness
        .client_state
        .modifier_global
        .expect("wp_alpha_modifier_v1 global should be advertised");
    assert_eq!(modifier_version, 1);

    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let registry = display_proxy.get_registry(&qh, ());

    let _modifier: WpAlphaModifierV1 = registry.bind(modifier_name, modifier_version, &qh, ());
    harness.sync_roundtrip()?;

    Ok(())
}

#[test]
fn test_wp_alpha_modifier_surface_set_multiplier_and_commit()
-> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TestHarness::new()?;
    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let registry = display_proxy.get_registry(&qh, ());

    let (comp_name, comp_ver) = harness
        .client_state
        .compositor_global
        .expect("wl_compositor should be advertised");
    let compositor: wl_compositor::WlCompositor = registry.bind(comp_name, comp_ver, &qh, ());

    let (mod_name, mod_ver) = harness
        .client_state
        .modifier_global
        .expect("wp_alpha_modifier_v1 should be advertised");
    let modifier: WpAlphaModifierV1 = registry.bind(mod_name, mod_ver, &qh, ());

    let surface = compositor.create_surface(&qh, ());
    let modifier_surface = modifier.get_surface(&surface, &qh, ());

    // Factor 0.75 = 3221225472 (0.75 * u32::MAX)
    let factor: u32 = 3_221_225_472;
    modifier_surface.set_multiplier(factor);
    surface.commit();

    harness.sync_roundtrip()?;

    // Verify destruction works cleanly
    modifier_surface.destroy();
    surface.commit();
    surface.destroy();
    modifier.destroy();
    harness.sync_roundtrip()?;

    Ok(())
}

#[test]
fn test_wp_alpha_modifier_duplicate_surface_raises_protocol_error()
-> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TestHarness::new()?;
    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let registry = display_proxy.get_registry(&qh, ());

    let (comp_name, comp_ver) = harness
        .client_state
        .compositor_global
        .ok_or("wl_compositor should be advertised")?;
    let compositor: wl_compositor::WlCompositor = registry.bind(comp_name, comp_ver, &qh, ());

    let (mod_name, mod_ver) = harness
        .client_state
        .modifier_global
        .ok_or("wp_alpha_modifier_v1 should be advertised")?;
    let modifier: WpAlphaModifierV1 = registry.bind(mod_name, mod_ver, &qh, ());

    let surface = compositor.create_surface(&qh, ());

    let _modifier_surface1 = modifier.get_surface(&surface, &qh, ());
    let _modifier_surface2 = modifier.get_surface(&surface, &qh, ());

    harness.conn.flush()?;
    harness.display.dispatch_clients(&mut harness.state)?;
    harness.display.flush_clients()?;

    if let Some(read_guard) = harness.conn.prepare_read() {
        let _ = read_guard.read();
    }
    let res = harness
        .event_queue
        .dispatch_pending(&mut harness.client_state);

    assert!(
        res.is_err() || harness.conn.protocol_error().is_some(),
        "Expected protocol error for duplicate alpha modifier on same surface"
    );

    Ok(())
}

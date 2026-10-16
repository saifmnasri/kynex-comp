// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for the Wayland Idle Inhibit protocol (`zwp_idle_inhibit_v1`).
//!
//! Covers:
//! - Registry advertising and binding of `zwp_idle_inhibit_manager_v1`.
//! - Creation and removal of `zwp_idle_inhibitor_v1` on a client surface.
//! - Reflection of active inhibitors in compositor state (`is_idle_inhibited()`).
//! - Clean removal when inhibitor is destroyed.

#![forbid(unsafe_code)]

use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::client::ClientState;
use kx_shell_core::state::KynexState;
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::wp::idle_inhibit::zv1::client::{
    zwp_idle_inhibit_manager_v1, zwp_idle_inhibitor_v1,
};

#[derive(Default)]
struct IdleInhibitClientData {
    compositor_global: Option<(u32, u32)>,
    idle_inhibit_global: Option<(u32, u32)>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for IdleInhibitClientData {
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
            if interface == "wl_compositor" {
                state.compositor_global = Some((name, version));
            } else if interface == "zwp_idle_inhibit_manager_v1" {
                state.idle_inhibit_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for IdleInhibitClientData {
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

impl Dispatch<wl_surface::WlSurface, ()> for IdleInhibitClientData {
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

impl Dispatch<zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1, ()> for IdleInhibitClientData {
    fn event(
        _state: &mut Self,
        _proxy: &zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1,
        _event: zwp_idle_inhibit_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1, ()> for IdleInhibitClientData {
    fn event(
        _state: &mut Self,
        _proxy: &zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1,
        _event: zwp_idle_inhibitor_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

struct TestHarness {
    display: Display<KynexState>,
    state: KynexState,
    client_conn: Connection,
    event_queue: EventQueue<IdleInhibitClientData>,
    client_data: IdleInhibitClientData,
}

impl TestHarness {
    fn new() -> Result<Self, Box<dyn Error>> {
        let display = Display::<KynexState>::new()?;
        let dh = display.handle();
        let state = KynexState::new(&dh, "wayland-test".to_string(), Size::from((1280, 800)))?;

        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_state = Arc::new(ClientState::default());
        display
            .handle()
            .insert_client(server_stream, client_state)?;

        let client_conn = Connection::from_socket(client_stream)?;
        let event_queue = client_conn.new_event_queue();
        let client_data = IdleInhibitClientData::default();

        Ok(Self {
            display,
            state,
            client_conn,
            event_queue,
            client_data,
        })
    }

    fn sync_roundtrip(&mut self) -> Result<(), Box<dyn Error>> {
        for _ in 0..3 {
            self.client_conn.flush()?;
            self.display.dispatch_clients(&mut self.state)?;
            self.display.flush_clients()?;

            if let Some(read_guard) = self.client_conn.prepare_read() {
                let _ = read_guard.read();
            }
            self.event_queue.dispatch_pending(&mut self.client_data)?;
        }
        Ok(())
    }
}

#[test]
fn test_client_binds_idle_inhibit_manager_global() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let qh = harness.event_queue.handle();

    let display_proxy = harness.client_conn.display();
    let registry = display_proxy.get_registry(&qh, ());

    harness.sync_roundtrip()?;

    let (name, version) = harness
        .client_data
        .idle_inhibit_global
        .ok_or("zwp_idle_inhibit_manager_v1 must be advertised in registry")?;
    assert!(version >= 1, "protocol version must be at least 1");

    let _manager: zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1 =
        registry.bind(name, 1, &qh, ());
    harness.sync_roundtrip()?;

    Ok(())
}

#[test]
fn test_idle_inhibitor_lifecycle() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let qh = harness.event_queue.handle();

    let display_proxy = harness.client_conn.display();
    let registry = display_proxy.get_registry(&qh, ());
    harness.sync_roundtrip()?;

    let (comp_name, comp_ver) = harness
        .client_data
        .compositor_global
        .ok_or("wl_compositor must be advertised")?;
    let compositor: wl_compositor::WlCompositor =
        registry.bind(comp_name, comp_ver.min(5), &qh, ());

    let (mgr_name, _) = harness
        .client_data
        .idle_inhibit_global
        .ok_or("zwp_idle_inhibit_manager_v1 must be advertised")?;
    let manager: zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1 =
        registry.bind(mgr_name, 1, &qh, ());

    harness.sync_roundtrip()?;

    // Initially no inhibitors are active
    assert!(
        !harness.state.is_idle_inhibited(),
        "State must not be idle inhibited initially"
    );

    // Create a surface and attach an idle inhibitor
    let surface = compositor.create_surface(&qh, ());
    let inhibitor = manager.create_inhibitor(&surface, &qh, ());
    harness.sync_roundtrip()?;

    assert!(
        harness.state.is_idle_inhibited(),
        "State must be idle inhibited after inhibitor is created"
    );
    assert_eq!(
        harness.state.idle_inhibitors.len(),
        1,
        "Should have exactly 1 active idle inhibitor"
    );

    // Destroy inhibitor
    inhibitor.destroy();
    harness.sync_roundtrip()?;

    assert!(
        !harness.state.is_idle_inhibited(),
        "State must no longer be idle inhibited after inhibitor is destroyed"
    );
    assert!(
        harness.state.idle_inhibitors.is_empty(),
        "Idle inhibitor list must be empty after destruction"
    );

    Ok(())
}

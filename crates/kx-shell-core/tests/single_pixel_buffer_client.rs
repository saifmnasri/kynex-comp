// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for single-pixel buffer protocol (`wp_single_pixel_buffer_v1`).

#![forbid(unsafe_code)]

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_buffer, wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::wp::single_pixel_buffer::v1::client::wp_single_pixel_buffer_manager_v1::{
    self, WpSinglePixelBufferManagerV1,
};

#[derive(Default)]
struct SinglePixelTestClientState {
    manager_global: Option<(u32, u32)>,
    compositor_global: Option<(u32, u32)>,
    released_buffers: usize,
}

impl Dispatch<wl_registry::WlRegistry, ()> for SinglePixelTestClientState {
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
            if interface == "wp_single_pixel_buffer_manager_v1" {
                state.manager_global = Some((name, version));
            } else if interface == "wl_compositor" {
                state.compositor_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<WpSinglePixelBufferManagerV1, ()> for SinglePixelTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WpSinglePixelBufferManagerV1,
        _event: wp_single_pixel_buffer_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for SinglePixelTestClientState {
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

impl Dispatch<wl_surface::WlSurface, ()> for SinglePixelTestClientState {
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

impl Dispatch<wl_buffer::WlBuffer, ()> for SinglePixelTestClientState {
    fn event(
        state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            state.released_buffers += 1;
        }
    }
}

struct SinglePixelTestHarness {
    display: Display<KynexState>,
    state: KynexState,
    conn: Connection,
    event_queue: EventQueue<SinglePixelTestClientState>,
    client_state: SinglePixelTestClientState,
}

impl SinglePixelTestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let display = Display::<KynexState>::new()?;
        let dh = display.handle();
        let initial_size: Size<i32, smithay::utils::Physical> = (1920, 1080).into();
        let state = KynexState::new(&dh, "wayland-single-pixel-test".to_string(), initial_size)?;

        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_state = Arc::new(ClientState::default());
        display
            .handle()
            .insert_client(server_stream, client_state)?;

        let conn = Connection::from_socket(client_stream)?;
        let event_queue = conn.new_event_queue();
        let client_state = SinglePixelTestClientState::default();

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
fn test_single_pixel_buffer_global_advertised_and_bindable()
-> Result<(), Box<dyn std::error::Error>> {
    let mut harness = SinglePixelTestHarness::new()?;

    let (name, version) = harness
        .client_state
        .manager_global
        .ok_or("wp_single_pixel_buffer_manager_v1 not advertised")?;
    assert_eq!(
        version, 1,
        "wp_single_pixel_buffer_manager_v1 should advertise version 1"
    );

    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let manager: WpSinglePixelBufferManagerV1 =
        display_proxy.get_registry(&qh, ()).bind(name, 1, &qh, ());

    harness.sync_roundtrip()?;

    manager.destroy();
    harness.sync_roundtrip()?;
    Ok(())
}

#[test]
fn test_single_pixel_buffer_creation_and_surface_commit() -> Result<(), Box<dyn std::error::Error>>
{
    let mut harness = SinglePixelTestHarness::new()?;

    let (mgr_name, mgr_version) = harness
        .client_state
        .manager_global
        .ok_or("single pixel manager global advertised")?;
    let (comp_name, comp_version) = harness
        .client_state
        .compositor_global
        .ok_or("compositor global advertised")?;

    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let registry = display_proxy.get_registry(&qh, ());

    let manager: WpSinglePixelBufferManagerV1 = registry.bind(mgr_name, mgr_version, &qh, ());
    let compositor: wl_compositor::WlCompositor =
        registry.bind(comp_name, comp_version.min(4), &qh, ());

    harness.sync_roundtrip()?;

    // Create a 1x1 32-bit RGBA buffer (red=0x11223344, green=0x55667788, blue=0x99AABBCC, alpha=0xFFFFFFFF)
    let buffer =
        manager.create_u32_rgba_buffer(0x1122_3344, 0x5566_7788, 0x99AA_BBCC, 0xFFFF_FFFF, &qh, ());

    let surface = compositor.create_surface(&qh, ());

    surface.attach(Some(&buffer), 0, 0);
    surface.damage(0, 0, 1, 1);
    surface.commit();

    harness.sync_roundtrip()?;

    // Clean teardown
    buffer.destroy();
    surface.destroy();
    manager.destroy();

    harness.sync_roundtrip()?;
    Ok(())
}

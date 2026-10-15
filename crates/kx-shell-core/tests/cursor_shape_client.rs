// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for cursor shape protocol (`wp_cursor_shape_v1`).

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_pointer, wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::{self, Shape, WpCursorShapeDeviceV1},
    wp_cursor_shape_manager_v1::{self, WpCursorShapeManagerV1},
};

#[derive(Default)]
struct CursorShapeTestClientState {
    manager_global: Option<(u32, u32)>,
    seat_global: Option<(u32, u32)>,
    pointer: Option<wl_pointer::WlPointer>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for CursorShapeTestClientState {
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
            if interface == "wp_cursor_shape_manager_v1" {
                state.manager_global = Some((name, version));
            } else if interface == "wl_seat" {
                state.seat_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<WpCursorShapeManagerV1, ()> for CursorShapeTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WpCursorShapeManagerV1,
        _event: wp_cursor_shape_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpCursorShapeDeviceV1, ()> for CursorShapeTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &WpCursorShapeDeviceV1,
        _event: wp_cursor_shape_device_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for CursorShapeTestClientState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities } = event {
            let caps = capabilities
                .into_result()
                .unwrap_or(wl_seat::Capability::empty());
            if caps.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qhandle, ()));
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for CursorShapeTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_pointer::WlPointer,
        _event: wl_pointer::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

struct CursorShapeTestHarness {
    display: Display<KynexState>,
    state: KynexState,
    conn: Connection,
    event_queue: EventQueue<CursorShapeTestClientState>,
    client_state: CursorShapeTestClientState,
}

impl CursorShapeTestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut display: Display<KynexState> = Display::new()?;
        let initial_size = Size::from((1280, 720));
        let mut state = KynexState::new(
            &display.handle(),
            "cursor-shape-test-sock".to_string(),
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

        let mut client_state = CursorShapeTestClientState::default();
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
fn test_cursor_shape_global_advertised_and_bindable() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = CursorShapeTestHarness::new()?;

    let (name, version) = harness
        .client_state
        .manager_global
        .ok_or("wp_cursor_shape_manager_v1 not advertised")?;
    assert_eq!(
        version, 2,
        "cursor shape manager should advertise version 2"
    );

    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let _manager: WpCursorShapeManagerV1 =
        display_proxy.get_registry(&qh, ()).bind(name, 2, &qh, ());

    harness.roundtrip()?;
    Ok(())
}

#[test]
fn test_cursor_shape_device_creation_and_set_shape() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = CursorShapeTestHarness::new()?;

    let (mgr_name, mgr_version) = harness
        .client_state
        .manager_global
        .ok_or("manager global advertised")?;
    let (seat_name, seat_version) = harness
        .client_state
        .seat_global
        .ok_or("seat global advertised")?;

    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let registry = display_proxy.get_registry(&qh, ());

    let manager: WpCursorShapeManagerV1 = registry.bind(mgr_name, mgr_version, &qh, ());
    let _seat: wl_seat::WlSeat = registry.bind(seat_name, seat_version, &qh, ());

    harness.roundtrip()?;

    let pointer = harness
        .client_state
        .pointer
        .clone()
        .ok_or("seat should offer pointer capability")?;

    let device = manager.get_pointer(&pointer, &qh, ());

    // Request setting cursor shape to Pointer
    device.set_shape(0, Shape::Pointer);

    harness.roundtrip()?;

    // Verify cursor device destroy lifecycle
    device.destroy();
    manager.destroy();

    harness.roundtrip()?;
    Ok(())
}

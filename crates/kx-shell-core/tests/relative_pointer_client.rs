// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for relative pointer protocol (`zwp_relative_pointer_v1`).

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientState, KynexState};
use smithay::input::pointer::{MotionEvent, RelativeMotionEvent};
use smithay::reexports::wayland_server::Display;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface as ServerWlSurface;
use smithay::utils::{Point, Size};
use wayland_client::protocol::{wl_compositor, wl_pointer, wl_registry, wl_seat, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::relative_pointer::zv1::client::{
    zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1,
    zwp_relative_pointer_v1::{self, ZwpRelativePointerV1},
};

#[derive(Debug, Clone, PartialEq)]
struct ReceivedRelativeMotion {
    utime_hi: u32,
    utime_lo: u32,
    dx: f64,
    dy: f64,
    dx_unaccel: f64,
    dy_unaccel: f64,
}

#[derive(Default)]
struct RelativePointerTestClientState {
    manager_global: Option<(u32, u32)>,
    compositor_global: Option<(u32, u32)>,
    seat_global: Option<(u32, u32)>,
    pointer: Option<wl_pointer::WlPointer>,
    motions: Vec<ReceivedRelativeMotion>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for RelativePointerTestClientState {
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
            if interface == "zwp_relative_pointer_manager_v1" {
                state.manager_global = Some((name, version));
            } else if interface == "wl_compositor" {
                state.compositor_global = Some((name, version));
            } else if interface == "wl_seat" {
                state.seat_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for RelativePointerTestClientState {
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

impl Dispatch<wl_seat::WlSeat, ()> for RelativePointerTestClientState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: wayland_client::WEnum::Value(caps),
        } = event
        {
            if caps.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qhandle, ()));
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for RelativePointerTestClientState {
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

impl Dispatch<wl_surface::WlSurface, ()> for RelativePointerTestClientState {
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

impl Dispatch<ZwpRelativePointerManagerV1, ()> for RelativePointerTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpRelativePointerManagerV1,
        _event: <ZwpRelativePointerManagerV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpRelativePointerV1, ()> for RelativePointerTestClientState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpRelativePointerV1,
        event: zwp_relative_pointer_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let zwp_relative_pointer_v1::Event::RelativeMotion {
            utime_hi,
            utime_lo,
            dx,
            dy,
            dx_unaccel,
            dy_unaccel,
        } = event
        {
            state.motions.push(ReceivedRelativeMotion {
                utime_hi,
                utime_lo,
                dx,
                dy,
                dx_unaccel,
                dy_unaccel,
            });
        }
    }
}

struct RelativePointerTestHarness {
    display: Display<KynexState>,
    state: KynexState,
    client: smithay::reexports::wayland_server::Client,
    conn: Connection,
    event_queue: EventQueue<RelativePointerTestClientState>,
    client_state: RelativePointerTestClientState,
}

impl RelativePointerTestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut display: Display<KynexState> = Display::new()?;
        let initial_size = Size::from((800, 600));
        let mut state = KynexState::new(
            &display.handle(),
            "relative-pointer-test-sock".to_string(),
            initial_size,
        )?;

        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_data = Arc::new(ClientState::default());
        let client = display.handle().insert_client(server_stream, client_data)?;

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

        let mut client_state = RelativePointerTestClientState::default();
        event_queue.dispatch_pending(&mut client_state)?;

        Ok(Self {
            display,
            state,
            client,
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
fn test_relative_pointer_global_advertised_and_bindable() -> Result<(), Box<dyn std::error::Error>>
{
    let mut harness = RelativePointerTestHarness::new()?;

    let (name, version) = harness
        .client_state
        .manager_global
        .expect("zwp_relative_pointer_manager_v1 global should be advertised");
    assert_eq!(
        version, 1,
        "Expected zwp_relative_pointer_manager_v1 version 1"
    );

    let qh = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qh, ());
    let manager: ZwpRelativePointerManagerV1 = registry.bind(name, 1, &qh, ());
    assert!(manager.is_alive(), "Bound manager should be alive");

    manager.destroy();
    harness.roundtrip()?;
    Ok(())
}

#[test]
fn test_relative_pointer_creation_and_destroy() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = RelativePointerTestHarness::new()?;

    let (mgr_name, _) = harness
        .client_state
        .manager_global
        .expect("Relative pointer manager global missing");
    let (seat_name, _) = harness
        .client_state
        .seat_global
        .expect("Seat global missing");

    let qh = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qh, ());
    let manager: ZwpRelativePointerManagerV1 = registry.bind(mgr_name, 1, &qh, ());
    let _seat: wl_seat::WlSeat = registry.bind(seat_name, 1, &qh, ());

    harness.roundtrip()?;

    let pointer = harness
        .client_state
        .pointer
        .as_ref()
        .expect("Pointer should be available");
    let rel_pointer = manager.get_relative_pointer(pointer, &qh, ());

    harness.roundtrip()?;
    assert!(rel_pointer.is_alive(), "Relative pointer should be alive");

    rel_pointer.destroy();
    manager.destroy();
    harness.roundtrip()?;
    assert!(
        !rel_pointer.is_alive(),
        "Relative pointer should be destroyed"
    );
    Ok(())
}

#[test]
fn test_relative_pointer_motion_event_dispatch() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = RelativePointerTestHarness::new()?;

    let (mgr_name, _) = harness
        .client_state
        .manager_global
        .expect("Relative pointer manager global missing");
    let (comp_name, _) = harness
        .client_state
        .compositor_global
        .expect("Compositor global missing");
    let (seat_name, _) = harness
        .client_state
        .seat_global
        .expect("Seat global missing");

    let qh = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qh, ());
    let manager: ZwpRelativePointerManagerV1 = registry.bind(mgr_name, 1, &qh, ());
    let compositor: wl_compositor::WlCompositor = registry.bind(comp_name, 1, &qh, ());
    let _seat: wl_seat::WlSeat = registry.bind(seat_name, 1, &qh, ());

    harness.roundtrip()?;

    let surface = compositor.create_surface(&qh, ());
    let pointer = harness
        .client_state
        .pointer
        .as_ref()
        .expect("Pointer should be available");
    let rel_pointer = manager.get_relative_pointer(pointer, &qh, ());

    harness.roundtrip()?;

    // Look up the server-side WlSurface for focus assignment
    let server_surface = harness
        .client
        .object_from_protocol_id::<ServerWlSurface>(
            &harness.display.handle(),
            surface.id().protocol_id(),
        )
        .expect("Server surface not found");

    // Set pointer motion and focus on the surface
    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
    let server_pointer = harness
        .state
        .seat
        .get_pointer()
        .expect("Pointer should exist on seat");
    server_pointer.motion(
        &mut harness.state,
        Some((server_surface.clone(), Point::from((0.0, 0.0)))),
        &MotionEvent {
            location: Point::from((10.0, 10.0)),
            serial,
            time: 1000,
        },
    );
    server_pointer.frame(&mut harness.state);

    // Dispatch a relative motion event
    let event = RelativeMotionEvent {
        delta: Point::from((14.5, -8.25)),
        delta_unaccel: Point::from((10.0, -5.5)),
        utime: 9_876_543_210,
    };
    server_pointer.relative_motion(
        &mut harness.state,
        Some((server_surface.clone(), Point::from((0.0, 0.0)))),
        &event,
    );
    server_pointer.frame(&mut harness.state);

    harness.roundtrip()?;

    assert_eq!(
        harness.client_state.motions.len(),
        1,
        "Expected exactly 1 relative motion event received"
    );
    let motion = &harness.client_state.motions[0];
    assert_eq!(motion.dx, 14.5);
    assert_eq!(motion.dy, -8.25);
    assert_eq!(motion.dx_unaccel, 10.0);
    assert_eq!(motion.dy_unaccel, -5.5);
    let full_utime = (u64::from(motion.utime_hi) << 32) | u64::from(motion.utime_lo);
    assert_eq!(full_utime, 9_876_543_210);

    rel_pointer.destroy();
    manager.destroy();
    surface.destroy();
    harness.roundtrip()?;
    Ok(())
}

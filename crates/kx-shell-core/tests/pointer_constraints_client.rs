// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for pointer constraints protocol (`zwp_pointer_constraints_v1`).

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_pointer, wl_registry, wl_seat, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::wp::pointer_constraints::zv1::client::{
    zwp_confined_pointer_v1::{self, ZwpConfinedPointerV1},
    zwp_locked_pointer_v1::{self, ZwpLockedPointerV1},
    zwp_pointer_constraints_v1::{self, Lifetime, ZwpPointerConstraintsV1},
};

#[derive(Debug, PartialEq, Eq)]
enum ConstraintEvent {
    Locked,
    Unlocked,
    Confined,
    Unconfined,
}

#[derive(Default)]
struct PointerConstraintsTestClientState {
    constraints_global: Option<(u32, u32)>,
    compositor_global: Option<(u32, u32)>,
    seat_global: Option<(u32, u32)>,
    pointer: Option<wl_pointer::WlPointer>,
    events: Vec<ConstraintEvent>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for PointerConstraintsTestClientState {
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
            if interface == "zwp_pointer_constraints_v1" {
                state.constraints_global = Some((name, version));
            } else if interface == "wl_compositor" {
                state.compositor_global = Some((name, version));
            } else if interface == "wl_seat" {
                state.seat_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<ZwpPointerConstraintsV1, ()> for PointerConstraintsTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpPointerConstraintsV1,
        _event: zwp_pointer_constraints_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpLockedPointerV1, ()> for PointerConstraintsTestClientState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpLockedPointerV1,
        event: zwp_locked_pointer_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwp_locked_pointer_v1::Event::Locked => {
                state.events.push(ConstraintEvent::Locked);
            }
            zwp_locked_pointer_v1::Event::Unlocked => {
                state.events.push(ConstraintEvent::Unlocked);
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwpConfinedPointerV1, ()> for PointerConstraintsTestClientState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpConfinedPointerV1,
        event: zwp_confined_pointer_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwp_confined_pointer_v1::Event::Confined => {
                state.events.push(ConstraintEvent::Confined);
            }
            zwp_confined_pointer_v1::Event::Unconfined => {
                state.events.push(ConstraintEvent::Unconfined);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for PointerConstraintsTestClientState {
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

impl Dispatch<wl_surface::WlSurface, ()> for PointerConstraintsTestClientState {
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

impl Dispatch<wl_seat::WlSeat, ()> for PointerConstraintsTestClientState {
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

impl Dispatch<wl_pointer::WlPointer, ()> for PointerConstraintsTestClientState {
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

struct PointerConstraintsTestHarness {
    display: Display<KynexState>,
    state: KynexState,
    client: smithay::reexports::wayland_server::Client,
    conn: Connection,
    event_queue: EventQueue<PointerConstraintsTestClientState>,
    client_state: PointerConstraintsTestClientState,
}

impl PointerConstraintsTestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut display: Display<KynexState> = Display::new()?;
        let initial_size = Size::from((1280, 720));
        let mut state = KynexState::new(
            &display.handle(),
            "pointer-constraints-test-sock".to_string(),
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

        let mut client_state = PointerConstraintsTestClientState::default();
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
fn test_pointer_constraints_global_advertised_and_bindable()
-> Result<(), Box<dyn std::error::Error>> {
    let mut harness = PointerConstraintsTestHarness::new()?;

    let (name, version) = harness
        .client_state
        .constraints_global
        .expect("zwp_pointer_constraints_v1 global should be advertised");

    assert_eq!(
        version, 1,
        "zwp_pointer_constraints_v1 should be advertised at version 1"
    );

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let _manager: ZwpPointerConstraintsV1 = registry.bind(name, version, &qhandle, ());

    harness.roundtrip()?;
    Ok(())
}

#[test]
fn test_pointer_constraints_locked_pointer_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = PointerConstraintsTestHarness::new()?;

    let (mgr_name, mgr_ver) = harness
        .client_state
        .constraints_global
        .expect("zwp_pointer_constraints_v1 global advertised");
    let (comp_name, comp_ver) = harness
        .client_state
        .compositor_global
        .expect("wl_compositor global advertised");
    let (seat_name, seat_ver) = harness
        .client_state
        .seat_global
        .expect("wl_seat global advertised");

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let manager: ZwpPointerConstraintsV1 = registry.bind(mgr_name, mgr_ver, &qhandle, ());
    let compositor: wl_compositor::WlCompositor = registry.bind(comp_name, comp_ver, &qhandle, ());
    let _seat: wl_seat::WlSeat = registry.bind(seat_name, seat_ver, &qhandle, ());

    harness.roundtrip()?;

    let pointer = harness
        .client_state
        .pointer
        .as_ref()
        .expect("pointer capability advertised")
        .clone();
    let surface = compositor.create_surface(&qhandle, ());

    // Request a locked pointer with Persistent lifetime
    let locked_pointer =
        manager.lock_pointer(&surface, &pointer, None, Lifetime::Persistent, &qhandle, ());

    // Set cursor position hint
    locked_pointer.set_cursor_position_hint(100.0, 150.0);
    surface.commit();

    harness.roundtrip()?;

    // Destroy locked pointer
    locked_pointer.destroy();
    surface.destroy();

    harness.roundtrip()?;
    Ok(())
}

#[test]
fn test_pointer_constraints_confined_pointer_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = PointerConstraintsTestHarness::new()?;

    let (mgr_name, mgr_ver) = harness
        .client_state
        .constraints_global
        .expect("zwp_pointer_constraints_v1 global advertised");
    let (comp_name, comp_ver) = harness
        .client_state
        .compositor_global
        .expect("wl_compositor global advertised");
    let (seat_name, seat_ver) = harness
        .client_state
        .seat_global
        .expect("wl_seat global advertised");

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let manager: ZwpPointerConstraintsV1 = registry.bind(mgr_name, mgr_ver, &qhandle, ());
    let compositor: wl_compositor::WlCompositor = registry.bind(comp_name, comp_ver, &qhandle, ());
    let _seat: wl_seat::WlSeat = registry.bind(seat_name, seat_ver, &qhandle, ());

    harness.roundtrip()?;

    let pointer = harness
        .client_state
        .pointer
        .as_ref()
        .expect("pointer capability advertised")
        .clone();
    let surface = compositor.create_surface(&qhandle, ());

    // Request a confined pointer with Persistent lifetime
    let confined_pointer =
        manager.confine_pointer(&surface, &pointer, None, Lifetime::Persistent, &qhandle, ());

    surface.commit();
    harness.roundtrip()?;

    // Destroy confined pointer
    confined_pointer.destroy();
    surface.destroy();

    harness.roundtrip()?;
    Ok(())
}

#[test]
fn test_pointer_constraints_activate_on_focus_acquisition() -> Result<(), Box<dyn std::error::Error>>
{
    use smithay::input::pointer::MotionEvent;
    use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface as ServerWlSurface;
    use smithay::utils::Point;
    use wayland_client::Proxy;

    let mut harness = PointerConstraintsTestHarness::new()?;

    let (mgr_name, mgr_ver) = harness
        .client_state
        .constraints_global
        .expect("zwp_pointer_constraints_v1 global advertised");
    let (comp_name, comp_ver) = harness
        .client_state
        .compositor_global
        .expect("wl_compositor global advertised");
    let (seat_name, seat_ver) = harness
        .client_state
        .seat_global
        .expect("wl_seat global advertised");

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let manager: ZwpPointerConstraintsV1 = registry.bind(mgr_name, mgr_ver, &qhandle, ());
    let compositor: wl_compositor::WlCompositor = registry.bind(comp_name, comp_ver, &qhandle, ());
    let _seat: wl_seat::WlSeat = registry.bind(seat_name, seat_ver, &qhandle, ());

    harness.roundtrip()?;

    let pointer = harness
        .client_state
        .pointer
        .as_ref()
        .expect("pointer capability advertised")
        .clone();
    let surface = compositor.create_surface(&qhandle, ());

    let locked_pointer =
        manager.lock_pointer(&surface, &pointer, None, Lifetime::Persistent, &qhandle, ());
    surface.commit();
    harness.roundtrip()?;

    // Surface has not yet received focus; constraint should remain inactive
    assert!(
        !harness
            .client_state
            .events
            .contains(&ConstraintEvent::Locked),
        "Constraint must not be active before focus acquisition"
    );

    let server_surface = harness
        .client
        .object_from_protocol_id::<ServerWlSurface>(
            &harness.display.handle(),
            surface.id().protocol_id(),
        )
        .expect("Server surface not found");

    let seat_pointer = harness.state.seat.get_pointer().expect("Seat pointer");
    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
    seat_pointer.motion(
        &mut harness.state,
        Some((server_surface.clone(), Point::from((0.0, 0.0)))),
        &MotionEvent {
            location: Point::from((10.0, 10.0)),
            serial,
            time: 100,
        },
    );
    seat_pointer.frame(&mut harness.state);

    harness
        .state
        .seat
        .get_keyboard()
        .expect("Keyboard")
        .set_focus(&mut harness.state, Some(server_surface), serial);

    harness.roundtrip()?;

    // Constraint should now be activated
    assert!(
        harness
            .client_state
            .events
            .contains(&ConstraintEvent::Locked),
        "Constraint must be activated after focus acquisition"
    );

    locked_pointer.destroy();
    surface.destroy();
    harness.roundtrip()?;
    Ok(())
}

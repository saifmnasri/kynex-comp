// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for interactive move and resize window grabs.

#![forbid(unsafe_code)]

use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::grab::{
    BTN_LEFT, BTN_RIGHT, start_compositor_move_grab, start_compositor_resize_grab,
    try_start_compositor_grab,
};
use kx_shell_core::{ClientState, KynexState};
use smithay::backend::input::ButtonState;
use smithay::input::pointer::{ButtonEvent, MotionEvent};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge;
use smithay::reexports::wayland_server::Display;
use smithay::utils::{Point, SERIAL_COUNTER, Size};
use wayland_client::protocol::{wl_compositor, wl_registry, wl_seat, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

#[derive(Default)]
struct TestClientData {
    compositor: Option<wl_compositor::WlCompositor>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    seat: Option<wl_seat::WlSeat>,
    last_configure_serial: Option<u32>,
    configured: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for TestClientData {
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
            } else if interface == "xdg_wm_base" && version >= 1 {
                state.wm_base = Some(registry.bind(name, 1, qhandle, ()));
            } else if interface == "wl_seat" && version >= 2 {
                state.seat = Some(registry.bind(name, 2, qhandle, ()));
            }
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for TestClientData {
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

impl Dispatch<wl_surface::WlSurface, ()> for TestClientData {
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

impl Dispatch<wl_seat::WlSeat, ()> for TestClientData {
    fn event(
        _state: &mut Self,
        _proxy: &wl_seat::WlSeat,
        _event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for TestClientData {
    fn event(
        _state: &mut Self,
        proxy: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            proxy.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for TestClientData {
    fn event(
        state: &mut Self,
        proxy: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            state.last_configure_serial = Some(serial);
            proxy.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for TestClientData {
    fn event(
        _state: &mut Self,
        _proxy: &xdg_toplevel::XdgToplevel,
        _event: xdg_toplevel::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

fn step_turn(
    conn: &Connection,
    event_queue: &mut wayland_client::EventQueue<TestClientData>,
    client_state: &mut TestClientData,
    display: &mut Display<KynexState>,
    state: &mut KynexState,
) -> Result<(), Box<dyn Error>> {
    conn.flush()?;
    display.dispatch_clients(state)?;
    display.flush_clients()?;

    let read_guard = conn
        .prepare_read()
        .ok_or("Failed to prepare connection read")?;
    read_guard.read()?;
    event_queue.dispatch_pending(client_state)?;
    Ok(())
}

struct GrabHarness {
    display: Display<KynexState>,
    state: KynexState,
    conn: Connection,
    event_queue: wayland_client::EventQueue<TestClientData>,
    client_state: TestClientData,
    _surface: wl_surface::WlSurface,
    xdg_toplevel: xdg_toplevel::XdgToplevel,
    seat: wl_seat::WlSeat,
}

impl GrabHarness {
    fn new() -> Result<Self, Box<dyn Error>> {
        let mut display: Display<KynexState> = Display::new()?;
        let initial_size = Size::from((1280, 720));
        let mut state = KynexState::new(
            &display.handle(),
            "wayland-grab-test".to_string(),
            initial_size,
        )?;

        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_data = Arc::new(ClientState::default());
        display.handle().insert_client(server_stream, client_data)?;

        let conn = Connection::from_socket(client_stream)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let mut client_state = TestClientData::default();

        let _registry = conn.display().get_registry(&qhandle, ());
        step_turn(
            &conn,
            &mut event_queue,
            &mut client_state,
            &mut display,
            &mut state,
        )?;

        let compositor = client_state
            .compositor
            .take()
            .ok_or("wl_compositor must be advertised")?;
        let wm_base = client_state
            .wm_base
            .take()
            .ok_or("xdg_wm_base must be advertised")?;
        let seat = client_state
            .seat
            .take()
            .ok_or("wl_seat must be advertised")?;

        let surface = compositor.create_surface(&qhandle, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qhandle, ());
        let xdg_toplevel = xdg_surface.get_toplevel(&qhandle, ());

        surface.commit();
        step_turn(
            &conn,
            &mut event_queue,
            &mut client_state,
            &mut display,
            &mut state,
        )?;

        surface.commit();
        step_turn(
            &conn,
            &mut event_queue,
            &mut client_state,
            &mut display,
            &mut state,
        )?;

        Ok(Self {
            display,
            state,
            conn,
            event_queue,
            client_state,
            _surface: surface,
            xdg_toplevel,
            seat,
        })
    }

    fn window_id(&self) -> Result<kx_layout::WindowId, Box<dyn Error>> {
        self.state
            .window_map
            .keys()
            .copied()
            .next()
            .ok_or_else(|| "WindowId not found".into())
    }
}

#[test]
fn test_client_move_request_invalid_serial_rejected() -> Result<(), Box<dyn Error>> {
    let mut h = GrabHarness::new()?;
    let pointer = h.state.seat.get_pointer().ok_or("Seat missing pointer")?;

    assert!(!pointer.is_grabbed());

    // Client requests move with invalid/fake serial
    h.xdg_toplevel._move(&h.seat, 0xdead_beef);
    step_turn(
        &h.conn,
        &mut h.event_queue,
        &mut h.client_state,
        &mut h.display,
        &mut h.state,
    )?;

    // Pointer must not have entered a grab
    assert!(!pointer.is_grabbed());
    Ok(())
}

#[test]
fn test_client_resize_request_invalid_serial_rejected() -> Result<(), Box<dyn Error>> {
    let mut h = GrabHarness::new()?;
    let pointer = h.state.seat.get_pointer().ok_or("Seat missing pointer")?;

    assert!(!pointer.is_grabbed());

    // Client requests resize with invalid/fake serial
    h.xdg_toplevel
        .resize(&h.seat, 0xdead_beef, xdg_toplevel::ResizeEdge::BottomRight);
    step_turn(
        &h.conn,
        &mut h.event_queue,
        &mut h.client_state,
        &mut h.display,
        &mut h.state,
    )?;

    // Pointer must not have entered a grab
    assert!(!pointer.is_grabbed());
    Ok(())
}

#[test]
fn test_compositor_move_grab_lifecycle() -> Result<(), Box<dyn Error>> {
    let mut h = GrabHarness::new()?;
    let wid = h.window_id()?;
    let pointer = h.state.seat.get_pointer().ok_or("Seat missing pointer")?;

    let initial_geo = h
        .state
        .window_manager
        .window(wid)
        .ok_or("window missing")?
        .geometry;

    let start_loc = Point::from((100.0, 100.0));
    let serial = SERIAL_COUNTER.next_serial();

    // Start compositor move grab directly
    start_compositor_move_grab(&mut h.state, wid, start_loc, BTN_LEFT, serial);
    assert!(pointer.is_grabbed(), "Pointer must be grabbed after move");

    // Move pointer by (50, 40)
    let move_serial = SERIAL_COUNTER.next_serial();
    pointer.motion(
        &mut h.state,
        None,
        &MotionEvent {
            location: Point::from((150.0, 140.0)),
            serial: move_serial,
            time: 100,
        },
    );

    let moved_geo = h
        .state
        .window_manager
        .window(wid)
        .ok_or("window missing")?
        .geometry;
    assert_eq!(moved_geo.x, initial_geo.x + 50);
    assert_eq!(moved_geo.y, initial_geo.y + 40);

    // Release mouse button to end grab
    let release_serial = SERIAL_COUNTER.next_serial();
    pointer.button(
        &mut h.state,
        &ButtonEvent {
            button: BTN_LEFT,
            state: ButtonState::Released,
            serial: release_serial,
            time: 120,
        },
    );

    assert!(
        !pointer.is_grabbed(),
        "Pointer grab must be unset upon button release"
    );
    Ok(())
}

#[test]
fn test_compositor_resize_grab_lifecycle() -> Result<(), Box<dyn Error>> {
    let mut h = GrabHarness::new()?;
    let wid = h.window_id()?;
    let pointer = h.state.seat.get_pointer().ok_or("Seat missing pointer")?;

    let initial_geo = h
        .state
        .window_manager
        .window(wid)
        .ok_or("window missing")?
        .geometry;

    let start_loc = Point::from((200.0, 200.0));
    let serial = SERIAL_COUNTER.next_serial();

    // Start compositor resize grab from BottomRight edge
    start_compositor_resize_grab(
        &mut h.state,
        wid,
        start_loc,
        ResizeEdge::BottomRight,
        BTN_RIGHT,
        serial,
    );
    assert!(pointer.is_grabbed(), "Pointer must be grabbed after resize");

    // Move pointer by (+60, +40)
    let resize_serial = SERIAL_COUNTER.next_serial();
    pointer.motion(
        &mut h.state,
        None,
        &MotionEvent {
            location: Point::from((260.0, 240.0)),
            serial: resize_serial,
            time: 200,
        },
    );

    let resized_geo = h
        .state
        .window_manager
        .window(wid)
        .ok_or("window missing")?
        .geometry;
    assert_eq!(resized_geo.width, initial_geo.width + 60);
    assert_eq!(resized_geo.height, initial_geo.height + 40);

    // Release right mouse button
    let release_serial = SERIAL_COUNTER.next_serial();
    pointer.button(
        &mut h.state,
        &ButtonEvent {
            button: BTN_RIGHT,
            state: ButtonState::Released,
            serial: release_serial,
            time: 220,
        },
    );

    assert!(
        !pointer.is_grabbed(),
        "Pointer grab must be unset after button release"
    );
    Ok(())
}

#[test]
fn test_try_start_compositor_grab_unmodified_click_ignored() -> Result<(), Box<dyn Error>> {
    let mut h = GrabHarness::new()?;
    let pointer = h.state.seat.get_pointer().ok_or("Seat missing pointer")?;
    let serial = SERIAL_COUNTER.next_serial();

    // Normal click without Super/Mod key should not initiate compositor grab
    let initiated =
        try_start_compositor_grab(&mut h.state, Point::from((50.0, 50.0)), BTN_LEFT, serial);
    assert!(!initiated);
    assert!(!pointer.is_grabbed());
    Ok(())
}

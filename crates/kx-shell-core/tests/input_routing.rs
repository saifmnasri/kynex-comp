// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration test verifying keyboard and pointer input routing from compositor to Wayland clients.

#![forbid(unsafe_code)]

use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientState, KynexState};
use smithay::backend::input::{
    ButtonState as BackendBtnState, KeyState as BackendKeyState, Keycode,
};
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::{ButtonEvent, MotionEvent};
use smithay::reexports::wayland_server::Display;
use smithay::utils::{Point, SERIAL_COUNTER, Size};
use wayland_client::protocol::{
    wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_surface,
};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

#[derive(Default)]
struct TestClientData {
    compositor: Option<wl_compositor::WlCompositor>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    seat: Option<wl_seat::WlSeat>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    configured: bool,
    keyboard_entered: bool,
    keys_received: Vec<(u32, wl_keyboard::KeyState)>,
    pointer_entered: bool,
    pointer_motions: Vec<(f64, f64)>,
    pointer_buttons: Vec<(u32, wl_pointer::ButtonState)>,
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

impl Dispatch<wl_seat::WlSeat, ()> for TestClientData {
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
            if caps.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qhandle, ()));
            }
            if caps.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qhandle, ()));
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for TestClientData {
    fn event(
        state: &mut Self,
        _keyboard: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Enter { .. } => state.keyboard_entered = true,
            wl_keyboard::Event::Leave { .. } => state.keyboard_entered = false,
            wl_keyboard::Event::Key {
                key,
                state: wayland_client::WEnum::Value(val),
                ..
            } => {
                state.keys_received.push((key, val));
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for TestClientData {
    fn event(
        state: &mut Self,
        _pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface_x,
                surface_y,
                ..
            } => {
                state.pointer_entered = true;
                state.pointer_motions.push((surface_x, surface_y));
            }
            wl_pointer::Event::Leave { .. } => state.pointer_entered = false,
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                state.pointer_motions.push((surface_x, surface_y));
            }
            wl_pointer::Event::Button {
                button,
                state: wayland_client::WEnum::Value(val),
                ..
            } => {
                state.pointer_buttons.push((button, val));
            }
            _ => {}
        }
    }
}

fn sync_turn(
    display: &mut Display<KynexState>,
    state: &mut KynexState,
    conn: &Connection,
    queue: &mut wayland_client::EventQueue<TestClientData>,
    client_data: &mut TestClientData,
) -> Result<(), Box<dyn Error>> {
    conn.flush()?;
    display.dispatch_clients(state)?;
    display.flush_clients()?;

    let read_guard = conn
        .prepare_read()
        .ok_or("Failed to prepare connection read")?;
    read_guard.read()?;
    queue.dispatch_pending(client_data)?;
    Ok(())
}

fn setup_test_client_window(
    display: &mut Display<KynexState>,
    state: &mut KynexState,
) -> Result<
    (
        Connection,
        wayland_client::EventQueue<TestClientData>,
        TestClientData,
    ),
    Box<dyn Error>,
> {
    let (client_stream, server_stream) = UnixStream::pair()?;
    display
        .handle()
        .insert_client(server_stream, Arc::new(ClientState::default()))?;

    let conn = Connection::from_socket(client_stream)?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let mut client_data = TestClientData::default();

    let _registry = conn.display().get_registry(&qh, ());
    sync_turn(display, state, &conn, &mut queue, &mut client_data)?;
    sync_turn(display, state, &conn, &mut queue, &mut client_data)?;

    let compositor = client_data
        .compositor
        .as_ref()
        .ok_or("missing compositor")?;
    let wm_base = client_data.wm_base.as_ref().ok_or("missing wm_base")?;
    let surface = compositor.create_surface(&qh, ());
    let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
    let _toplevel = xdg_surface.get_toplevel(&qh, ());
    surface.commit();

    sync_turn(display, state, &conn, &mut queue, &mut client_data)?;
    Ok((conn, queue, client_data))
}

#[test]
fn test_keyboard_input_routing() -> Result<(), Box<dyn Error>> {
    let mut display: Display<KynexState> = Display::new()?;
    let mut state = KynexState::new(
        &display.handle(),
        "wayland-input-test".to_string(),
        Size::from((1280, 720)),
    )?;

    let (conn, mut queue, mut client_data) = setup_test_client_window(&mut display, &mut state)?;
    assert!(client_data.configured, "Window should be configured");
    assert!(
        client_data.keyboard_entered,
        "Focused window should receive keyboard enter"
    );

    let keyboard = state.seat.get_keyboard().ok_or("missing seat keyboard")?;
    let serial = SERIAL_COUNTER.next_serial();
    keyboard.input::<(), _>(
        &mut state,
        Keycode::new(30),
        BackendKeyState::Pressed,
        serial,
        100,
        |_data, _modifiers, _keysym| FilterResult::Forward,
    );

    sync_turn(
        &mut display,
        &mut state,
        &conn,
        &mut queue,
        &mut client_data,
    )?;
    // Wayland wl_keyboard.key sends `keycode.raw() - 8` per XKB-to-evdev protocol conventions.
    let expected_wayland_key = 30 - 8;
    assert!(
        client_data
            .keys_received
            .iter()
            .any(|&(k, s)| k == expected_wayland_key && s == wl_keyboard::KeyState::Pressed),
        "Client should receive forwarded keypress event. Expected key {expected_wayland_key}, got: {:?}",
        client_data.keys_received
    );

    Ok(())
}

#[test]
fn test_pointer_input_routing() -> Result<(), Box<dyn Error>> {
    let mut display: Display<KynexState> = Display::new()?;
    let mut state = KynexState::new(
        &display.handle(),
        "wayland-pointer-test".to_string(),
        Size::from((1280, 720)),
    )?;

    let (conn, mut queue, mut client_data) = setup_test_client_window(&mut display, &mut state)?;

    let pointer = state.seat.get_pointer().ok_or("missing seat pointer")?;
    let window = state
        .space
        .elements()
        .next()
        .cloned()
        .ok_or("window missing in space")?;
    let focused_surface = window
        .toplevel()
        .ok_or("toplevel missing")?
        .wl_surface()
        .clone();

    let serial1 = SERIAL_COUNTER.next_serial();
    pointer.motion(
        &mut state,
        Some((focused_surface, Point::from((0.0, 0.0)))),
        &MotionEvent {
            location: Point::from((50.0, 50.0)),
            serial: serial1,
            time: 200,
        },
    );
    pointer.frame(&mut state);
    sync_turn(
        &mut display,
        &mut state,
        &conn,
        &mut queue,
        &mut client_data,
    )?;

    assert!(
        client_data.pointer_entered,
        "Client should receive pointer enter"
    );
    assert!(
        client_data
            .pointer_motions
            .iter()
            .any(|&(x, y)| (x - 50.0).abs() < 1e-4 && (y - 50.0).abs() < 1e-4),
        "Client should receive motion coordinates (50.0, 50.0)"
    );

    let serial2 = SERIAL_COUNTER.next_serial();
    pointer.button(
        &mut state,
        &ButtonEvent {
            serial: serial2,
            time: 250,
            button: 0x110,
            state: BackendBtnState::Pressed,
        },
    );
    pointer.frame(&mut state);
    sync_turn(
        &mut display,
        &mut state,
        &conn,
        &mut queue,
        &mut client_data,
    )?;

    assert!(
        client_data
            .pointer_buttons
            .iter()
            .any(|&(b, s)| b == 0x110 && s == wl_pointer::ButtonState::Pressed),
        "Client should receive button press event"
    );

    Ok(())
}

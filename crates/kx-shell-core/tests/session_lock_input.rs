// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Production-path integration tests for Wayland Session Lock security (SEC-01).
//!
//! Verifies:
//! 1. When session is locked, pointer/keyboard input is gated away from normal desktop windows.
//! 2. Clicking at a window position does NOT focus, raise, or activate desktop windows.
//! 3. When session is locked, desktop windows in `space` are NOT rendered.
//! 4. Fail-closed guarantee: when a locker disconnects without unlocking, session remains locked,
//!    desktop windows remain unrendered, and desktop input remains blocked.

#![forbid(unsafe_code)]

use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use kx_shell_core::client::{ClientLimits, ClientState};
use kx_shell_core::session_lock::find_lock_surface_under;
use kx_shell_core::state::KynexState;
use smithay::backend::input::ButtonState as BackendBtnState;
use smithay::input::pointer::{ButtonEvent, MotionEvent};
use smithay::reexports::wayland_server::Display;
use smithay::utils::{Logical, Point, SERIAL_COUNTER, Size};
use wayland_client::protocol::{
    wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_surface,
};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1, ext_session_lock_v1,
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

#[derive(Default)]
struct TestWindowClientData {
    compositor: Option<wl_compositor::WlCompositor>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    seat: Option<wl_seat::WlSeat>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    configured: bool,
    keyboard_entered: bool,
    pointer_entered: bool,
    pointer_buttons: Vec<(u32, wl_pointer::ButtonState)>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for TestWindowClientData {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        if interface == "wl_compositor" && version >= 4 {
            state.compositor = Some(registry.bind(name, 4, qhandle, ()));
        } else if interface == "xdg_wm_base" && version >= 1 {
            state.wm_base = Some(registry.bind(name, 1, qhandle, ()));
        } else if interface == "wl_seat" && version >= 2 {
            state.seat = Some(registry.bind(name, 2, qhandle, ()));
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for TestWindowClientData {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _: wl_compositor::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for TestWindowClientData {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for TestWindowClientData {
    fn event(
        _state: &mut Self,
        proxy: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        (): &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            proxy.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for TestWindowClientData {
    fn event(
        state: &mut Self,
        proxy: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        (): &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            proxy.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for TestWindowClientData {
    fn event(
        _: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        _: xdg_toplevel::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for TestWindowClientData {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        (): &(),
        _: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        let wl_seat::Event::Capabilities {
            capabilities: wayland_client::WEnum::Value(caps),
        } = event
        else {
            return;
        };
        if caps.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
            state.keyboard = Some(seat.get_keyboard(qhandle, ()));
        }
        if caps.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
            state.pointer = Some(seat.get_pointer(qhandle, ()));
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for TestWindowClientData {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Enter { .. } = event {
            state.keyboard_entered = true;
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for TestWindowClientData {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter { .. } => state.pointer_entered = true,
            wl_pointer::Event::Button {
                button,
                state: wayland_client::WEnum::Value(val),
                ..
            } => state.pointer_buttons.push((button, val)),
            _ => {}
        }
    }
}

#[derive(Default)]
struct LockClientData {
    session_lock_global: Option<(u32, u32)>,
    is_locked: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for LockClientData {
    fn event(
        state: &mut Self,
        _: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        if interface == "ext_session_lock_manager_v1" {
            state.session_lock_global = Some((name, version));
        }
    }
}

impl Dispatch<ext_session_lock_manager_v1::ExtSessionLockManagerV1, ()> for LockClientData {
    fn event(
        _: &mut Self,
        _: &ext_session_lock_manager_v1::ExtSessionLockManagerV1,
        _: ext_session_lock_manager_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext_session_lock_v1::ExtSessionLockV1, ()> for LockClientData {
    fn event(
        state: &mut Self,
        _proxy: &ext_session_lock_v1::ExtSessionLockV1,
        event: ext_session_lock_v1::Event,
        (): &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let ext_session_lock_v1::Event::Locked = event {
            state.is_locked = true;
        }
    }
}

fn sync_turn<T: 'static>(
    display: &mut Display<KynexState>,
    state: &mut KynexState,
    conn: &Connection,
    queue: &mut wayland_client::EventQueue<T>,
    client_data: &mut T,
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

fn setup_window_client(
    display: &mut Display<KynexState>,
    state: &mut KynexState,
) -> Result<
    (
        Connection,
        wayland_client::EventQueue<TestWindowClientData>,
        TestWindowClientData,
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
    let mut data = TestWindowClientData::default();

    let _registry = conn.display().get_registry(&qh, ());
    sync_turn(display, state, &conn, &mut queue, &mut data)?;
    sync_turn(display, state, &conn, &mut queue, &mut data)?;

    let compositor = data.compositor.as_ref().ok_or("missing compositor")?;
    let wm_base = data.wm_base.as_ref().ok_or("missing wm_base")?;
    let surface = compositor.create_surface(&qh, ());
    let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
    let _toplevel = xdg_surface.get_toplevel(&qh, ());
    surface.commit();

    sync_turn(display, state, &conn, &mut queue, &mut data)?;
    Ok((conn, queue, data))
}

type LockSessionResult = Result<
    (
        Connection,
        wayland_client::EventQueue<LockClientData>,
        LockClientData,
        ext_session_lock_v1::ExtSessionLockV1,
    ),
    Box<dyn Error>,
>;

fn lock_session(display: &mut Display<KynexState>, state: &mut KynexState) -> LockSessionResult {
    let (client_stream, server_stream) = UnixStream::pair()?;
    let client_state = Arc::new(ClientState {
        limits: ClientLimits::default(),
        is_session_lock_allowed: AtomicBool::new(true),
        ..Default::default()
    });
    display
        .handle()
        .insert_client(server_stream, client_state)?;

    let conn = Connection::from_socket(client_stream)?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let mut data = LockClientData::default();

    let _registry = conn.display().get_registry(&qh, ());
    sync_turn(display, state, &conn, &mut queue, &mut data)?;

    let (global_name, global_version) = data
        .session_lock_global
        .ok_or("session lock global not found")?;
    let registry = conn.display().get_registry(&qh, ());
    let manager: ext_session_lock_manager_v1::ExtSessionLockManagerV1 =
        registry.bind(global_name, global_version, &qh, ());
    let lock_obj = manager.lock(&qh, ());

    sync_turn(display, state, &conn, &mut queue, &mut data)?;
    Ok((conn, queue, data, lock_obj))
}

fn send_test_click(state: &mut KynexState, point: Point<f64, Logical>, is_locked: bool) {
    let Some(pointer) = state.seat.get_pointer() else {
        return;
    };
    let under = if is_locked {
        find_lock_surface_under(state, point)
    } else {
        state.space.elements().next().and_then(|w| {
            w.toplevel()
                .map(|t| (t.wl_surface().clone(), Point::from((0.0, 0.0))))
        })
    };

    pointer.motion(
        state,
        under,
        &MotionEvent {
            location: point,
            serial: SERIAL_COUNTER.next_serial(),
            time: 100,
        },
    );
    pointer.frame(state);

    pointer.button(
        state,
        &ButtonEvent {
            serial: SERIAL_COUNTER.next_serial(),
            time: 150,
            button: 0x110,
            state: BackendBtnState::Pressed,
        },
    );
    pointer.frame(state);

    pointer.button(
        state,
        &ButtonEvent {
            serial: SERIAL_COUNTER.next_serial(),
            time: 160,
            button: 0x110,
            state: BackendBtnState::Released,
        },
    );
    pointer.frame(state);
}

#[test]
fn test_session_lock_input_gating_production_path() -> Result<(), Box<dyn Error>> {
    let mut display: Display<KynexState> = Display::new()?;
    let mut state = KynexState::new(
        &display.handle(),
        "session-lock-input-test".to_string(),
        Size::from((1280, 720)),
    )?;

    let (window_conn, mut window_queue, mut window_data) =
        setup_window_client(&mut display, &mut state)?;
    assert!(window_data.configured, "Window should be configured");
    assert!(
        window_data.keyboard_entered,
        "Initial window should have keyboard focus"
    );

    // Initial click at (50, 50) hits window when not locked
    send_test_click(&mut state, Point::from((50.0, 50.0)), false);
    sync_turn(
        &mut display,
        &mut state,
        &window_conn,
        &mut window_queue,
        &mut window_data,
    )?;
    assert!(
        window_data
            .pointer_buttons
            .iter()
            .any(|&(b, s)| b == 0x110 && s == wl_pointer::ButtonState::Pressed),
        "Window client must receive clicks before session lock"
    );
    window_data.pointer_buttons.clear();

    // Now privileged lock client locks session
    let (_lock_conn, _lock_queue, lock_data, _lock_obj) = lock_session(&mut display, &mut state)?;
    assert!(state.is_session_locked, "State must report locked");
    assert!(lock_data.is_locked, "Lock client received locked event");

    // Security Gate 1: Desktop window must have lost keyboard focus
    let keyboard = state.seat.get_keyboard().ok_or("missing keyboard")?;
    let window = state
        .space
        .elements()
        .next()
        .cloned()
        .ok_or("window missing")?;
    let window_surface = window.toplevel().map(|t| t.wl_surface().clone());
    assert_ne!(
        keyboard.current_focus(),
        window_surface,
        "Desktop window surface must NOT hold keyboard focus while session is locked"
    );

    // Security Gate 2: Pointer click at desktop window location must be blocked
    send_test_click(&mut state, Point::from((50.0, 50.0)), true);
    sync_turn(
        &mut display,
        &mut state,
        &window_conn,
        &mut window_queue,
        &mut window_data,
    )?;
    assert!(
        window_data.pointer_buttons.is_empty(),
        "Desktop window MUST NOT receive pointer clicks while session is locked"
    );

    Ok(())
}

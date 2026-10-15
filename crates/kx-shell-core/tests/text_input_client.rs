// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for the text input protocol (`zwp_text_input_v3`).

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface as ServerWlSurface;
use smithay::utils::Size;
use wayland_client::protocol::{wl_callback, wl_compositor, wl_registry, wl_seat, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::{self, ZwpTextInputManagerV3},
    zwp_text_input_v3::{self, ZwpTextInputV3},
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReceivedTextInputEvent {
    Enter(String),
    Leave(String),
    Done(u32),
    CommitString(Option<String>),
}

#[derive(Default)]
struct TextInputTestClientState {
    manager_global: Option<(u32, u32)>,
    compositor_global: Option<(u32, u32)>,
    seat_global: Option<(u32, u32)>,
    events: Vec<ReceivedTextInputEvent>,
}

impl Dispatch<wl_callback::WlCallback, ()> for TextInputTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_callback::WlCallback,
        _event: wl_callback::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for TextInputTestClientState {
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
            if interface == "zwp_text_input_manager_v3" {
                state.manager_global = Some((name, version));
            } else if interface == "wl_compositor" {
                state.compositor_global = Some((name, version));
            } else if interface == "wl_seat" {
                state.seat_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for TextInputTestClientState {
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

impl Dispatch<wl_seat::WlSeat, ()> for TextInputTestClientState {
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

impl Dispatch<wl_surface::WlSurface, ()> for TextInputTestClientState {
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

impl Dispatch<ZwpTextInputManagerV3, ()> for TextInputTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTextInputManagerV3,
        _event: zwp_text_input_manager_v3::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpTextInputV3, ()> for TextInputTestClientState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpTextInputV3,
        event: zwp_text_input_v3::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwp_text_input_v3::Event::Enter { surface } => {
                state
                    .events
                    .push(ReceivedTextInputEvent::Enter(surface.id().to_string()));
            }
            zwp_text_input_v3::Event::Leave { surface } => {
                state
                    .events
                    .push(ReceivedTextInputEvent::Leave(surface.id().to_string()));
            }
            zwp_text_input_v3::Event::Done { serial } => {
                state.events.push(ReceivedTextInputEvent::Done(serial));
            }
            zwp_text_input_v3::Event::CommitString { text } => {
                state
                    .events
                    .push(ReceivedTextInputEvent::CommitString(text));
            }
            _ => {}
        }
    }
}

struct TextInputTestHarness {
    display: Display<KynexState>,
    state: KynexState,
    client: smithay::reexports::wayland_server::Client,
    conn: Connection,
    event_queue: EventQueue<TextInputTestClientState>,
    client_state: TextInputTestClientState,
}

impl TextInputTestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut display = Display::<KynexState>::new()?;
        let mut dh = display.handle();
        let mut state = KynexState::new(
            &dh,
            "kynex-test-text-input".to_string(),
            Size::from((1280, 720)),
        )?;

        let (server_stream, client_stream) = UnixStream::pair()?;
        let client = dh.insert_client(server_stream, Arc::new(ClientState::default()))?;
        let conn = Connection::from_socket(client_stream)?;
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();

        let _registry = conn.display().get_registry(&qh, ());
        let _sync = conn.display().sync(&qh, ());
        conn.flush()?;

        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;

        let read_guard = conn.prepare_read().ok_or("Failed to prepare client read")?;
        read_guard.read()?;

        let mut client_state = TextInputTestClientState::default();
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
        let qh = self.event_queue.handle();
        let _sync = self.conn.display().sync(&qh, ());
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
fn test_text_input_manager_advertised() -> Result<(), Box<dyn std::error::Error>> {
    let harness = TextInputTestHarness::new()?;

    let (mgr_name, version) = harness
        .client_state
        .manager_global
        .expect("zwp_text_input_manager_v3 global should be advertised");

    assert!(mgr_name > 0);
    assert_eq!(
        version, 1,
        "zwp_text_input_manager_v3 should advertise version 1"
    );
    Ok(())
}

#[test]
fn test_text_input_lifecycle_and_requests() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TextInputTestHarness::new()?;

    let (mgr_name, _) = harness
        .client_state
        .manager_global
        .expect("zwp_text_input_manager_v3 missing");
    let (seat_name, _) = harness
        .client_state
        .seat_global
        .expect("wl_seat global missing");

    let qh = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qh, ());
    let manager: ZwpTextInputManagerV3 = registry.bind(mgr_name, 1, &qh, ());
    let seat: wl_seat::WlSeat = registry.bind(seat_name, 1, &qh, ());

    harness.roundtrip()?;

    let text_input = manager.get_text_input(&seat, &qh, ());
    harness.roundtrip()?;
    assert!(
        text_input.is_alive(),
        "Text input should be created and alive"
    );

    // Send valid client requests
    text_input.enable();
    text_input.set_surrounding_text("hello world".to_string(), 5, 5);
    text_input.set_cursor_rectangle(10, 20, 100, 30);
    text_input.commit();
    harness.roundtrip()?;

    // Disable and commit
    text_input.disable();
    text_input.commit();
    harness.roundtrip()?;

    // Destroy
    text_input.destroy();
    manager.destroy();
    harness.roundtrip()?;

    assert!(!text_input.is_alive(), "Text input should be destroyed");
    Ok(())
}

#[test]
fn test_text_input_focus_enter_and_leave() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TextInputTestHarness::new()?;

    let (mgr_name, _) = harness
        .client_state
        .manager_global
        .expect("zwp_text_input_manager_v3 missing");
    let (comp_name, _) = harness
        .client_state
        .compositor_global
        .expect("wl_compositor missing");
    let (seat_name, _) = harness.client_state.seat_global.expect("wl_seat missing");

    let qh = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qh, ());
    let manager: ZwpTextInputManagerV3 = registry.bind(mgr_name, 1, &qh, ());
    let compositor: wl_compositor::WlCompositor = registry.bind(comp_name, 1, &qh, ());
    let seat: wl_seat::WlSeat = registry.bind(seat_name, 1, &qh, ());

    harness.roundtrip()?;

    let surface = compositor.create_surface(&qh, ());
    let text_input = manager.get_text_input(&seat, &qh, ());
    harness.roundtrip()?;

    // Resolve server-side surface
    let server_surface = harness
        .client
        .object_from_protocol_id::<ServerWlSurface>(
            &harness.display.handle(),
            surface.id().protocol_id(),
        )
        .expect("Server surface not found");

    // Enable text input for client
    text_input.enable();
    text_input.commit();
    harness.roundtrip()?;

    // Set keyboard focus on this surface in compositor
    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
    let keyboard = harness
        .state
        .seat
        .get_keyboard()
        .expect("Seat should have keyboard");
    keyboard.set_focus(&mut harness.state, Some(server_surface.clone()), serial);
    harness.roundtrip()?;

    // Client should have received Enter event
    assert!(
        harness
            .client_state
            .events
            .iter()
            .any(|e| matches!(e, ReceivedTextInputEvent::Enter(_))),
        "Expected Enter event on text_input when surface gained focus"
    );

    // Clear focus
    let serial2 = smithay::utils::SERIAL_COUNTER.next_serial();
    keyboard.set_focus(&mut harness.state, None, serial2);
    harness.roundtrip()?;

    // Client should have received Leave event
    assert!(
        harness
            .client_state
            .events
            .iter()
            .any(|e| matches!(e, ReceivedTextInputEvent::Leave(_))),
        "Expected Leave event on text_input when surface lost focus"
    );

    text_input.destroy();
    manager.destroy();
    harness.roundtrip()?;
    Ok(())
}

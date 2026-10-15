// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for linux-dmabuf v4 feedback and format table advertising.

use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use kx_shell_core::{ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_buffer_params_v1, zwp_linux_dmabuf_feedback_v1, zwp_linux_dmabuf_v1,
};

#[derive(Default)]
struct DmabufTestClientState {
    dmabuf_global: Option<(u32, u32)>,
    main_device_received: Arc<AtomicBool>,
    format_table_received: Arc<AtomicBool>,
    feedback_done_received: Arc<AtomicBool>,
    buffer_failed_received: Arc<AtomicBool>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for DmabufTestClientState {
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
            if interface == "zwp_linux_dmabuf_v1" {
                state.dmabuf_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1, ()> for DmabufTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
        _event: zwp_linux_dmabuf_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1, ()>
    for DmabufTestClientState
{
    fn event(
        state: &mut Self,
        _proxy: &zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1,
        event: zwp_linux_dmabuf_feedback_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwp_linux_dmabuf_feedback_v1::Event::MainDevice { .. } => {
                state.main_device_received.store(true, Ordering::SeqCst);
            }
            zwp_linux_dmabuf_feedback_v1::Event::FormatTable { .. } => {
                state.format_table_received.store(true, Ordering::SeqCst);
            }
            zwp_linux_dmabuf_feedback_v1::Event::Done => {
                state.feedback_done_received.store(true, Ordering::SeqCst);
            }
            _ => {}
        }
    }
}

impl Dispatch<zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1, ()> for DmabufTestClientState {
    fn event(
        state: &mut Self,
        _proxy: &zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
        event: zwp_linux_buffer_params_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let zwp_linux_buffer_params_v1::Event::Failed = event {
            state.buffer_failed_received.store(true, Ordering::SeqCst);
        }
    }
}

#[test]
fn test_dmabuf_v4_global_advertised_and_feedback_received() {
    let mut display: Display<KynexState> = Display::new().unwrap();
    let initial_size = Size::from((1280, 720));
    let mut state = KynexState::new(
        &display.handle(),
        "dmabuf-test-sock".to_string(),
        initial_size,
    )
    .unwrap();

    let (server_stream, client_stream) = UnixStream::pair().unwrap();
    let client_data = Arc::new(ClientState::default());
    display
        .handle()
        .insert_client(server_stream, client_data)
        .unwrap();

    let conn = Connection::from_socket(client_stream).unwrap();
    let mut event_queue = conn.new_event_queue();
    let qhandle = event_queue.handle();

    let display_proxy = conn.display();
    let registry = display_proxy.get_registry(&qhandle, ());

    // Roundtrip 1: discover globals
    conn.flush().unwrap();
    display.dispatch_clients(&mut state).unwrap();
    display.flush_clients().unwrap();

    let read_guard = conn.prepare_read().unwrap();
    read_guard.read().unwrap();

    let mut client_state = DmabufTestClientState::default();
    event_queue.dispatch_pending(&mut client_state).unwrap();

    let (dmabuf_name, dmabuf_version) = client_state
        .dmabuf_global
        .expect("zwp_linux_dmabuf_v1 global must be advertised");
    assert!(
        dmabuf_version >= 4,
        "zwp_linux_dmabuf_v1 should be advertised at version >= 4, got {dmabuf_version}"
    );

    // Bind zwp_linux_dmabuf_v1 at version 4
    let dmabuf: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1 = registry.bind(dmabuf_name, 4, &qhandle, ());

    // Request default feedback
    let _feedback = dmabuf.get_default_feedback(&qhandle, ());

    // Roundtrip 2: dispatch feedback request and receive events
    conn.flush().unwrap();
    display.dispatch_clients(&mut state).unwrap();
    display.flush_clients().unwrap();

    let read_guard2 = conn.prepare_read().unwrap();
    read_guard2.read().unwrap();
    event_queue.dispatch_pending(&mut client_state).unwrap();

    assert!(
        client_state.main_device_received.load(Ordering::SeqCst),
        "Client must receive MainDevice event"
    );
    assert!(
        client_state.format_table_received.load(Ordering::SeqCst),
        "Client must receive FormatTable event"
    );
    assert!(
        client_state.feedback_done_received.load(Ordering::SeqCst),
        "Client must receive Done event for default feedback"
    );
}

#[test]
fn test_dmabuf_hostile_unsupported_format_disconnects_client() {
    let mut display: Display<KynexState> = Display::new().unwrap();
    let initial_size = Size::from((1280, 720));
    let mut state = KynexState::new(
        &display.handle(),
        "dmabuf-hostile-sock".to_string(),
        initial_size,
    )
    .unwrap();

    let (server_stream, client_stream) = UnixStream::pair().unwrap();
    let client_data = Arc::new(ClientState::default());
    display
        .handle()
        .insert_client(server_stream, client_data)
        .unwrap();

    let conn = Connection::from_socket(client_stream).unwrap();
    let mut event_queue = conn.new_event_queue();
    let qhandle = event_queue.handle();

    let display_proxy = conn.display();
    let registry = display_proxy.get_registry(&qhandle, ());

    conn.flush().unwrap();
    display.dispatch_clients(&mut state).unwrap();
    display.flush_clients().unwrap();

    let read_guard = conn.prepare_read().unwrap();
    read_guard.read().unwrap();

    let mut client_state = DmabufTestClientState::default();
    event_queue.dispatch_pending(&mut client_state).unwrap();

    let (dmabuf_name, _) = client_state
        .dmabuf_global
        .expect("zwp_linux_dmabuf_v1 global must be advertised");

    let dmabuf: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1 = registry.bind(dmabuf_name, 4, &qhandle, ());
    let params = dmabuf.create_params(&qhandle, ());

    // Hostile client: request buffer creation with unsupported format 0
    params.create(100, 100, 0, zwp_linux_buffer_params_v1::Flags::empty());

    conn.flush().unwrap();
    display.dispatch_clients(&mut state).unwrap();
    display.flush_clients().unwrap();

    // Client read must see protocol error and server must not crash
    let read_guard2 = conn.prepare_read().unwrap();
    let read_result = read_guard2.read();
    assert!(
        read_result.is_err(),
        "Server must disconnect client with protocol error on unsupported format"
    );
}

#[test]
fn test_dmabuf_hostile_zero_dimension_rejected() {
    let mut display: Display<KynexState> = Display::new().unwrap();
    let initial_size = Size::from((1280, 720));
    let mut state = KynexState::new(
        &display.handle(),
        "dmabuf-zero-dim-sock".to_string(),
        initial_size,
    )
    .unwrap();

    let (server_stream, client_stream) = UnixStream::pair().unwrap();
    let client_data = Arc::new(ClientState::default());
    display
        .handle()
        .insert_client(server_stream, client_data)
        .unwrap();

    let conn = Connection::from_socket(client_stream).unwrap();
    let mut event_queue = conn.new_event_queue();
    let qhandle = event_queue.handle();

    let display_proxy = conn.display();
    let registry = display_proxy.get_registry(&qhandle, ());

    conn.flush().unwrap();
    display.dispatch_clients(&mut state).unwrap();
    display.flush_clients().unwrap();

    let read_guard = conn.prepare_read().unwrap();
    read_guard.read().unwrap();

    let mut client_state = DmabufTestClientState::default();
    event_queue.dispatch_pending(&mut client_state).unwrap();

    let (dmabuf_name, _) = client_state
        .dmabuf_global
        .expect("zwp_linux_dmabuf_v1 global must be advertised");

    let dmabuf: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1 = registry.bind(dmabuf_name, 4, &qhandle, ());
    let params = dmabuf.create_params(&qhandle, ());

    // Hostile client: request buffer creation with ARGB8888 (0x34325241) but width/height = 0
    params.create(
        0,
        0,
        0x3432_5241,
        zwp_linux_buffer_params_v1::Flags::empty(),
    );

    conn.flush().unwrap();
    display.dispatch_clients(&mut state).unwrap();
    display.flush_clients().unwrap();

    // The compositor enforces invalid_dimensions or sends failed event, server stays robust
    let read_guard2 = conn.prepare_read().unwrap();
    let read_result = read_guard2.read();
    if read_result.is_ok() {
        let _ = event_queue.dispatch_pending(&mut client_state);
        assert!(
            client_state.buffer_failed_received.load(Ordering::SeqCst),
            "Client should receive Failed event when zero dimensions are rejected"
        );
    }
}

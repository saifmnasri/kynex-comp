// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Protocol client integration tests for core globals.

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, QueueHandle};

#[derive(Default)]
struct TestClientData {
    globals: Vec<(String, u32)>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for TestClientData {
    fn event(
        state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name: _,
            interface,
            version,
        } = event
        {
            state.globals.push((interface, version));
        }
    }
}

#[test]
fn test_wayland_client_connect_and_bind_globals() {
    let mut display: Display<KynexState> = Display::new().unwrap();
    let initial_size = Size::from((1280, 720));
    let mut state = KynexState::new(
        &display.handle(),
        "wayland-test-sock".to_string(),
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
    let _registry = display_proxy.get_registry(&qhandle, ());

    // 1. Client flushes get_registry to server
    conn.flush().unwrap();

    // 2. Server dispatches request and flushes global announcements to client
    display.dispatch_clients(&mut state).unwrap();
    display.flush_clients().unwrap();

    // 3. Client reads events from socket and dispatches queue
    let read_guard = conn.prepare_read().unwrap();
    read_guard.read().unwrap();

    let mut client_state = TestClientData::default();
    event_queue.dispatch_pending(&mut client_state).unwrap();

    let global_names: Vec<String> = client_state
        .globals
        .iter()
        .map(|(name, _)| name.clone())
        .collect();

    assert!(global_names.contains(&"wl_compositor".to_string()));
    assert!(global_names.contains(&"wl_subcompositor".to_string()));
    assert!(global_names.contains(&"wl_shm".to_string()));
    assert!(global_names.contains(&"wl_seat".to_string()));
    assert!(global_names.contains(&"wl_output".to_string()));
}

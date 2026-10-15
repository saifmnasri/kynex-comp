// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for the Wayland Session Lock protocol (`ext_session_lock_v1`).
//!
//! Covers:
//! - Unauthorized/unprivileged client filtering (`ext_session_lock_manager_v1` hidden from registry).
//! - Privileged client registry discovery (`ext_session_lock_manager_v1` advertised with version 1).
//! - Privileged client session lock lifecycle (lock, confirmation, and unlocking).
//! - Fail-closed guarantee per Rule 05 (compositor remains locked if client crashes or disconnects).

#![forbid(unsafe_code)]

use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use kx_shell_core::client::{ClientLimits, ClientState};
use kx_shell_core::state::KynexState;
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1, ext_session_lock_v1,
};

#[derive(Default)]
struct LockTestClientData {
    compositor_global: Option<(u32, u32)>,
    session_lock_global: Option<(u32, u32)>,
    all_globals: Vec<(String, u32)>,
    is_locked_event_received: bool,
    is_finished_event_received: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for LockTestClientData {
    fn event(
        state: &mut Self,
        _registry: &wl_registry::WlRegistry,
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
            state.all_globals.push((interface.clone(), version));
            if interface == "wl_compositor" {
                state.compositor_global = Some((name, version));
            } else if interface == "ext_session_lock_manager_v1" {
                state.session_lock_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for LockTestClientData {
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

impl Dispatch<ext_session_lock_manager_v1::ExtSessionLockManagerV1, ()> for LockTestClientData {
    fn event(
        _state: &mut Self,
        _proxy: &ext_session_lock_manager_v1::ExtSessionLockManagerV1,
        _event: ext_session_lock_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext_session_lock_v1::ExtSessionLockV1, ()> for LockTestClientData {
    fn event(
        state: &mut Self,
        _proxy: &ext_session_lock_v1::ExtSessionLockV1,
        event: ext_session_lock_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            ext_session_lock_v1::Event::Locked => {
                state.is_locked_event_received = true;
            }
            ext_session_lock_v1::Event::Finished => {
                state.is_finished_event_received = true;
            }
            _ => {}
        }
    }
}

type ClientHandle = (
    Connection,
    wayland_client::EventQueue<LockTestClientData>,
    LockTestClientData,
    Arc<ClientState>,
    smithay::reexports::wayland_server::Client,
);

struct TestHarness {
    display: Display<KynexState>,
    state: KynexState,
}

impl TestHarness {
    fn new() -> Result<Self, Box<dyn Error>> {
        let display = Display::new()?;
        let state = KynexState::new(
            &display.handle(),
            "wayland-session-lock-test".to_string(),
            Size::from((1280, 720)),
        )?;
        Ok(Self { display, state })
    }

    fn connect_client(
        &mut self,
        is_session_lock_allowed: bool,
    ) -> Result<ClientHandle, Box<dyn Error>> {
        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_state = Arc::new(ClientState {
            limits: ClientLimits::default(),
            is_session_lock_allowed: AtomicBool::new(is_session_lock_allowed),
            ..Default::default()
        });
        let client = self
            .display
            .handle()
            .insert_client(server_stream, client_state.clone())?;

        let conn = Connection::from_socket(client_stream)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let mut client_data = LockTestClientData::default();

        let _registry = conn.display().get_registry(&qhandle, ());
        conn.flush()?;
        self.display.dispatch_clients(&mut self.state)?;
        self.display.flush_clients()?;

        let read_guard = conn.prepare_read().ok_or("Failed to prepare client read")?;
        read_guard.read()?;
        event_queue.dispatch_pending(&mut client_data)?;

        Ok((conn, event_queue, client_data, client_state, client))
    }
}

#[test]
fn test_unprivileged_client_session_lock_global_hidden() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (_conn, _queue, data, _client_state, _client) = harness.connect_client(false)?;

    assert!(
        data.compositor_global.is_some(),
        "Core compositor global must be advertised"
    );
    assert!(
        data.session_lock_global.is_none(),
        "ext_session_lock_manager_v1 must be hidden from unprivileged clients"
    );
    assert!(
        !data
            .all_globals
            .iter()
            .any(|(interface, _)| interface == "ext_session_lock_manager_v1"),
        "Registry event stream must not contain ext_session_lock_manager_v1"
    );
    Ok(())
}

#[test]
fn test_privileged_client_session_lock_global_advertised() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (_conn, _queue, data, _client_state, _client) = harness.connect_client(true)?;

    assert!(
        data.compositor_global.is_some(),
        "Core compositor global must be advertised"
    );
    assert!(
        data.session_lock_global.is_some(),
        "ext_session_lock_manager_v1 must be advertised to privileged clients"
    );

    let (global_name, global_version) = data.session_lock_global.ok_or("Global missing")?;
    assert!(global_name > 0, "Global name must be valid u32 identifier");
    assert_eq!(
        global_version, 1,
        "ext_session_lock_manager_v1 version must be 1"
    );
    Ok(())
}

#[test]
fn test_privileged_client_lock_and_unlock_lifecycle() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (conn, mut queue, data, _client_state, _client) = harness.connect_client(true)?;

    let (global_name, global_version) = data
        .session_lock_global
        .ok_or("Session lock global not advertised")?;

    let qhandle = queue.handle();
    let registry = conn.display().get_registry(&qhandle, ());
    let lock_manager: ext_session_lock_manager_v1::ExtSessionLockManagerV1 =
        registry.bind(global_name, global_version, &qhandle, ());

    assert!(
        !harness.state.is_session_locked,
        "Initial session must not be locked"
    );

    // Request session lock
    let lock_obj = lock_manager.lock(&qhandle, ());
    conn.flush()?;

    // Server dispatches request
    harness.display.dispatch_clients(&mut harness.state)?;
    harness.display.flush_clients()?;

    assert!(
        harness.state.is_session_locked,
        "Compositor must be locked after processing lock request"
    );

    // Client receives locked confirmation event
    let mut client_data = LockTestClientData::default();
    let read_guard = conn.prepare_read().ok_or("Failed to prepare client read")?;
    read_guard.read()?;
    queue.dispatch_pending(&mut client_data)?;

    assert!(
        client_data.is_locked_event_received,
        "Client must receive locked event confirmation"
    );

    // Unlock and destroy
    lock_obj.unlock_and_destroy();
    conn.flush()?;

    harness.display.dispatch_clients(&mut harness.state)?;
    harness.display.flush_clients()?;

    assert!(
        !harness.state.is_session_locked,
        "Compositor must be unlocked after client unlocks and destroys"
    );

    Ok(())
}

#[test]
fn test_session_lock_fail_closed_on_client_disconnect() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let (conn, queue, data, _client_state, _client) = harness.connect_client(true)?;

    let (global_name, global_version) = data
        .session_lock_global
        .ok_or("Session lock global not advertised")?;

    let qhandle = queue.handle();
    let registry = conn.display().get_registry(&qhandle, ());
    let lock_manager: ext_session_lock_manager_v1::ExtSessionLockManagerV1 =
        registry.bind(global_name, global_version, &qhandle, ());

    // Request session lock
    let _lock_obj = lock_manager.lock(&qhandle, ());
    conn.flush()?;

    harness.display.dispatch_clients(&mut harness.state)?;
    harness.display.flush_clients()?;

    assert!(harness.state.is_session_locked, "Compositor must be locked");

    // Simulate locker process crash / disconnect: client drops connection without unlock
    drop(conn);

    // Server dispatches remaining client events and detects disconnect
    let _ = harness.display.dispatch_clients(&mut harness.state);
    let _ = harness.display.flush_clients();

    // FAIL-CLOSED SECURITY INVARIANT (Rule 05):
    // If the locker disconnects or crashes without unlocking, the compositor MUST STAY LOCKED.
    assert!(
        harness.state.is_session_locked,
        "Compositor must fail closed and remain locked if lock client crashes"
    );

    Ok(())
}

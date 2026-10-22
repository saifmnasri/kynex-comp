// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for idle notify protocol (`ext_idle_notifier_v1`).

#![forbid(unsafe_code)]

use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;

use kx_shell_core::{ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notification_v1::{
    self, ExtIdleNotificationV1,
};
use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notifier_v1::{
    self, ExtIdleNotifierV1,
};

#[derive(Default)]
struct IdleNotifyTestClientState {
    notifier_global: Option<(u32, u32)>,
    seat_global: Option<(u32, u32)>,
    idled_count: u32,
    resumed_count: u32,
}

impl Dispatch<wl_registry::WlRegistry, ()> for IdleNotifyTestClientState {
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
            if interface == "ext_idle_notifier_v1" {
                state.notifier_global = Some((name, version));
            } else if interface == "wl_seat" {
                state.seat_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<ExtIdleNotifierV1, ()> for IdleNotifyTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &ExtIdleNotifierV1,
        _event: ext_idle_notifier_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtIdleNotificationV1, ()> for IdleNotifyTestClientState {
    fn event(
        state: &mut Self,
        _proxy: &ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            ext_idle_notification_v1::Event::Idled => {
                state.idled_count = state.idled_count.saturating_add(1);
            }
            ext_idle_notification_v1::Event::Resumed => {
                state.resumed_count = state.resumed_count.saturating_add(1);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for IdleNotifyTestClientState {
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

struct TestHarness {
    display: Display<KynexState>,
    state: KynexState,
    conn: Connection,
    event_queue: EventQueue<IdleNotifyTestClientState>,
    client_state: IdleNotifyTestClientState,
}

impl TestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let display = Display::new()?;
        let dh = display.handle();

        let state = KynexState::new(
            &dh,
            "kynex-test-idle-notify".to_string(),
            Size::from((1280, 800)),
        )?;

        let (server_stream, client_stream) = UnixStream::pair()?;
        server_stream.set_nonblocking(true)?;
        client_stream.set_nonblocking(true)?;

        let client_state = Arc::new(ClientState::default());
        display
            .handle()
            .insert_client(server_stream, client_state)?;

        let conn = Connection::from_socket(client_stream)?;
        let event_queue = conn.new_event_queue();
        let client_state = IdleNotifyTestClientState::default();

        let mut harness = Self {
            display,
            state,
            conn,
            event_queue,
            client_state,
        };

        let qh = harness.event_queue.handle();
        let display_proxy = harness.conn.display();
        let _registry = display_proxy.get_registry(&qh, ());
        harness.sync_roundtrip()?;

        Ok(harness)
    }

    fn sync_roundtrip(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for _ in 0..3 {
            self.conn.flush()?;
            self.display.dispatch_clients(&mut self.state)?;
            self.display.flush_clients()?;

            if let Some(read_guard) = self.conn.prepare_read() {
                let _ = read_guard.read();
            }
            self.event_queue.dispatch_pending(&mut self.client_state)?;
        }
        Ok(())
    }
}

#[test]
fn test_ext_idle_notify_global_advertised_and_bindable() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TestHarness::new()?;

    let (mgr_name, version) = harness
        .client_state
        .notifier_global
        .ok_or("ext_idle_notifier_v1 global not advertised")?;
    assert!(
        version >= 1,
        "ext_idle_notifier_v1 should advertise at least version 1"
    );

    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let registry = display_proxy.get_registry(&qh, ());
    let notifier: ExtIdleNotifierV1 = registry.bind(mgr_name, 1, &qh, ());

    harness.sync_roundtrip()?;
    notifier.destroy();
    harness.sync_roundtrip()?;
    Ok(())
}

#[test]
fn test_ext_idle_notification_lifecycle_and_destruction() -> Result<(), Box<dyn std::error::Error>>
{
    let mut harness = TestHarness::new()?;

    let (mgr_name, _) = harness
        .client_state
        .notifier_global
        .ok_or("ext_idle_notifier_v1 global not advertised")?;
    let (seat_name, _) = harness
        .client_state
        .seat_global
        .ok_or("wl_seat global not advertised")?;

    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let registry = display_proxy.get_registry(&qh, ());

    let notifier: ExtIdleNotifierV1 = registry.bind(mgr_name, 1, &qh, ());
    let seat: wl_seat::WlSeat = registry.bind(seat_name, 1, &qh, ());

    let notification: ExtIdleNotificationV1 = notifier.get_idle_notification(500, &seat, &qh, ());

    harness.sync_roundtrip()?;

    // Clean destruction
    notification.destroy();
    notifier.destroy();
    harness.sync_roundtrip()?;

    Ok(())
}

#[test]
fn test_ext_idle_notification_idled_and_resumed() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TestHarness::new()?;

    let (mgr_name, _) = harness
        .client_state
        .notifier_global
        .ok_or("ext_idle_notifier_v1 global not advertised")?;
    let (seat_name, _) = harness
        .client_state
        .seat_global
        .ok_or("wl_seat global not advertised")?;

    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let registry = display_proxy.get_registry(&qh, ());

    let notifier: ExtIdleNotifierV1 = registry.bind(mgr_name, 1, &qh, ());
    let seat: wl_seat::WlSeat = registry.bind(seat_name, 1, &qh, ());

    let notification: ExtIdleNotificationV1 = notifier.get_idle_notification(50, &seat, &qh, ());

    harness.sync_roundtrip()?;
    assert_eq!(harness.client_state.idled_count, 0);

    // Dispatch Calloop event loop with timeout to trigger idle state timer
    harness
        .state
        .dispatch_idle_timers(Some(Duration::from_millis(70)))?;
    harness.sync_roundtrip()?;

    assert_eq!(
        harness.client_state.idled_count, 1,
        "client should receive idled event after threshold expires"
    );

    // Now simulate user activity on seat
    let server_seat = harness.state.seat.clone();
    harness.state.notify_idle_activity(&server_seat);
    harness.sync_roundtrip()?;

    assert_eq!(
        harness.client_state.resumed_count, 1,
        "client should receive resumed event after activity notification"
    );

    notification.destroy();
    notifier.destroy();
    harness.sync_roundtrip()?;

    Ok(())
}

#[test]
fn test_idle_notify_production_loop_and_input() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = TestHarness::new()?;

    let (mgr_name, _) = harness
        .client_state
        .notifier_global
        .ok_or("ext_idle_notifier_v1 global not advertised")?;
    let (seat_name, _) = harness
        .client_state
        .seat_global
        .ok_or("wl_seat global not advertised")?;

    let qh = harness.event_queue.handle();
    let display_proxy = harness.conn.display();
    let registry = display_proxy.get_registry(&qh, ());

    let notifier: ExtIdleNotifierV1 = registry.bind(mgr_name, 1, &qh, ());
    let seat: wl_seat::WlSeat = registry.bind(seat_name, 1, &qh, ());

    let notification: ExtIdleNotificationV1 = notifier.get_idle_notification(50, &seat, &qh, ());

    harness.sync_roundtrip()?;
    assert_eq!(harness.client_state.idled_count, 0);

    // Call production process_input_activity which resets the idle timer
    harness.state.process_input_activity();

    // Now dispatch idle timers through compositor event loop dispatch with 70ms timeout
    harness
        .state
        .dispatch_idle_timers(Some(Duration::from_millis(70)))?;
    harness.sync_roundtrip()?;

    assert_eq!(
        harness.client_state.idled_count, 1,
        "client should receive idled event when timer expires"
    );

    // Now test that production process_input_activity wakes the idle notifier without calling notify_idle_activity directly
    harness.state.process_input_activity();
    harness.sync_roundtrip()?;

    assert_eq!(
        harness.client_state.resumed_count, 1,
        "client should receive resumed event after process_input_activity"
    );

    notification.destroy();
    notifier.destroy();
    harness.sync_roundtrip()?;

    Ok(())
}

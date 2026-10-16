// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for the Wayland primary selection protocol
//! (`zwp_primary_selection_device_manager_v1`).
//!
//! Covers:
//! - Registry advertising and binding of `zwp_primary_selection_device_manager_v1`.
//! - Creation and management of `zwp_primary_selection_device_v1` for a seat.
//! - Creation, mime offer, and destruction of `zwp_primary_selection_source_v1`.

#![forbid(unsafe_code)]

use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::client::ClientState;
use kx_shell_core::state::KynexState;
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::wp::primary_selection::zv1::client::{
    zwp_primary_selection_device_manager_v1, zwp_primary_selection_device_v1,
    zwp_primary_selection_offer_v1, zwp_primary_selection_source_v1,
};

#[derive(Default)]
struct PrimarySelectionClientData {
    seat_global: Option<(u32, u32)>,
    primary_selection_global: Option<(u32, u32)>,
    offers_received: Vec<zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for PrimarySelectionClientData {
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
            if interface == "wl_seat" {
                state.seat_global = Some((name, version));
            } else if interface == "zwp_primary_selection_device_manager_v1" {
                state.primary_selection_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for PrimarySelectionClientData {
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

impl Dispatch<zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1, ()>
    for PrimarySelectionClientData
{
    fn event(
        _state: &mut Self,
        _proxy: &zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1,
        _event: zwp_primary_selection_device_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1, ()>
    for PrimarySelectionClientData
{
    fn event(
        state: &mut Self,
        _proxy: &zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1,
        event: zwp_primary_selection_device_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let zwp_primary_selection_device_v1::Event::DataOffer { offer } = event {
            state.offers_received.push(offer);
        }
    }
}

impl Dispatch<zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1, ()>
    for PrimarySelectionClientData
{
    fn event(
        _state: &mut Self,
        _proxy: &zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1,
        _event: zwp_primary_selection_source_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1, ()>
    for PrimarySelectionClientData
{
    fn event(
        _state: &mut Self,
        _proxy: &zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1,
        _event: zwp_primary_selection_offer_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

struct TestHarness {
    display: Display<KynexState>,
    state: KynexState,
    client_conn: Connection,
    event_queue: EventQueue<PrimarySelectionClientData>,
    client_data: PrimarySelectionClientData,
}

impl TestHarness {
    fn new() -> Result<Self, Box<dyn Error>> {
        let display = Display::<KynexState>::new()?;
        let dh = display.handle();
        let state = KynexState::new(&dh, "wayland-test".to_string(), Size::from((1280, 800)))?;

        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_state = Arc::new(ClientState::default());
        display
            .handle()
            .insert_client(server_stream, client_state)?;

        let client_conn = Connection::from_socket(client_stream)?;
        let event_queue = client_conn.new_event_queue();
        let client_data = PrimarySelectionClientData::default();

        Ok(Self {
            display,
            state,
            client_conn,
            event_queue,
            client_data,
        })
    }

    fn sync_roundtrip(&mut self) -> Result<(), Box<dyn Error>> {
        for _ in 0..3 {
            self.client_conn.flush()?;
            self.display.dispatch_clients(&mut self.state)?;
            self.display.flush_clients()?;

            if let Some(read_guard) = self.client_conn.prepare_read() {
                let _ = read_guard.read();
            }
            self.event_queue.dispatch_pending(&mut self.client_data)?;
        }
        Ok(())
    }
}

#[test]
fn test_client_binds_primary_selection_manager_global() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let qh = harness.event_queue.handle();

    let display_proxy = harness.client_conn.display();
    let registry = display_proxy.get_registry(&qh, ());
    harness.sync_roundtrip()?;

    let (name, version) = harness
        .client_data
        .primary_selection_global
        .ok_or("zwp_primary_selection_device_manager_v1 must be advertised in registry")?;
    assert_eq!(version, 1, "protocol version must be 1");

    let _manager: zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1 =
        registry.bind(name, 1, &qh, ());
    harness.sync_roundtrip()?;

    Ok(())
}

#[test]
fn test_primary_selection_device_and_source_lifecycle() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let qh = harness.event_queue.handle();

    let display_proxy = harness.client_conn.display();
    let registry = display_proxy.get_registry(&qh, ());
    harness.sync_roundtrip()?;

    let (seat_name, seat_ver) = harness
        .client_data
        .seat_global
        .ok_or("wl_seat must be advertised in registry")?;
    let seat: wl_seat::WlSeat = registry.bind(seat_name, seat_ver.min(5), &qh, ());

    let (mgr_name, _) = harness
        .client_data
        .primary_selection_global
        .ok_or("zwp_primary_selection_device_manager_v1 must be advertised")?;
    let manager: zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1 =
        registry.bind(mgr_name, 1, &qh, ());
    harness.sync_roundtrip()?;

    // Create a primary selection device for the seat
    let device = manager.get_device(&seat, &qh, ());

    // Create a data source and advertise text MIME type
    let source = manager.create_source(&qh, ());
    source.offer("text/plain;charset=utf-8".to_string());

    // Set selection on the device with a mock serial
    device.set_selection(Some(&source), 1);
    harness.sync_roundtrip()?;

    // Clear selection
    device.set_selection(None, 2);
    harness.sync_roundtrip()?;

    // Clean teardown
    source.destroy();
    device.destroy();
    harness.sync_roundtrip()?;

    Ok(())
}

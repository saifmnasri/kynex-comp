// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for presentation time protocol (`wp_presentation`).

use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;

use kx_shell_core::{ClientState, KynexState, post_presentation_feedback};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_output, wl_registry, wl_shm, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::wp::presentation_time::client::{wp_presentation, wp_presentation_feedback};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

#[derive(Debug, Clone, PartialEq, Eq)]
struct PresentedRecord {
    tv_sec: u64,
    tv_nsec: u32,
    refresh: u32,
    seq: u64,
    flags: wp_presentation_feedback::Kind,
}

#[derive(Default)]
struct PresentationTestClientState {
    presentation_global: Option<(u32, u32)>,
    compositor_global: Option<(u32, u32)>,
    shm_global: Option<(u32, u32)>,
    xdg_wm_base_global: Option<(u32, u32)>,
    output_global: Option<(u32, u32)>,
    clock_id: Option<u32>,
    presented: Vec<PresentedRecord>,
    discarded_count: u32,
    configured_surfaces: Vec<xdg_surface::XdgSurface>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for PresentationTestClientState {
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
            match interface.as_str() {
                "wp_presentation" => state.presentation_global = Some((name, version)),
                "wl_compositor" => state.compositor_global = Some((name, version)),
                "wl_shm" => state.shm_global = Some((name, version)),
                "xdg_wm_base" => state.xdg_wm_base_global = Some((name, version)),
                "wl_output" => state.output_global = Some((name, version)),
                _ => {}
            }
        }
    }
}

impl Dispatch<wp_presentation::WpPresentation, ()> for PresentationTestClientState {
    fn event(
        state: &mut Self,
        _proxy: &wp_presentation::WpPresentation,
        event: wp_presentation::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wp_presentation::Event::ClockId { clk_id } = event {
            state.clock_id = Some(clk_id);
        }
    }
}

impl Dispatch<wp_presentation_feedback::WpPresentationFeedback, ()>
    for PresentationTestClientState
{
    fn event(
        state: &mut Self,
        _proxy: &wp_presentation_feedback::WpPresentationFeedback,
        event: wp_presentation_feedback::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wp_presentation_feedback::Event::Presented {
                tv_sec_hi,
                tv_sec_lo,
                tv_nsec,
                refresh,
                seq_hi,
                seq_lo,
                flags,
            } => {
                let tv_sec = ((u64::from(tv_sec_hi)) << 32) | u64::from(tv_sec_lo);
                let seq = ((u64::from(seq_hi)) << 32) | u64::from(seq_lo);
                state.presented.push(PresentedRecord {
                    tv_sec,
                    tv_nsec,
                    refresh,
                    seq,
                    flags: flags
                        .into_result()
                        .unwrap_or(wp_presentation_feedback::Kind::empty()),
                });
            }
            wp_presentation_feedback::Event::Discarded => {
                state.discarded_count = state.discarded_count.saturating_add(1);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for PresentationTestClientState {
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

impl Dispatch<wl_surface::WlSurface, ()> for PresentationTestClientState {
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

impl Dispatch<wl_shm::WlShm, ()> for PresentationTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm::WlShm,
        _event: wl_shm::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_output::WlOutput, ()> for PresentationTestClientState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_output::WlOutput,
        _event: wl_output::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for PresentationTestClientState {
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

impl Dispatch<xdg_surface::XdgSurface, ()> for PresentationTestClientState {
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
            state.configured_surfaces.push(proxy.clone());
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for PresentationTestClientState {
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

struct TestHarness {
    display: Display<KynexState>,
    state: KynexState,
    client_conn: Connection,
    event_queue: EventQueue<PresentationTestClientState>,
    client_state: PresentationTestClientState,
}

impl TestHarness {
    fn new() -> Result<Self, Box<dyn Error>> {
        let display = Display::<KynexState>::new()?;
        let dh = display.handle();
        let state = KynexState::new(&dh, "wayland-test".to_string(), Size::from((1280, 800)))?;

        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_state_data = Arc::new(ClientState::default());

        display
            .handle()
            .insert_client(server_stream, client_state_data)?;

        let client_conn = Connection::from_socket(client_stream)?;
        let event_queue = client_conn.new_event_queue();
        let client_state = PresentationTestClientState::default();

        Ok(Self {
            display,
            state,
            client_conn,
            event_queue,
            client_state,
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
            self.event_queue.dispatch_pending(&mut self.client_state)?;
        }
        Ok(())
    }
}

#[test]
fn test_client_binds_presentation_global_and_receives_clock_id() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let qhandle = harness.event_queue.handle();

    let display_proxy = harness.client_conn.display();
    let registry = display_proxy.get_registry(&qhandle, ());

    harness.sync_roundtrip()?;

    let (name, version) = harness
        .client_state
        .presentation_global
        .ok_or("wp_presentation must be advertised")?;
    assert!(version >= 1, "wp_presentation version must be >= 1");

    let _presentation: wp_presentation::WpPresentation = registry.bind(name, 1, &qhandle, ());
    harness.sync_roundtrip()?;

    assert_eq!(
        harness.client_state.clock_id,
        Some(1),
        "Clock ID must be 1 (CLOCK_MONOTONIC)"
    );
    Ok(())
}

#[test]
fn test_presentation_feedback_presented_on_window() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let qhandle = harness.event_queue.handle();

    let display_proxy = harness.client_conn.display();
    let registry = display_proxy.get_registry(&qhandle, ());

    harness.sync_roundtrip()?;

    let (pres_name, _) = harness
        .client_state
        .presentation_global
        .ok_or("wp_presentation global")?;
    let (comp_name, _) = harness
        .client_state
        .compositor_global
        .ok_or("wl_compositor global")?;
    let (xdg_name, _) = harness
        .client_state
        .xdg_wm_base_global
        .ok_or("xdg_wm_base global")?;

    let presentation: wp_presentation::WpPresentation = registry.bind(pres_name, 1, &qhandle, ());
    let compositor: wl_compositor::WlCompositor = registry.bind(comp_name, 4, &qhandle, ());
    let xdg_wm_base: xdg_wm_base::XdgWmBase = registry.bind(xdg_name, 1, &qhandle, ());

    harness.sync_roundtrip()?;

    let surface = compositor.create_surface(&qhandle, ());
    let xdg_surface = xdg_wm_base.get_xdg_surface(&surface, &qhandle, ());
    let toplevel = xdg_surface.get_toplevel(&qhandle, ());
    toplevel.set_title("Presentation Test Window".to_string());
    surface.commit();

    harness.sync_roundtrip()?;

    // Ack configure and commit initial buffer state
    surface.commit();
    harness.sync_roundtrip()?;

    // Window is mapped in space
    assert_eq!(harness.state.space.elements().count(), 1);

    // Request presentation feedback on the surface
    let _feedback = presentation.feedback(&surface, &qhandle, ());
    surface.commit();

    harness.sync_roundtrip()?;

    // Trigger presentation feedback delivery on the output
    let timestamp = Duration::from_millis(250);
    post_presentation_feedback(&harness.state.space, &harness.state.output, timestamp, 42);

    harness.sync_roundtrip()?;

    assert_eq!(
        harness.client_state.presented.len(),
        1,
        "Must receive exactly one presented event"
    );
    let record = &harness.client_state.presented[0];
    assert_eq!(record.seq, 42);
    assert_eq!(record.tv_sec, 0);
    assert_eq!(record.tv_nsec, 250_000_000);
    assert!(record.flags.contains(wp_presentation_feedback::Kind::Vsync));
    assert_eq!(harness.client_state.discarded_count, 0);
    Ok(())
}

#[test]
fn test_presentation_feedback_discarded_when_superseded() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let qhandle = harness.event_queue.handle();

    let display_proxy = harness.client_conn.display();
    let registry = display_proxy.get_registry(&qhandle, ());

    harness.sync_roundtrip()?;

    let (pres_name, _) = harness
        .client_state
        .presentation_global
        .ok_or("wp_presentation global")?;
    let (comp_name, _) = harness
        .client_state
        .compositor_global
        .ok_or("wl_compositor global")?;

    let presentation: wp_presentation::WpPresentation = registry.bind(pres_name, 1, &qhandle, ());
    let compositor: wl_compositor::WlCompositor = registry.bind(comp_name, 4, &qhandle, ());

    harness.sync_roundtrip()?;

    let surface = compositor.create_surface(&qhandle, ());

    // Request first feedback and commit
    let _fb1 = presentation.feedback(&surface, &qhandle, ());
    surface.commit();

    harness.sync_roundtrip()?;

    // Request second feedback and commit before the first was presented.
    // The second commit supersedes the first, so the first feedback callback must be discarded.
    let _fb2 = presentation.feedback(&surface, &qhandle, ());
    surface.commit();

    harness.sync_roundtrip()?;

    assert_eq!(
        harness.client_state.discarded_count, 1,
        "Superseded presentation feedback must receive discarded event"
    );
    Ok(())
}

#[test]
fn test_presentation_feedback_multiple_frames() -> Result<(), Box<dyn Error>> {
    let mut harness = TestHarness::new()?;
    let qhandle = harness.event_queue.handle();

    let display_proxy = harness.client_conn.display();
    let registry = display_proxy.get_registry(&qhandle, ());

    harness.sync_roundtrip()?;

    let (pres_name, _) = harness
        .client_state
        .presentation_global
        .ok_or("wp_presentation global")?;
    let (comp_name, _) = harness
        .client_state
        .compositor_global
        .ok_or("wl_compositor global")?;
    let (xdg_name, _) = harness
        .client_state
        .xdg_wm_base_global
        .ok_or("xdg_wm_base global")?;

    let presentation: wp_presentation::WpPresentation = registry.bind(pres_name, 1, &qhandle, ());
    let compositor: wl_compositor::WlCompositor = registry.bind(comp_name, 4, &qhandle, ());
    let xdg_wm_base: xdg_wm_base::XdgWmBase = registry.bind(xdg_name, 1, &qhandle, ());

    harness.sync_roundtrip()?;

    let surface = compositor.create_surface(&qhandle, ());
    let xdg_surface = xdg_wm_base.get_xdg_surface(&surface, &qhandle, ());
    let toplevel = xdg_surface.get_toplevel(&qhandle, ());
    toplevel.set_title("Presentation Multi Frame".to_string());
    surface.commit();
    harness.sync_roundtrip()?;

    surface.commit();
    harness.sync_roundtrip()?;
    assert_eq!(harness.state.space.elements().count(), 1);

    let _fb1 = presentation.feedback(&surface, &qhandle, ());
    surface.commit();
    harness.sync_roundtrip()?;

    post_presentation_feedback(
        &harness.state.space,
        &harness.state.output,
        Duration::from_millis(16),
        1,
    );
    harness.sync_roundtrip()?;

    assert_eq!(harness.client_state.presented.len(), 1);
    assert_eq!(harness.client_state.presented[0].seq, 1);

    let _fb2 = presentation.feedback(&surface, &qhandle, ());
    surface.commit();
    harness.sync_roundtrip()?;

    post_presentation_feedback(
        &harness.state.space,
        &harness.state.output,
        Duration::from_millis(33),
        2,
    );
    harness.sync_roundtrip()?;

    assert_eq!(harness.client_state.presented.len(), 2);
    assert_eq!(harness.client_state.presented[1].seq, 2);
    Ok(())
}

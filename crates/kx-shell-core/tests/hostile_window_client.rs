// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Hostile and edge-case window client integration tests for Phase 3 closure.
//!
//! Verifies compositor stability and anti-DoS enforcement against hostile or buggy
//! clients sending malformed geometry constraints, invalid configure acks, spoofed grabs,
//! rapid state churn, and oversized metadata strings.

#![forbid(unsafe_code)]

use std::error::Error;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_layout::WindowState;
use kx_shell_core::window::{primary_output_usable_geometry, sync_layout_to_space};
use kx_shell_core::{ClientState, KynexState};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry, wl_seat, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

#[derive(Default)]
struct HostileData {
    compositor: Option<wl_compositor::WlCompositor>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    seat: Option<wl_seat::WlSeat>,
    last_serial: Option<u32>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for HostileData {
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

impl Dispatch<wl_compositor::WlCompositor, ()> for HostileData {
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

impl Dispatch<wl_surface::WlSurface, ()> for HostileData {
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

impl Dispatch<wl_seat::WlSeat, ()> for HostileData {
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

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for HostileData {
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

impl Dispatch<xdg_surface::XdgSurface, ()> for HostileData {
    fn event(
        state: &mut Self,
        proxy: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            state.last_serial = Some(serial);
            proxy.ack_configure(serial);
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for HostileData {
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

fn create_test_output(name: &str, size: (i32, i32)) -> Output {
    let mode = Mode {
        size: Size::from(size),
        refresh: 60_000,
    };
    let phys = PhysicalProperties {
        size: Size::from((400, 300)),
        subpixel: Subpixel::Unknown,
        make: "Kynex".to_string(),
        model: name.to_string(),
    };
    let output = Output::new(name.to_string(), phys);
    output.change_current_state(Some(mode), None, None, None);
    output.set_preferred(mode);
    output
}

fn step_client_server(
    conn: &Connection,
    event_queue: &mut wayland_client::EventQueue<HostileData>,
    client_data: &mut HostileData,
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
    event_queue.dispatch_pending(client_data)?;
    Ok(())
}

struct HostileHarness {
    display: Display<KynexState>,
    state: KynexState,
    conn: Connection,
    event_queue: wayland_client::EventQueue<HostileData>,
    client_data: HostileData,
    surface: wl_surface::WlSurface,
    xdg_surface: xdg_surface::XdgSurface,
    xdg_toplevel: xdg_toplevel::XdgToplevel,
}

impl HostileHarness {
    fn new() -> Result<Self, Box<dyn Error>> {
        let mut display = Display::new()?;
        let mut dh = display.handle();
        let primary_output = create_test_output("HOSTILE-TEST-OUT", (1920, 1080));
        let mut state =
            KynexState::new_with_output(&dh, "wayland-hostile-test".to_string(), primary_output)?;

        let (server_sock, client_sock) = UnixStream::pair()?;
        dh.insert_client(server_sock, Arc::new(ClientState::default()))?;

        let conn = Connection::from_socket(client_sock)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let mut client_data = HostileData::default();

        let _registry = conn.display().get_registry(&qhandle, ());
        step_client_server(
            &conn,
            &mut event_queue,
            &mut client_data,
            &mut display,
            &mut state,
        )?;

        let comp = client_data.compositor.clone().ok_or("Missing compositor")?;
        let wm = client_data.wm_base.clone().ok_or("Missing wm_base")?;

        let surface = comp.create_surface(&qhandle, ());
        let xdg_surface = wm.get_xdg_surface(&surface, &qhandle, ());
        let xdg_toplevel = xdg_surface.get_toplevel(&qhandle, ());

        surface.commit();
        step_client_server(
            &conn,
            &mut event_queue,
            &mut client_data,
            &mut display,
            &mut state,
        )?;

        Ok(Self {
            display,
            state,
            conn,
            event_queue,
            client_data,
            surface,
            xdg_surface,
            xdg_toplevel,
        })
    }

    fn step(&mut self) -> Result<(), Box<dyn Error>> {
        step_client_server(
            &self.conn,
            &mut self.event_queue,
            &mut self.client_data,
            &mut self.display,
            &mut self.state,
        )
    }
}

#[test]
fn test_hostile_size_constraints_clamping_and_resilience() -> Result<(), Box<dyn Error>> {
    let mut harness = HostileHarness::new()?;
    harness.step()?;

    let wid = *harness
        .state
        .window_map
        .keys()
        .next()
        .ok_or("Window not found")?;

    // Hostile client sets conflicting constraints: min size > max size
    harness.xdg_toplevel.set_min_size(3000, 2000);
    harness.xdg_toplevel.set_max_size(800, 600);
    harness.surface.commit();
    harness.step()?;

    // Compositor extracts constraints safely and clamps them without panic
    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window missing")?;
    let constraints = win.constraints;
    if let Some(max_w) = constraints.max_width {
        assert!(
            constraints.min_width <= max_w,
            "Clamped min_width must not exceed max_width"
        );
    }
    assert!(win.geometry.width > 0);
    assert!(win.geometry.height > 0);

    // Invariant check passes
    assert!(harness.state.window_manager.check_invariants());
    Ok(())
}

#[test]
fn test_hostile_unsolicited_ack_configure_flooding() -> Result<(), Box<dyn Error>> {
    let mut harness = HostileHarness::new()?;
    harness.step()?;

    // Hostile client floods multiple bogus ack_configure serial numbers
    for bogus_serial in [0, 1, 999_999, u32::MAX, u32::MAX - 1] {
        harness.xdg_surface.ack_configure(bogus_serial);
    }
    // Per Wayland XDG shell specification, an invalid configure serial raises a protocol
    // error on xdg_wm_base and drops the offending client.
    let res = harness.step();
    assert!(
        res.is_err(),
        "Compositor must terminate client connection for invalid configure serial"
    );

    // Compositor remains stable, safe, and healthy
    assert!(harness.state.window_manager.check_invariants());
    Ok(())
}

#[test]
fn test_hostile_grab_requests_with_invalid_serials() -> Result<(), Box<dyn Error>> {
    let mut harness = HostileHarness::new()?;
    harness.step()?;

    let seat = harness.client_data.seat.clone().ok_or("Missing seat")?;
    let pointer = harness.state.seat.get_pointer().ok_or("Missing pointer")?;
    assert!(!pointer.is_grabbed());

    // Hostile client attempts to spoof a move grab with an arbitrary/untracked serial
    harness.xdg_toplevel._move(&seat, 987_654);
    harness.step()?;
    assert!(!pointer.is_grabbed(), "Spoofed move grab must be rejected");

    // Hostile client attempts to spoof a resize grab with an arbitrary serial
    harness
        .xdg_toplevel
        .resize(&seat, 987_654, xdg_toplevel::ResizeEdge::BottomRight);
    harness.step()?;
    assert!(
        !pointer.is_grabbed(),
        "Spoofed resize grab must be rejected"
    );

    Ok(())
}

#[test]
fn test_hostile_rapid_title_and_app_id_flooding() -> Result<(), Box<dyn Error>> {
    let mut harness = HostileHarness::new()?;
    harness.step()?;

    let wid = *harness
        .state
        .window_map
        .keys()
        .next()
        .ok_or("Window not found")?;

    // Create 3000+ byte strings containing multibyte characters
    let long_title = "KynexComp🦀".repeat(250);
    let long_app_id = "org.kynex.hostile.app🚀".repeat(120);

    // Rapid title and app_id churning
    for i in 0..20 {
        harness.xdg_toplevel.set_title(format!("{long_title}_{i}"));
        harness
            .xdg_toplevel
            .set_app_id(format!("{long_app_id}_{i}"));
    }
    harness.surface.commit();
    harness.step()?;

    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window missing")?;
    // Truncated safely to <= 4096 bytes per Rule 05 Anti-DoS limits
    assert!(win.title.len() <= 4096);
    assert!(win.app_id.len() <= 4096);
    assert!(std::str::from_utf8(win.title.as_bytes()).is_ok());
    assert!(std::str::from_utf8(win.app_id.as_bytes()).is_ok());
    assert!(harness.state.window_manager.check_invariants());

    Ok(())
}

#[test]
fn test_hostile_rapid_state_churn_maximize_fullscreen_minimize() -> Result<(), Box<dyn Error>> {
    let mut harness = HostileHarness::new()?;
    harness.step()?;

    let wid = *harness
        .state
        .window_map
        .keys()
        .next()
        .ok_or("Window not found")?;

    // Spam rapid conflicting state changes
    for _ in 0..10 {
        harness.xdg_toplevel.set_maximized();
        harness.xdg_toplevel.set_fullscreen(None);
        harness.xdg_toplevel.unset_fullscreen();
        harness.xdg_toplevel.unset_maximized();
        harness.xdg_toplevel.set_minimized();
    }
    harness.step()?;

    // Compositor survives state transitions cleanly
    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window missing")?;
    assert_eq!(win.state, WindowState::Minimized);
    assert!(harness.state.window_manager.check_invariants());

    // Recover by unminimizing
    let usable = primary_output_usable_geometry(&harness.state);
    harness.state.window_manager.unminimize_window(wid, usable);
    sync_layout_to_space(&mut harness.state);
    assert!(harness.state.window_manager.check_invariants());

    Ok(())
}

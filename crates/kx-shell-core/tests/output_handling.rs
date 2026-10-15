// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for multi-output tracking, fullscreen targeting, and hotplug rehoming.

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_layout::WindowState;
use kx_layout::decoration::DecorationHitTest;
use kx_shell_core::decoration::{collect_ssd_render_elements, find_window_hit_test};
use kx_shell_core::window::{primary_output_for_window, sync_layout_to_space};
use kx_shell_core::{ClientState, KynexState};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::wayland_server::Display;
use smithay::utils::{Point, Size};
use wayland_client::protocol::{wl_compositor, wl_output, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

#[derive(Default)]
struct OutputClientData {
    compositor: Option<wl_compositor::WlCompositor>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    outputs: Vec<wl_output::WlOutput>,
    entered_outputs: Vec<wl_output::WlOutput>,
    left_outputs: Vec<wl_output::WlOutput>,
    configured: bool,
}

impl Dispatch<wl_registry::WlRegistry, ()> for OutputClientData {
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
            } else if interface == "wl_output" && version >= 2 {
                state.outputs.push(registry.bind(name, 2, qhandle, ()));
            }
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for OutputClientData {
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

impl Dispatch<wl_output::WlOutput, ()> for OutputClientData {
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

impl Dispatch<wl_surface::WlSurface, ()> for OutputClientData {
    fn event(
        state: &mut Self,
        _proxy: &wl_surface::WlSurface,
        event: wl_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_surface::Event::Enter { output } => {
                state.entered_outputs.push(output);
            }
            wl_surface::Event::Leave { output } => {
                state.left_outputs.push(output);
            }
            _ => {}
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for OutputClientData {
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

impl Dispatch<xdg_surface::XdgSurface, ()> for OutputClientData {
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

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for OutputClientData {
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
    event_queue: &mut wayland_client::EventQueue<OutputClientData>,
    client_data: &mut OutputClientData,
    display: &mut Display<KynexState>,
    state: &mut KynexState,
) -> Result<(), Box<dyn std::error::Error>> {
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

struct OutputTestHarness {
    display: Display<KynexState>,
    state: KynexState,
    secondary_output: Output,
    conn: Connection,
    event_queue: wayland_client::EventQueue<OutputClientData>,
    client_data: OutputClientData,
    #[expect(dead_code, reason = "kept alive for test scope")]
    surface: wl_surface::WlSurface,
    #[expect(dead_code, reason = "kept alive for test scope")]
    xdg_surface: xdg_surface::XdgSurface,
    xdg_toplevel: xdg_toplevel::XdgToplevel,
}

impl OutputTestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut display = Display::new()?;
        let mut dh = display.handle();
        let primary_output = create_test_output("TEST-PRIMARY", (1280, 720));
        let secondary_output = create_test_output("TEST-SECONDARY", (1920, 1080));

        let mut state =
            KynexState::new_with_output(&dh, "wayland-test-out".to_string(), primary_output)?;
        state.map_output(&dh, &secondary_output, (1280, 0));

        let (server_sock, client_sock) = UnixStream::pair()?;
        dh.insert_client(server_sock, Arc::new(ClientState::default()))?;

        let conn = Connection::from_socket(client_sock)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();
        let mut client_data = OutputClientData::default();

        let _registry = conn.display().get_registry(&qhandle, ());
        step_client_server(
            &conn,
            &mut event_queue,
            &mut client_data,
            &mut display,
            &mut state,
        )?;

        let compositor = client_data
            .compositor
            .clone()
            .ok_or("Missing wl_compositor")?;
        let wm_base = client_data.wm_base.clone().ok_or("Missing xdg_wm_base")?;

        let surface = compositor.create_surface(&qhandle, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qhandle, ());
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
            secondary_output,
            conn,
            event_queue,
            client_data,
            surface,
            xdg_surface,
            xdg_toplevel,
        })
    }

    fn step(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
fn test_primary_output_overlap_calculation() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = OutputTestHarness::new()?;
    harness.step()?;

    let wid = *harness
        .state
        .window_map
        .keys()
        .next()
        .ok_or("No mapped window")?;

    // Window located at (0, 0) - fully inside primary output (0..1280)
    let best_out = primary_output_for_window(&harness.state, wid);
    assert_eq!(best_out, Some(harness.state.output.clone()));

    // Move window so it mostly overlaps secondary output (1280..3200)
    if let Some(win) = harness.state.window_manager.window(wid) {
        let mut geom = win.geometry;
        geom.x = 1200; // overlapping 80px on primary, 720px on secondary (800px width)
        harness.state.window_manager.set_window_geometry(wid, geom);
        sync_layout_to_space(&mut harness.state);
    }

    let best_out = primary_output_for_window(&harness.state, wid);
    assert_eq!(best_out, Some(harness.secondary_output.clone()));

    Ok(())
}

#[test]
fn test_output_targeted_fullscreen_and_decoration_suppression()
-> Result<(), Box<dyn std::error::Error>> {
    let mut harness = OutputTestHarness::new()?;
    harness.step()?;

    let wid = *harness
        .state
        .window_map
        .keys()
        .next()
        .ok_or("No mapped window")?;

    // Request fullscreen targeted at secondary output
    let sec_wl_out = harness.client_data.outputs.get(1).cloned();
    harness.xdg_toplevel.set_fullscreen(sec_wl_out.as_ref());
    harness.step()?;

    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    assert_eq!(win.state, WindowState::Fullscreen);
    // Verified: geometry spans secondary output (origin 1280, 0, size 1920x1080)
    assert_eq!(win.geometry.x, 1280);
    assert_eq!(win.geometry.y, 0);
    assert_eq!(win.geometry.width, 1920);
    assert_eq!(win.geometry.height, 1080);

    // ADR-0029: SSD decorations are suppressed for fullscreen windows
    let ssd_elements = collect_ssd_render_elements(&harness.state);
    assert!(
        ssd_elements.is_empty(),
        "SSD decorations must be suppressed in fullscreen"
    );

    // Hit-testing inside fullscreen window returns Content (no resize borders)
    let hit = find_window_hit_test(&harness.state, (1300.0, 100.0).into());
    assert_eq!(hit, Some((wid, DecorationHitTest::Content)));

    // Unfullscreen restores normal window state and geometry
    harness.xdg_toplevel.unset_fullscreen();
    harness.step()?;

    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    assert_eq!(win.state, WindowState::Normal);

    Ok(())
}

#[test]
fn test_output_unplug_window_migration() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = OutputTestHarness::new()?;
    harness.step()?;

    let wid = *harness
        .state
        .window_map
        .keys()
        .next()
        .ok_or("No mapped window")?;

    // Position window on secondary output (1400, 100)
    if let Some(win) = harness.state.window_manager.window(wid) {
        let mut geom = win.geometry;
        geom.x = 1400;
        geom.y = 100;
        harness.state.window_manager.set_window_geometry(wid, geom);
        sync_layout_to_space(&mut harness.state);
    }

    // Unmap secondary output and migrate surviving windows
    let sec_output = harness.secondary_output.clone();
    harness.state.unmap_output(&sec_output);
    harness.state.migrate_windows_from_unplugged(
        Point::from((1280, 0)),
        Size::from((1920, 1080)),
        Point::from((0, 0)),
        Size::from((1280, 720)),
    );

    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    // Window must be rehomed to primary output bounds (0 <= x < 1280)
    assert!(
        win.geometry.x >= 0 && win.geometry.x < 1280,
        "Migrated window must be placed within primary output bounds: x={}",
        win.geometry.x
    );

    // Verify Space location is synchronized with WindowManager (ADR-0030)
    let window = harness
        .state
        .window_map
        .get(&wid)
        .ok_or("Window element missing")?;
    let space_loc = harness
        .state
        .space
        .element_location(window)
        .ok_or("Window unmapped in space")?;
    assert_eq!(space_loc.x, win.geometry.x);
    assert_eq!(space_loc.y, win.geometry.y);

    Ok(())
}

#[test]
fn test_output_resize_resizes_maximized_window() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = OutputTestHarness::new()?;
    harness.step()?;

    let wid = *harness
        .state
        .window_map
        .keys()
        .next()
        .ok_or("No mapped window")?;

    // Request maximize
    harness.xdg_toplevel.set_maximized();
    harness.step()?;

    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    assert_eq!(win.state, WindowState::Maximized);
    assert_eq!(win.geometry.width, 1280);
    assert_eq!(win.geometry.height, 720);

    // Update output size (e.g. user resizes nested winit window to 1600x900)
    harness.state.update_output_size((1600, 900).into());
    harness.step()?;

    let win = harness
        .state
        .window_manager
        .window(wid)
        .ok_or("Window not found")?;
    assert_eq!(win.state, WindowState::Maximized);
    assert_eq!(win.geometry.width, 1600);
    assert_eq!(win.geometry.height, 900);

    let window_elem = harness
        .state
        .window_map
        .get(&wid)
        .ok_or("Window element missing")?;
    let loc = harness
        .state
        .space
        .element_location(window_elem)
        .ok_or("Window not in space")?;
    assert_eq!(loc.x, win.geometry.x);
    assert_eq!(loc.y, win.geometry.y);

    Ok(())
}

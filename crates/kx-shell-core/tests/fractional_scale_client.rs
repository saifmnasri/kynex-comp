// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for fractional scale protocol (`wp_fractional_scale_v1`).

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientState, KynexState};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

#[derive(Default)]
struct FractionalScaleTestClientState {
    manager_global: Option<(u32, u32)>,
    compositor_global: Option<(u32, u32)>,
    wm_base_global: Option<(u32, u32)>,
    preferred_scales: Vec<u32>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for FractionalScaleTestClientState {
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
            if interface == "wp_fractional_scale_manager_v1" {
                state.manager_global = Some((name, version));
            } else if interface == "wl_compositor" {
                state.compositor_global = Some((name, version));
            } else if interface == "xdg_wm_base" {
                state.wm_base_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1, ()>
    for FractionalScaleTestClientState
{
    fn event(
        _state: &mut Self,
        _proxy: &wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        _event: wp_fractional_scale_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wp_fractional_scale_v1::WpFractionalScaleV1, ()> for FractionalScaleTestClientState {
    fn event(
        state: &mut Self,
        _proxy: &wp_fractional_scale_v1::WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            state.preferred_scales.push(scale);
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for FractionalScaleTestClientState {
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

impl Dispatch<wl_surface::WlSurface, ()> for FractionalScaleTestClientState {
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

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for FractionalScaleTestClientState {
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

impl Dispatch<xdg_surface::XdgSurface, ()> for FractionalScaleTestClientState {
    fn event(
        _state: &mut Self,
        proxy: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            proxy.ack_configure(serial);
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for FractionalScaleTestClientState {
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

struct FractionalScaleTestHarness {
    display: Display<KynexState>,
    state: KynexState,
    conn: Connection,
    event_queue: EventQueue<FractionalScaleTestClientState>,
    client_state: FractionalScaleTestClientState,
}

impl FractionalScaleTestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut display: Display<KynexState> = Display::new()?;
        let initial_size = Size::from((1280, 720));
        let mut state = KynexState::new(
            &display.handle(),
            "fractional-scale-test-sock".to_string(),
            initial_size,
        )?;

        let (server_stream, client_stream) = UnixStream::pair()?;
        let client_data = Arc::new(ClientState::default());
        display.handle().insert_client(server_stream, client_data)?;

        let conn = Connection::from_socket(client_stream)?;
        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();

        let display_proxy = conn.display();
        let _registry = display_proxy.get_registry(&qhandle, ());
        conn.flush()?;

        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;

        let read_guard = conn.prepare_read().ok_or("Failed to prepare client read")?;
        read_guard.read()?;

        let mut client_state = FractionalScaleTestClientState::default();
        event_queue.dispatch_pending(&mut client_state)?;

        Ok(Self {
            display,
            state,
            conn,
            event_queue,
            client_state,
        })
    }

    fn roundtrip(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
fn test_fractional_scale_global_advertised_and_bindable() -> Result<(), Box<dyn std::error::Error>>
{
    let mut harness = FractionalScaleTestHarness::new()?;

    let (manager_name, manager_version) = harness
        .client_state
        .manager_global
        .ok_or("wp_fractional_scale_manager_v1 global must be advertised")?;

    assert_eq!(
        manager_version, 1,
        "wp_fractional_scale_manager_v1 version must be 1"
    );

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let _manager: wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1 =
        registry.bind(manager_name, 1, &qhandle, ());

    harness.roundtrip()?;
    Ok(())
}

#[test]
fn test_fractional_scale_preferred_scale_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = FractionalScaleTestHarness::new()?;

    let (manager_name, _) = harness
        .client_state
        .manager_global
        .ok_or("wp_fractional_scale_manager_v1 global must be advertised")?;
    let (comp_name, comp_version) = harness
        .client_state
        .compositor_global
        .ok_or("wl_compositor global must be advertised")?;

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let compositor: wl_compositor::WlCompositor =
        registry.bind(comp_name, comp_version.min(5), &qhandle, ());
    let manager: wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1 =
        registry.bind(manager_name, 1, &qhandle, ());

    // 1. Create surface under standard 1.0x output scale (scale = 120 in 120ths)
    let surface1 = compositor.create_surface(&qhandle, ());
    let fs1 = manager.get_fractional_scale(&surface1, &qhandle, ());
    surface1.commit();

    harness.roundtrip()?;

    assert_eq!(
        harness.client_state.preferred_scales.as_slice(),
        &[120],
        "Preferred scale for default 1.0x output must be 120 (120/120)"
    );

    // 2. Change output scale to 1.5x (1.5 * 120 = 180 in 120ths)
    harness
        .state
        .output
        .change_current_state(None, None, Some(Scale::Fractional(1.5)), None);

    let surface2 = compositor.create_surface(&qhandle, ());
    let fs2 = manager.get_fractional_scale(&surface2, &qhandle, ());
    surface2.commit();

    harness.roundtrip()?;

    assert_eq!(
        harness.client_state.preferred_scales.as_slice(),
        &[120, 180],
        "Preferred scale for 1.5x output must be 180 (180/120)"
    );

    // 3. Clean destruction
    fs1.destroy();
    fs2.destroy();
    surface1.destroy();
    surface2.destroy();
    manager.destroy();

    harness.roundtrip()?;
    Ok(())
}

#[test]
fn test_fractional_scale_duplicate_surface_rejection() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = FractionalScaleTestHarness::new()?;

    let (manager_name, _) = harness
        .client_state
        .manager_global
        .ok_or("wp_fractional_scale_manager_v1 global must be advertised")?;
    let (comp_name, comp_version) = harness
        .client_state
        .compositor_global
        .ok_or("wl_compositor global must be advertised")?;

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let compositor: wl_compositor::WlCompositor =
        registry.bind(comp_name, comp_version.min(5), &qhandle, ());
    let manager: wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1 =
        registry.bind(manager_name, 1, &qhandle, ());
    let surface = compositor.create_surface(&qhandle, ());

    // First get_fractional_scale succeeds
    let _fs1 = manager.get_fractional_scale(&surface, &qhandle, ());

    // Hostile client: attempt duplicate get_fractional_scale on the same surface
    let _fs2 = manager.get_fractional_scale(&surface, &qhandle, ());

    harness.conn.flush()?;

    // Server must post FractionalScaleExists (code 0) protocol error
    let _ = harness.display.dispatch_clients(&mut harness.state);
    let _ = harness.display.flush_clients();

    // Client connection should encounter protocol error and disconnect
    let read_err = match harness.conn.prepare_read() {
        Some(guard) => guard.read().is_err(),
        None => true,
    };
    let mut dummy = FractionalScaleTestClientState::default();
    let dispatch_result = harness.event_queue.dispatch_pending(&mut dummy);

    let had_error = read_err || dispatch_result.is_err();
    assert!(
        had_error,
        "Duplicate get_fractional_scale must cause protocol error and disconnect client"
    );

    Ok(())
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

#[test]
fn test_fractional_scale_output_migration_and_scale_update()
-> Result<(), Box<dyn std::error::Error>> {
    let mut harness = FractionalScaleTestHarness::new()?;
    let (wm_name, _) = harness
        .client_state
        .wm_base_global
        .ok_or("no xdg_wm_base")?;
    let (comp_name, comp_v) = harness
        .client_state
        .compositor_global
        .ok_or("no wl_compositor")?;
    let (mgr_name, _) = harness
        .client_state
        .manager_global
        .ok_or("no fractional_scale")?;

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());
    let compositor: wl_compositor::WlCompositor =
        registry.bind(comp_name, comp_v.min(5), &qhandle, ());
    let wm_base: xdg_wm_base::XdgWmBase = registry.bind(wm_name, 1, &qhandle, ());
    let manager: wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1 =
        registry.bind(mgr_name, 1, &qhandle, ());

    let dh = harness.display.handle();
    let sec_out = create_test_output("TEST-SEC", (1920, 1080));
    sec_out.change_current_state(None, None, Some(Scale::Fractional(2.0)), None);
    harness.state.map_output(&dh, &sec_out, (1280, 0));

    let surface = compositor.create_surface(&qhandle, ());
    let fs = manager.get_fractional_scale(&surface, &qhandle, ());
    let xdg_surface = wm_base.get_xdg_surface(&surface, &qhandle, ());
    let xdg_toplevel = xdg_surface.get_toplevel(&qhandle, ());

    surface.commit();
    harness.roundtrip()?;
    harness.roundtrip()?;
    assert_eq!(harness.client_state.preferred_scales.as_slice(), &[120]);

    let wid = *harness.state.window_map.keys().next().ok_or("no window")?;
    if let Some(win) = harness.state.window_manager.window(wid) {
        let mut geom = win.geometry;
        geom.x = 1400;
        harness.state.window_manager.set_window_geometry(wid, geom);
        kx_shell_core::window::sync_layout_to_space(&mut harness.state);
    }
    harness.roundtrip()?;
    assert_eq!(
        harness.client_state.preferred_scales.as_slice(),
        &[120, 240]
    );

    harness
        .state
        .update_output_scale(&sec_out, Scale::Fractional(1.75));
    harness.roundtrip()?;
    assert_eq!(
        harness.client_state.preferred_scales.as_slice(),
        &[120, 240, 210]
    );

    fs.destroy();
    xdg_toplevel.destroy();
    xdg_surface.destroy();
    surface.destroy();
    manager.destroy();
    wm_base.destroy();
    harness.roundtrip()?;
    Ok(())
}

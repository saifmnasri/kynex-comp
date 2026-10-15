// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Integration tests for linux-drm-syncobj explicit synchronization protocol
//! (`wp_linux_drm_syncobj_manager_v1`).

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use kx_shell_core::{ClientState, KynexState};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Size;
use wayland_client::protocol::{wl_compositor, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::wp::linux_drm_syncobj::v1::client::{
    wp_linux_drm_syncobj_manager_v1, wp_linux_drm_syncobj_surface_v1,
    wp_linux_drm_syncobj_timeline_v1,
};

#[derive(Default)]
struct SyncobjTestClientState {
    syncobj_global: Option<(u32, u32)>,
    compositor_global: Option<(u32, u32)>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for SyncobjTestClientState {
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
            if interface == "wp_linux_drm_syncobj_manager_v1" {
                state.syncobj_global = Some((name, version));
            } else if interface == "wl_compositor" {
                state.compositor_global = Some((name, version));
            }
        }
    }
}

impl Dispatch<wp_linux_drm_syncobj_manager_v1::WpLinuxDrmSyncobjManagerV1, ()>
    for SyncobjTestClientState
{
    fn event(
        _state: &mut Self,
        _proxy: &wp_linux_drm_syncobj_manager_v1::WpLinuxDrmSyncobjManagerV1,
        _event: wp_linux_drm_syncobj_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wp_linux_drm_syncobj_surface_v1::WpLinuxDrmSyncobjSurfaceV1, ()>
    for SyncobjTestClientState
{
    fn event(
        _state: &mut Self,
        _proxy: &wp_linux_drm_syncobj_surface_v1::WpLinuxDrmSyncobjSurfaceV1,
        _event: wp_linux_drm_syncobj_surface_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wp_linux_drm_syncobj_timeline_v1::WpLinuxDrmSyncobjTimelineV1, ()>
    for SyncobjTestClientState
{
    fn event(
        _state: &mut Self,
        _proxy: &wp_linux_drm_syncobj_timeline_v1::WpLinuxDrmSyncobjTimelineV1,
        _event: wp_linux_drm_syncobj_timeline_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for SyncobjTestClientState {
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

impl Dispatch<wl_surface::WlSurface, ()> for SyncobjTestClientState {
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

struct SyncobjTestHarness {
    display: Display<KynexState>,
    state: KynexState,
    conn: Connection,
    event_queue: wayland_client::EventQueue<SyncobjTestClientState>,
    client_state: SyncobjTestClientState,
}

impl SyncobjTestHarness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let mut display: Display<KynexState> = Display::new()?;
        let initial_size = Size::from((1280, 720));
        let mut state = KynexState::new(
            &display.handle(),
            "syncobj-test-sock".to_string(),
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

        let mut client_state = SyncobjTestClientState::default();
        event_queue.dispatch_pending(&mut client_state)?;

        Ok(Self {
            display,
            state,
            conn,
            event_queue,
            client_state,
        })
    }
}

#[test]
fn test_drm_syncobj_global_advertised_and_bindable() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = SyncobjTestHarness::new()?;

    let Some((syncobj_name, syncobj_version)) = harness.client_state.syncobj_global else {
        tracing::warn!(
            "wp_linux_drm_syncobj_manager_v1 not advertised in this environment (no DRM render node)"
        );
        return Ok(());
    };

    assert_eq!(
        syncobj_version, 1,
        "wp_linux_drm_syncobj_manager_v1 version must be 1"
    );

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let _manager: wp_linux_drm_syncobj_manager_v1::WpLinuxDrmSyncobjManagerV1 =
        registry.bind(syncobj_name, 1, &qhandle, ());

    harness.conn.flush()?;
    harness.display.dispatch_clients(&mut harness.state)?;
    harness.display.flush_clients()?;

    let read_guard = harness
        .conn
        .prepare_read()
        .ok_or("Failed to prepare client read")?;
    read_guard.read()?;
    harness
        .event_queue
        .dispatch_pending(&mut SyncobjTestClientState::default())?;

    Ok(())
}

#[test]
fn test_drm_syncobj_duplicate_surface_rejection() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = SyncobjTestHarness::new()?;

    let Some((syncobj_name, _)) = harness.client_state.syncobj_global else {
        tracing::warn!("wp_linux_drm_syncobj_manager_v1 not advertised; skipping duplicate test");
        return Ok(());
    };
    let Some((comp_name, comp_version)) = harness.client_state.compositor_global else {
        return Err("wl_compositor global must be advertised".into());
    };

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let compositor: wl_compositor::WlCompositor =
        registry.bind(comp_name, comp_version.min(5), &qhandle, ());
    let manager: wp_linux_drm_syncobj_manager_v1::WpLinuxDrmSyncobjManagerV1 =
        registry.bind(syncobj_name, 1, &qhandle, ());
    let surface = compositor.create_surface(&qhandle, ());

    // First get_surface succeeds
    let _sync_surface1 = manager.get_surface(&surface, &qhandle, ());

    // Hostile client: call get_surface on the same wl_surface again
    let _sync_surface2 = manager.get_surface(&surface, &qhandle, ());

    harness.conn.flush()?;

    // Server must reject the duplicate surface request with protocol error (surface_exists = 1)
    let _ = harness.display.dispatch_clients(&mut harness.state);
    let _ = harness.display.flush_clients();

    // Client connection should encounter protocol error
    let read_err = match harness.conn.prepare_read() {
        Some(guard) => guard.read().is_err(),
        None => true,
    };
    let mut dummy = SyncobjTestClientState::default();
    let dispatch_result = harness.event_queue.dispatch_pending(&mut dummy);

    let had_error = read_err || dispatch_result.is_err();
    assert!(
        had_error,
        "Duplicate get_surface must cause protocol error and disconnect client"
    );

    Ok(())
}

#[test]
fn test_drm_syncobj_surface_destroy_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let mut harness = SyncobjTestHarness::new()?;

    let Some((syncobj_name, _)) = harness.client_state.syncobj_global else {
        tracing::warn!("wp_linux_drm_syncobj_manager_v1 not advertised; skipping lifecycle test");
        return Ok(());
    };
    let Some((comp_name, comp_version)) = harness.client_state.compositor_global else {
        return Err("wl_compositor global must be advertised".into());
    };

    let qhandle = harness.event_queue.handle();
    let registry = harness.conn.display().get_registry(&qhandle, ());

    let compositor: wl_compositor::WlCompositor =
        registry.bind(comp_name, comp_version.min(5), &qhandle, ());
    let manager: wp_linux_drm_syncobj_manager_v1::WpLinuxDrmSyncobjManagerV1 =
        registry.bind(syncobj_name, 1, &qhandle, ());
    let surface = compositor.create_surface(&qhandle, ());

    // Create syncobj surface
    let sync_surface = manager.get_surface(&surface, &qhandle, ());

    harness.conn.flush()?;
    harness.display.dispatch_clients(&mut harness.state)?;
    harness.display.flush_clients()?;

    // Destroy syncobj surface cleanly
    sync_surface.destroy();
    surface.destroy();

    harness.conn.flush()?;
    harness.display.dispatch_clients(&mut harness.state)?;
    harness.display.flush_clients()?;

    let read_guard = harness
        .conn
        .prepare_read()
        .ok_or("Failed to prepare client read")?;
    read_guard.read()?;
    harness
        .event_queue
        .dispatch_pending(&mut SyncobjTestClientState::default())?;

    Ok(())
}

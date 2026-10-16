// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Tearing control protocol handler (`wp_tearing_control_v1`).
//!
//! Allows clients to hint whether a surface's content is suitable for
//! asynchronous presentation (tearing) to minimize latency.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use smithay::reexports::wayland_protocols::wp::tearing_control::v1::server::wp_tearing_control_manager_v1::{
    self, WpTearingControlManagerV1,
};
use smithay::reexports::wayland_protocols::wp::tearing_control::v1::server::wp_tearing_control_v1::{
    self, WpTearingControlV1,
};
use smithay::reexports::wayland_server::backend::{ClientId, GlobalId};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, WEnum, Weak,
};
use smithay::wayland::compositor::{self, Cacheable};
use tracing::info;

use crate::state::KynexState;

/// Double-buffered presentation hint cached on the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TearingControlSurfaceCachedState {
    presentation_hint: wp_tearing_control_v1::PresentationHint,
}

impl TearingControlSurfaceCachedState {
    /// Returns the active presentation hint.
    #[must_use]
    pub fn presentation_hint(&self) -> wp_tearing_control_v1::PresentationHint {
        self.presentation_hint
    }
}

impl Default for TearingControlSurfaceCachedState {
    fn default() -> Self {
        Self {
            presentation_hint: wp_tearing_control_v1::PresentationHint::Vsync,
        }
    }
}

impl Cacheable for TearingControlSurfaceCachedState {
    fn commit(&mut self, _dh: &DisplayHandle) -> Self {
        *self
    }

    fn merge_into(self, into: &mut Self, _dh: &DisplayHandle) {
        *into = self;
    }
}

#[derive(Debug, Default)]
struct TearingControlSurfaceData {
    is_resource_attached: AtomicBool,
}

impl TearingControlSurfaceData {
    fn set_attached(&self, attached: bool) {
        self.is_resource_attached.store(attached, Ordering::Release);
    }

    fn is_attached(&self) -> bool {
        self.is_resource_attached.load(Ordering::Acquire)
    }
}

/// User data attached to a `WpTearingControlV1` Wayland resource.
#[derive(Debug)]
pub struct TearingControlUserData {
    surface: Mutex<Weak<WlSurface>>,
}

impl TearingControlUserData {
    fn new(surface: &WlSurface) -> Self {
        Self {
            surface: Mutex::new(surface.downgrade()),
        }
    }

    fn wl_surface(&self) -> Option<WlSurface> {
        match self.surface.lock() {
            Ok(guard) => guard.upgrade().ok(),
            Err(_) => None,
        }
    }
}

/// State for the `wp_tearing_control_manager_v1` global.
#[derive(Debug)]
pub struct TearingControlState {
    global: GlobalId,
}

impl TearingControlState {
    /// Initializes the `wp_tearing_control_manager_v1` global.
    pub fn new<D>(dh: &DisplayHandle) -> Self
    where
        D: GlobalDispatch<WpTearingControlManagerV1, ()>
            + Dispatch<WpTearingControlManagerV1, ()>
            + Dispatch<WpTearingControlV1, TearingControlUserData>
            + 'static,
    {
        info!("Initializing wp_tearing_control_manager_v1 global (version 1)");
        let global = dh.create_global::<D, WpTearingControlManagerV1, _>(1, ());
        Self { global }
    }

    /// Returns the global ID.
    #[must_use]
    pub fn global(&self) -> &GlobalId {
        &self.global
    }
}

/// Initializes the tearing control manager global.
#[must_use]
pub fn init_tearing_control_global(dh: &DisplayHandle) -> TearingControlState {
    TearingControlState::new::<KynexState>(dh)
}

impl GlobalDispatch<WpTearingControlManagerV1, ()> for KynexState {
    fn bind(
        _state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<WpTearingControlManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<WpTearingControlManagerV1, ()> for KynexState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        resource: &WpTearingControlManagerV1,
        request: wp_tearing_control_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wp_tearing_control_manager_v1::Request::GetTearingControl { id, surface } = request {
            let already_attached = compositor::with_states(&surface, |states| {
                states
                    .data_map
                    .insert_if_missing(TearingControlSurfaceData::default);
                if let Some(data) = states.data_map.get::<TearingControlSurfaceData>() {
                    if data.is_attached() {
                        return true;
                    }
                    data.set_attached(true);
                }
                false
            });

            if already_attached {
                resource.post_error(
                    wp_tearing_control_manager_v1::Error::TearingControlExists,
                    "surface already has a tearing object associated",
                );
                return;
            }

            data_init.init(id, TearingControlUserData::new(&surface));
        }
    }
}

impl Dispatch<WpTearingControlV1, TearingControlUserData> for KynexState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &WpTearingControlV1,
        request: wp_tearing_control_v1::Request,
        data: &TearingControlUserData,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wp_tearing_control_v1::Request::SetPresentationHint { hint } => {
                let presentation_hint = match hint {
                    WEnum::Value(h) => h,
                    WEnum::Unknown(_) => return,
                };

                if let Some(surface) = data.wl_surface() {
                    compositor::with_states(&surface, |states| {
                        states
                            .cached_state
                            .get::<TearingControlSurfaceCachedState>()
                            .pending()
                            .presentation_hint = presentation_hint;
                    });
                }
            }
            wp_tearing_control_v1::Request::Destroy => {
                if let Some(surface) = data.wl_surface() {
                    compositor::with_states(&surface, |states| {
                        if let Some(surface_data) =
                            states.data_map.get::<TearingControlSurfaceData>()
                        {
                            surface_data.set_attached(false);
                        }
                        states
                            .cached_state
                            .get::<TearingControlSurfaceCachedState>()
                            .pending()
                            .presentation_hint = wp_tearing_control_v1::PresentationHint::Vsync;
                    });
                }
            }
            _ => {}
        }
    }

    fn destroyed(
        _state: &mut Self,
        _client: ClientId,
        _object: &WpTearingControlV1,
        data: &TearingControlUserData,
    ) {
        if let Some(surface) = data.wl_surface() {
            compositor::with_states(&surface, |states| {
                if let Some(surface_data) = states.data_map.get::<TearingControlSurfaceData>() {
                    surface_data.set_attached(false);
                }
            });
        }
    }
}

// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Shell core for `KynexComp`.
//!
//! Provides the primary compositor state, output tracking, window management,
//! rendering loop, and protocol dispatch.

pub mod action;
pub mod activation;
pub mod client;
pub mod data_device;
pub mod decoration;
pub mod dmabuf;
pub mod drm;
pub mod error;
pub mod frame_clock;
pub mod grab;
pub mod layer_shell;
pub mod nested;
pub mod state;
pub mod syncobj;
pub mod viewporter;
pub mod window;

#[cfg(feature = "fault-injection")]
pub mod fault;

pub use action::{activate_and_focus_window, clear_window_focus, execute_compositor_action};
pub use client::{ClientLimits, ClientState};
pub use dmabuf::{MAX_DMABUF_DIMENSION, default_dmabuf_formats, init_dmabuf_global};
pub use drm::{
    ConnectorInfo, DrmDeviceInfo, DrmDiagnosticReport, DrmOptions, DrmSession, ModeInfo, run_drm,
};
pub use error::ShellError;
#[cfg(feature = "fault-injection")]
pub use fault::{FaultController, FaultError, FaultSpec, MAX_HANG_SECS, MAX_TRIGGER_FRAME};
pub use frame_clock::{FrameClock, FrameClockError, FrameState, QueueResult};
pub use grab::{
    BTN_LEFT, BTN_RIGHT, MoveSurfaceGrab, ResizeSurfaceGrab, detect_resize_edge,
    handle_client_move_request, handle_client_resize_request, is_primary_modifier_active,
    modifiers_to_key_modifiers, start_compositor_move_grab, start_compositor_resize_grab,
    try_start_compositor_grab,
};
pub use nested::{DEFAULT_BG_COLOR, NestedConfig, NestedOptions, run_nested};
pub use state::KynexState;
pub use syncobj::init_drm_syncobj_global;
pub use viewporter::init_viewporter_global;

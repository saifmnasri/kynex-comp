// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Shell core for `KynexComp`.
//!
//! Provides the primary compositor state, output tracking, window management,
//! rendering loop, and protocol dispatch.

pub mod action;
pub mod activation;
pub mod client;
pub mod content_type;
pub mod cursor_shape;
pub mod data_device;
pub mod decoration;
pub mod dmabuf;
pub mod drm;
pub mod error;
pub mod foreign_toplevel;
pub mod fractional_scale;
pub mod frame_clock;
pub mod grab;
pub mod idle_inhibit;
pub mod input_method;
pub mod layer_shell;
pub mod nested;
pub mod output;
pub mod pointer_constraints;
pub mod presentation;
pub mod primary_selection;
pub mod relative_pointer;
pub mod session_lock;
pub mod single_pixel_buffer;
pub mod state;
pub mod syncobj;
pub mod tearing_control;
pub mod text_input;
pub mod viewporter;
pub mod window;
pub mod xdg_shell;

#[cfg(feature = "fault-injection")]
pub mod fault;

pub use action::{activate_and_focus_window, clear_window_focus, execute_compositor_action};
pub use client::{ClientLimits, ClientState};
pub use content_type::init_content_type_global;
pub use cursor_shape::init_cursor_shape_global;
pub use dmabuf::{MAX_DMABUF_DIMENSION, default_dmabuf_formats, init_dmabuf_global};
pub use drm::{
    ConnectorInfo, DrmDeviceInfo, DrmDiagnosticReport, DrmOptions, DrmSession, ModeInfo, run_drm,
};
pub use error::ShellError;
#[cfg(feature = "fault-injection")]
pub use fault::{FaultController, FaultError, FaultSpec, MAX_HANG_SECS, MAX_TRIGGER_FRAME};
pub use foreign_toplevel::init_foreign_toplevel_list_global;
pub use fractional_scale::{init_fractional_scale_global, update_surface_fractional_scale};
pub use frame_clock::{FrameClock, FrameClockError, FrameState, QueueResult};
pub use grab::{
    BTN_LEFT, BTN_RIGHT, MoveSurfaceGrab, ResizeSurfaceGrab, detect_resize_edge,
    handle_client_move_request, handle_client_resize_request, is_primary_modifier_active,
    modifiers_to_key_modifiers, start_compositor_move_grab, start_compositor_resize_grab,
    try_start_compositor_grab,
};
pub use idle_inhibit::init_idle_inhibit_global;
pub use input_method::init_input_method_global;
pub use nested::{DEFAULT_BG_COLOR, NestedConfig, NestedOptions, run_nested};
pub use pointer_constraints::{init_pointer_constraints_global, maybe_activate_pointer_constraint};
pub use presentation::{init_presentation_global, post_presentation_feedback};
pub use primary_selection::init_primary_selection_global;
pub use relative_pointer::init_relative_pointer_global;
pub use session_lock::init_session_lock_global;
pub use single_pixel_buffer::init_single_pixel_buffer_global;
pub use state::KynexState;
pub use syncobj::init_drm_syncobj_global;
pub use tearing_control::init_tearing_control_global;
pub use text_input::init_text_input_global;
pub use viewporter::init_viewporter_global;

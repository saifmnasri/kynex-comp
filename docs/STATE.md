# Project State (handoff file)

> Update at the end of every task. A fresh chat session must be able to continue from this file alone.

**Current phase:** 5 — XWayland (Phase 4 — Real-App Protocol Completeness — CLOSED & AUDIT REMEDIATED)
**Last updated:** 2026-10-22 — Independent audit findings F1–F7 from `docs/reviews/phase-4-independent-review.md` fully remediated on branch `fix/phase-4-audit-remediation`. Verification gate 100% green.

## Done
- **Phase 4 Independent Audit Remediation — CLOSED:**
  - **F1 (SEC-02 / SEC-07: Independent Privilege Gating & Quotas):** Decoupled `[privilege]` allow-lists into separate config vectors (`layer_shell_allow`, `session_lock_allow`, `input_method_allow`, `foreign_toplevel_allow`) in `kx-config` and `kx-shell-core`. Added per-client quota bounding across all protocol objects (`max_locked_pointers`, `max_confined_pointers`, `max_text_inputs`, `max_input_methods`, `max_idle_inhibitors`, etc.). Added comprehensive integration tests.
  - **F2 (SEC-01: Session Lock Fail-Closed & Rendering/Input Gate):** Enforced fail-closed session lock state machine in `crates/kx-shell-core/src/session_lock.rs`. All window/layer surface rendering and non-lock input events are suppressed while locked. If lock client disconnects without unlocking, session remains persistently locked with black screen fallback.
  - **F3 (BUG-03: Idle Notifier Event Loop & Input Activity Hook):** Created dedicated Calloop event loop for idle notifier timers in `crates/kx-shell-core/src/idle_notify.rs`, hooked input activity notifications on all pointer motion, button, key, and touch events to reset idle timeouts.
  - **F4 (BUG-04: DRM Presentation Feedback):** Wired real DRM page-flip vblank presentation feedback in `crates/kx-shell-core/src/drm/mod.rs` and `drm/output.rs`, invoking `post_presentation_feedback` with real monotonic hardware timestamps and refresh sequence counters.
  - **F5 (BUG-05: Pointer Constraints Activation on Focus):** Hooked pointer focus changes in `crates/kx-shell-core/src/pointer_constraints.rs` to automatically activate locked/confined pointer constraints when constrained surface acquires focus and deactivate on unfocus.
  - **F6 (BUG-06: Fractional Scale on Output Migration):** Updated `crates/kx-shell-core/src/fractional_scale.rs`, `output.rs`, and `window.rs` (`sync_layout_to_space`) to advertise updated preferred scales to surfaces when a window migrates across outputs with differing scale factors or dynamic display scale changes. Verified via `test_fractional_scale_output_migration_and_scale_update`.
  - **F7 (ARCH-09 / DOC-11 / SPEC-08: Split Oversized Functions, Record ADR-0034, Docs Integrity):**
    - Split `semantic_validation` test in `crates/kx-config/src/load.rs` into three focused tests $\le 35$ lines.
    - Split `KynexState::new_with_output` in `crates/kx-shell-core/src/state.rs` (using `CoreState` and `build_state`) to $\le 50$ lines.
    - Extracted `create_render_elements` in `crates/kx-shell-core/src/drm/output.rs` reducing `initialize_output_entry` to 54 lines.
    - Recorded ADR-0034 in `docs/DECISIONS.md` documenting Phase 4 protocol consolidation in `kx-shell-core`, formal deferrals of extended protocols, and unsafe policy scope clarification.
    - Reconciled protocol count (19) and unsafe policy description in `docs/reviews/phase-4-review.md`.
- **Phase 4 (Real-App Protocol Completeness) — CLOSED:**
  - **Milestone 4F (Extended Quality Protocols) — DONE:**
    - Step 4F.1: Primary selection protocol handler in `crates/kx-shell-core/src/primary_selection.rs` (`zwp_primary_selection_device_manager_v1`) implementing `init_primary_selection_global`, `PrimarySelectionHandler`, focus-synchronized offer updates via `SeatHandler::focus_changed` per Rule 05, and integration tests in `crates/kx-shell-core/tests/primary_selection_client.rs` verifying global advertisement, binding, and device creation.
    - Step 4F.2: Single pixel buffer protocol handler in `crates/kx-shell-core/src/single_pixel_buffer.rs` (`wp_single_pixel_buffer_manager_v1`) implementing `init_single_pixel_buffer_global`, `SinglePixelBufferHandler`, 1x1 32-bit RGBA buffer creation, and integration tests in `crates/kx-shell-core/tests/single_pixel_buffer_client.rs` verifying global advertisement, binding, and 1x1 buffer allocation.
    - Step 4F.3: Content type protocol handler in `crates/kx-shell-core/src/content_type.rs` (`wp_content_type_manager_v1` and `wp_content_type_v1`) implementing `init_content_type_global`, `delegate_content_type!(KynexState)`, cached surface state storage, single-attachment enforcement, and integration tests in `crates/kx-shell-core/tests/content_type_client.rs` verifying global advertisement, binding, content type hint commit, destroy, and duplicate attachment rejection (`already_constructed`).
    - Step 4F.4: Tearing control protocol handler in `crates/kx-shell-core/src/tearing_control.rs` (`wp_tearing_control_manager_v1` and `wp_tearing_control_v1`) implementing `init_tearing_control_global`, `delegate_tearing_control!(KynexState)`, presentation hint storage in surface cached state, and integration tests in `crates/kx-shell-core/tests/tearing_control_client.rs` verifying global advertisement, binding, presentation hint commit, destroy, and duplicate attachment error (`tearing_control_exists`).
    - Step 4F.5: Idle notifier protocol handler in `crates/kx-shell-core/src/idle_notifier.rs` (`ext_idle_notifier_v1` and `ext_idle_notification_v1`) implementing `init_idle_notifier_global`, `IdleNotifierHandler`, seat idle timer registration via non-blocking Calloop timer sources, user activity resumption (`resumed`), and integration tests in `crates/kx-shell-core/tests/idle_notify_client.rs` verifying global advertisement, notification object creation, and timeout dispatch.
    - Step 4F.6: Alpha modifier protocol handler in `crates/kx-shell-core/src/alpha_modifier.rs` (`wp_alpha_modifier_v1` and `wp_alpha_modifier_surface_v1`) implementing `init_alpha_modifier_global`, `delegate_alpha_modifier!(KynexState)`, cached surface multiplier state, and integration tests in `crates/kx-shell-core/tests/alpha_modifier_client.rs` verifying global advertisement, multiplier commit, clean destruction, and duplicate attachment rejection (`already_constructed`).
    - Step 4F.7: Phase 4 Closure Gate & Documentation: updated `docs/ROADMAP.md`, `docs/SECURITY.md`, and `docs/STATE.md`. All verification gates green.
  - **Milestone 4E (Session Control & Telemetry) — DONE:**
    - Step 4E.1: Session lock protocol handler in `crates/kx-shell-core/src/session_lock.rs` (`ext_session_lock_v1` and `ext_session_lock_manager_v1`) implementing `init_session_lock_global`, `SessionLockHandler`, privileged socket peer credential gating (`is_session_lock_allowed`), fail-closed screen blanking and lock surface presentation, and integration tests in `crates/kx-shell-core/tests/session_lock_client.rs` verifying global filtering for unauthorized clients, authorized lock lifecycle, and fail-closed persistence upon client disconnect.
    - Step 4E.2: Foreign toplevel list protocol handler in `crates/kx-shell-core/src/foreign_toplevel.rs` (`ext_foreign_toplevel_list_v1` and `ext_foreign_toplevel_handle_v1`) implementing `init_foreign_toplevel_list_global`, `ForeignToplevelListHandler`, restricted socket peer credential filtering (`is_foreign_toplevel_allowed`), window lifecycle event dispatch (identifier, title, app_id, state, done, closed), and integration tests in `crates/kx-shell-core/tests/foreign_toplevel_client.rs`.
    - Step 4E.3: Presentation time protocol handler in `crates/kx-shell-core/src/presentation.rs` (`wp_presentation` and `wp_presentation_feedback`) implementing `init_presentation_global`, `delegate_presentation!(KynexState)`, `post_presentation_feedback` traversing mapped windows and layer surfaces, monotonic clock ID advertisement, and integration tests in `crates/kx-shell-core/tests/presentation_client.rs` verifying global binding, clock id event, and presentation feedback callbacks.
    - Step 4E.4: Wayland idle inhibitor protocol handler in `crates/kx-shell-core/src/idle_inhibit.rs` (`zwp_idle_inhibit_unstable_v1`) implementing `init_idle_inhibit_global`, `IdleInhibitHandler` on `KynexState`, alive surface retention, `is_idle_inhibited()` query helper, and integration tests in `crates/kx-shell-core/tests/idle_inhibit_client.rs` verifying global advertisement, inhibitor creation and destroy lifecycle, and state tracking.
  - **Milestone 4D (Text Input & IME):**
    - Step 4D.1: Server-side text input protocol handler in `crates/kx-shell-core/src/text_input.rs` (`zwp_text_input_manager_v3` and `zwp_text_input_v3`) implementing `init_text_input_global`, `delegate_text_input_manager!(KynexState)`, `SeatHandler::focus_changed` forwarding to `seat.text_input()` (`leave()`, `set_focus()`, `enter()`), `docs/SECURITY.md` table update (Public global), and integration tests in `crates/kx-shell-core/tests/text_input_client.rs` verifying global advertisement, version 1 binding, lifecycle requests (enable, set_surrounding_text, set_cursor_rectangle, commit, disable, destroy), and focus enter/leave event delivery.
  - **Milestone 4C (Cursor & Pointer Control) — DONE:**
    - Step 4C.1: Server-side cursor shape protocol handler in `crates/kx-shell-core/src/cursor_shape.rs` (`wp_cursor_shape_manager_v1` and `wp_cursor_shape_device_v1`) implementing `init_cursor_shape_global`, `CursorShapeHandler`, `delegate_cursor_shape!(KynexState)`, `docs/SECURITY.md` table update, and integration tests in `crates/kx-shell-core/tests/cursor_shape_client.rs` verifying global advertisement, version 1 binding, shape setting request dispatch, and clean destroy lifecycle.
    - Step 4C.2: Pointer constraints protocol handler in `crates/kx-shell-core/src/pointer_constraints.rs` (`zwp_pointer_constraints_v1`, `ZwpLockedPointerV1`, and `ZwpConfinedPointerV1`) implementing `init_pointer_constraints_global`, `PointerConstraintsHandler`, `delegate_pointer_constraints!(KynexState)`, `docs/SECURITY.md` table update, and integration tests in `crates/kx-shell-core/tests/pointer_constraints_client.rs` verifying global advertisement, version 1 binding, locked pointer creation and destroy, and confined pointer creation and destroy.
    - Step 4C.3: Relative pointer motions protocol handler in `crates/kx-shell-core/src/relative_pointer.rs` (`zwp_relative_pointer_manager_v1` and `ZwpRelativePointerV1`) implementing `init_relative_pointer_global`, `delegate_relative_pointer!(KynexState)`, `docs/SECURITY.md` table update, and integration tests in `crates/kx-shell-core/tests/relative_pointer_client.rs` verifying global advertisement, version 1 binding, relative pointer creation/destruction, and relative motion event delivery with dx/dy, dx_unaccel/dy_unaccel, and 64-bit microsecond timestamps.
    - Live interactive multi-app session verification: 3,025 frames rendered across Ptyxis terminal, GNOME Calculator, and Firefox on `wayland-1`, smooth event dispatch, zero panics, clean shutdown (exit code 0) triggered via compositor action.
    - Output Resize & Window Maximize Geometry Synchronization: Resolved window maximize clipping issue where windows in resized nested/DRM outputs retained stale margins (e.g. 907px width) due to unarranged `LayerMap::non_exclusive_zone()`. Updated `output_usable_geometry` to arrange layer maps and derive mode size dynamically, enhanced `update_output_size` to re-arrange `WindowManager`, reconfigure active maximized/fullscreen toplevels (`toplevel.send_configure()`), and synchronize `Space` locations (ADR-0033). Verified with integration test `test_output_resize_resizes_maximized_window` in `crates/kx-shell-core/tests/output_handling.rs`.
  - **Milestone 4B (Scaling & Viewport Transformation) — DONE:**
    - Step 4B.1: `wp_viewporter` protocol handler in `crates/kx-shell-core/src/viewporter.rs` implementing `init_viewporter_global`, `delegate_viewporter!(KynexState)`, `docs/SECURITY.md` table update, and integration tests in `crates/kx-shell-core/tests/viewporter_client.rs` verifying global advertisement, normal crop/destination lifecycle, hostile duplicate viewport rejection (`ViewportExists` error 0), invalid destination rejection (`BadValue` error 0), and invalid source coordinate rejection.
    - Step 4B.2: Fractional scale protocol handler in `crates/kx-shell-core/src/fractional_scale.rs` (`wp_fractional_scale_manager_v1` and `wp_fractional_scale_v1`) implementing `init_fractional_scale_global`, preferred fractional scale reporting in units of 120ths (`preferred_scale`), dynamic scale updates (`update_surface_fractional_scale`), `FractionalScaleHandler` integration with `Output::current_scale().fractional_scale()`, `docs/SECURITY.md` table update, and integration tests in `crates/kx-shell-core/tests/fractional_scale_client.rs` verifying global advertisement, preferred scale events (1.0x -> 120, 1.5x -> 180), clean destroy lifecycle, and hostile client duplicate `get_fractional_scale` rejection (`FractionalScaleExists` error 0).
  - **Milestone 4A (Buffer Management & Explicit Sync) — DONE:**
    - Step 4A.1: `linux-dmabuf` protocol handler in `crates/kx-shell-core/src/dmabuf.rs` (`zwp_linux_dmabuf_v1` and `zwp_linux_dmabuf_feedback_v1`) advertising version 4/5 format tables, linear modifiers for standard 32-bit formats (ARGB8888, XRGB8888, ABGR8888, XBGR8888), default device feedback tranches, plane count bounding ($1 \le \text{planes} \le 4$), dimension sanity checking ($0 < \text{dim} \le 16384$), and integration tests in `crates/kx-shell-core/tests/dmabuf_client.rs` verifying binding, feedback reception, and hostile client rejection with protocol errors.
    - Step 4A.2: Explicit synchronization protocol via `linux-drm-syncobj` (`wp_linux_drm_syncobj_manager_v1`) in `crates/kx-shell-core/src/syncobj.rs` with `init_drm_syncobj_global`, DRM render node probing (`/dev/dri/renderD128`), syncobj eventfd capability checking, `delegate_drm_syncobj!`, `docs/SECURITY.md` table update, and integration tests in `crates/kx-shell-core/tests/drm_syncobj_client.rs` verifying global advertisement, version 1 binding, surface destroy lifecycle, and hostile client duplicate `get_surface` protocol rejection.
- **Phase 0 (Foundation):**
  - Pinned toolchain (Rust 2024 / 1.99.0) and dependencies.
  - `crates/kx-config`: typed configuration, defaults, 256 KiB size boundary, two-pass version parser, proptests.
  - `crates/kx-common`: `init_logging` with `EnvFilter`, `install_panic_hook` with structured crash reports and safe file permissions.
  - Fedora Workstation 44 ARM64 VM environment verified and dev setup documented.
- **Phase 1 (Minimal Nested Compositor):**
  - **Milestone 1A (Dependencies & CLI):** Pinned Smithay 0.7.0 from crates.io with features `backend_winit`, `wayland_frontend`, `desktop`, `renderer_gl`. CLI options `--backend <winit|nested|drm|headless>`, `--config <path>`, `--max-frames <count>`, `--help`, `--version`.
  - **Milestone 1B (Event Loop & Window):** Winit nested backend, calloop event loop integration, EGL/GLES2 virgl rendering, emergency exit shortcuts (Ctrl+Shift+Escape & Ctrl+Alt+Backspace).
  - **Milestone 1C (Display & Globals):** Listening Wayland socket auto-binding (`wayland-1`), core Wayland globals (`wl_compositor`, `wl_subcompositor`, `wl_shm`, `wl_seat`, `wl_output`), per-client `ClientState` tracking with quota limits (`ClientLimits`).
  - **Milestone 1D (XDG Shell & Damage Tracking):** `xdg_wm_base` (surfaces, toplevels, popups), `Space<Window>` window management, Smithay `OutputDamageTracker` with selective region compositing, throttled `wl_surface.frame` callbacks.
  - **Milestone 1E (Seat & Input Routing):** Winit keyboard and pointer event routing, xkbcommon keymap loading, pointer focus and window-under-cursor hit detection, click-to-focus and raise, interactive window dragging with clamped coordinates.
  - **Milestone 1F (Hostile Clients & Anti-DoS):** Strict per-client surface quota enforcement with protocol error 2 (`wl_display::Error::NoMemory`) and client disconnect, automatic quota reclamation on surface destroy, hostile client integration suite (quota exhaustion, churn connect/disconnect drop, unconfigured commits), live nested run verified in Fedora ARM64 UTM GNOME Wayland session (`target/debug/kynex-comp --backend nested --max-frames 10`).
- **Post-Phase 1 Fixes:**
  - **Nested Output Orientation (Y-axis Inversion):** Configured `Transform::Flipped180` on the nested Winit output to cancel `GlesRenderer`'s internal `flip180` projection matrix (which assumes DRM hardware scanout), ensuring window contents and input coordinates render 100% right-side up. Verified visually with `weston-terminal`.
- **Phase 2 (DRM/KMS Real Hardware Backend) — CLOSED:**
  - **Milestone 2A (Dependency & Crate Foundations):** Smithay DRM/GBM/libseat/udev/libinput features enabled, `xcursor = "0.3.11"` added, `udev` LGPL-2.1 exception added to `deny.toml` (approved), `kx-input` pure-logic crate created with `BindingMatcher`, `KeyModifiers`, `KeySym`, `CompositorAction`, ADR-0014 through ADR-0019 recorded.
  - **Milestone 2B (Safety Net & Observability Foundation):**
    - Step 2B.1: `kx-input` binding matcher wired into `KynexState` and nested winit event loop with XKB keysym interception via `keyboard.input` filter (resolving winit `+8` scancode offset) supporting `Ctrl+Shift+Escape`, `Ctrl+Alt+Backspace`, and `Ctrl+Shift+Q` (rollover-safe for MacBook keyboard matrix ghosting).
    - Step 2B.2: `sd_notify` (`READY=1`, `WATCHDOG=1`, `STOPPING=1`) implemented in `kx-common::systemd` with unit tests using mock Unix datagram sockets.
    - Step 2B.3: `--exit-after <duration>` CLI timeout flag, Calloop timer integration, and child process environment sanitation (`KYNEX_FAULT` stripped).
    - Step 2B.4: Pure-logic `FrameClock` state machine (`Idle -> Queued -> WaitingForVBlank -> Idle`) with frame coalescing and moving-max render estimation in `kx-shell-core::frame_clock`.
    - Step 2B.5: Feature-gated `fault-injection` test harness (`park_timeout` hang loop, single scoped panic suppression, release `compile_error!`, simulated GPU loss `SwapBuffersError::ContextLost`, strict token parser, `+fault-injection` in `--version`).
    - Step 2B.6: Draft systemd user unit in `docs/kynex-comp.service`.
  - **Milestone 2C (Read-Only DRM Device & Session Enumeration):**
    - Step 2C.1: `DrmSession` safe wrapper over `LibSeatSession` in `kx-shell-core::drm::session` (`open`, `seat`, `is_active`, `open_device`, `close_device`, `change_vt`).
    - Step 2C.2: Udev and DRM KMS resource discovery in `kx-shell-core::drm::device` querying primary/all GPUs, CRTCs, planes, connectors, and video modes.
    - Step 2C.3: `DrmDiagnosticReport` with structured `log_report()` detailing hardware capabilities without activating modesetting.
    - Step 2C.4: `--backend drm` and `--diagnostic` CLI flags wired with 10s safety timeout default for non-diagnostic runs.
  - **Milestone 2D (DRM Modesetting, GBM/EGL Pipeline & Clear Color Render Loop):**
    - Step 2D.1: Isolated unsafe EGL driver initialization in `kx-shell-core::drm::egl` with crate-level `unsafe_code = "deny"` override and module-level `#![expect(unsafe_code)]`.
    - Step 2D.2: GPU resource bundle in `kx-shell-core::drm::gpu` with `DrmGpuResources` initializing `DrmDevice`, `GbmDevice`, `GbmAllocator`, `GbmFramebufferExporter`, and `GlesRenderer`.
    - Step 2D.3: Hardware modesetting pipeline in `kx-shell-core::drm::output` with `DrmOutputPipeline` detecting primary connected connector, preferred video mode, CRTC, and configuring `DrmOutputManager`.
    - Step 2D.4: Libinput backend in `kx-shell-core::drm::input` with `LibinputInputBackend` and `process_keyboard_event` dispatching emergency exit shortcuts (`Ctrl+Shift+Escape`, `Ctrl+Alt+Backspace`, `Ctrl+Shift+Q`).
    - Step 2D.5: Main DRM execution loop in `kx-shell-core::drm::mod` with Calloop sources for Libinput, DRM page flips, Libseat VT pause/activate, SIGINT/SIGTERM, Wayland listening socket, client event loops, and frame pacing rendering `DEFAULT_BG_COLOR` clear color.
    - Step 2D.6: Systemd `sd_notify` readiness signal dispatched upon first successful page flip submission, and stopping notification on shutdown.
    - Step 2D.7: Live hardware verification passed on TTY3 with DRM modeset, clear color render, `--exit-after 10s` clean timeout shutdown, and device drop.
  - **Milestone 2E (Multi-Output, Hardware Cursor Planes & Dynamic Hotplug):**
    - Step 2E.1: Pure-logic horizontal multi-output layout engine and CRTC allocation (`allocate_outputs`, `migrate_window_position`) in `kx-shell-core::drm::layout` with 5 unit tests.
    - Step 2E.2: Multi-output DRM rendering pipeline, per-output state tracking (`DrmOutputEntry`), and `map_output`/`unmap_output` in `kx-shell-core::drm::output`, `state.rs`, and `drm/mod.rs`.
    - Step 2E.3: Dynamic monitor hotplug via `UdevBackend` (`UdevEvent::Changed`), connector status re-probing, window migration from unplugged outputs, and zero-display headless fallback in `kx-shell-core::drm::hotplug`.
    - Step 2E.4: Hardware cursor planes enabled via `FrameFlags::ALLOW_CURSOR_PLANE_SCANOUT` with automatic compositing fallback, Libinput pointer routing (relative, absolute, click-to-focus/raise, scroll axes), and coordinate clamping across the multi-monitor bounding box (`global_bounds()`) with 5 unit tests.
    - Step 2E.5: ADR-0021 and ADR-0022 recorded in `docs/DECISIONS.md`.
  - **Milestone 2F (Performance Profiling, VBlank Telemetry & Phase 2 Closure Gate):**
    - Step 2F.1: Monotonic frame pacing telemetry in `kx-shell-core::frame_clock`: zero-allocation circular buffer (`[Duration; 64]`), `last_render_duration()`, `p99_render_duration()`, and missed vblank detection with 8 unit tests.
    - Step 2F.2: Integrated per-output frame timing logging in `DrmOutputPipeline::log_frame_timing` enforcing Rule 00 budget ($\le 4$ ms p99 render CPU time).
    - Step 2F.3: Integrated `criterion = "0.5.1"` benchmarks across `kx-config` (config parsing), `kx-input` (binding lookup), and `kx-shell-core` (frame pacing state cycle).
    - Step 2F.4: Benchmark baseline recorded in `benches/BASELINE.md` per Rule 06 with sub-microsecond and sub-nanosecond timings.
    - Step 2F.5: ADR-0023 recorded in `docs/DECISIONS.md`.
  - **Phase 2 Audit & Code Hygiene Refactoring:**
    - Zero Critical or High audit findings.
    - Decomposed `crates/kx-shell-core/src/nested.rs` into `nested/mod.rs` and `nested/event.rs`.
    - Refactored `kynex-comp/src/main.rs` and `kx-shell-core/src/drm/output.rs`.
    - Strict verification: 0 files > 500 lines, 0 production functions > 60 lines across the workspace.
- **Phase 3 (Window Management) — CLOSED:**
  - **Sub-phase 3A (Architecture & ADRs) — DONE:**
    - Step 3A.1: Architecture decisions ADR-0024 through ADR-0030 recorded in `docs/DECISIONS.md`. Implementation plan approved.
  - **Sub-phase 3B (Pure-Logic Layout Engine `kx-layout`) — DONE:**
    - Step 3B.1: Pure-logic crate `kx-layout` scaffold created with strict `#![forbid(unsafe_code)]` and zero panics.
    - Step 3B.2: Window model types in `crates/kx-layout/src/window.rs` (`WindowId`, `WindowState`, `WindowGeometry`, `SizeConstraints`, safe clamping, safe UTF-8 truncation).
    - Step 3B.3: Workspace model in `crates/kx-layout/src/workspace.rs` (`Workspace`, `WorkspaceId`, `WorkspaceManager`, anti-DoS bound `MAX_WINDOWS_PER_WORKSPACE = 256`, non-panicking `check_invariants()`).
    - Step 3B.4: Focus stack and stacking order in `crates/kx-layout/src/focus.rs` (`FocusStack`, `StackingOrder`, `FocusPolicy`, `AltTabSession` with wrap-around, `should_focus_new_window`).
    - Step 3B.5: Floating layout engine in `crates/kx-layout/src/engine.rs` (`FloatingLayout`, `CascadeTracker`, `ExclusiveMargins`, `clamp_to_reachability`, `center_window`).
    - Step 3B.6: Authoritative `WindowManager` coordinator in `crates/kx-layout/src/manager.rs` uniting window entities, workspaces, focus stacks, stacking order, and layout engines with `MapWindowParams` and invariant verification.
    - Step 3B.7: Property tests in `crates/kx-layout/tests/layout_properties.rs` with `proptest` verifying reachability clamping guarantees, exclusive margins math, size constraints clamping, and WindowManager invariant preservation.
    - Step 3B.8: Structured byte-driven layout fuzz target harness in `fuzz/fuzz_targets/layout_ops.rs` and byte fuzz test in `layout_properties.rs`.
  - **Sub-phase 3C (XDG Toplevel Lifecycle in `kx-shell-core`) — DONE:**
    - Step 3C.1: Added `kx-layout` dependency and integrated authoritative `WindowManager`, `window_map: HashMap<WindowId, WindowElement>`, and `surface_to_window_id: HashMap<WlSurface, WindowId>` into `KynexState`.
    - Step 3C.2: Implemented `sync_layout_to_space` synchronizing `WindowManager` positions/stacking into Smithay's `Space`, plus `map_new_toplevel` and `unmap_toplevel` in `crates/kx-shell-core/src/window.rs`.
    - Step 3C.3: Handled `xdg_toplevel` protocol requests: `set_maximized`, `unset_maximized`, `set_fullscreen`, `unset_fullscreen`, `set_minimized`, `ack_configure`, app_id and title changes, and buffer commit size synchronisation.
    - Step 3C.4: Integration test suite in `crates/kx-shell-core/tests/xdg_shell_client.rs` with wayland-client verifying toplevel lifecycle, state requests, and minimize/unminimize without panics.
  - **Sub-phase 3D (Interactive Move/Resize, Keybindings, and Workspaces) — DONE:**
    - Step 3D.1: Configurable primary modifier (`primary_modifier = "Alt"` or `"Logo"`) in `kx-config` and `kx-input` with collision avoidance.
    - Step 3D.2: Atomic hot-reload safety with fallback in `kx-config`.
    - Step 3D.3: Keybinding action dispatch in nested and DRM event loops: close window, toggle maximize/fullscreen, switch workspace, move window to workspace.
    - Step 3D.4: Interactive move grab: serial validation, `MoveSurfaceGrab`, reachability clamping, `set_window_geometry` on `WindowManager`.
    - Step 3D.5: Interactive resize grab: edge detection (`detect_resize_edge`), `ResizeSurfaceGrab`, arithmetic math (`calculate_resize_geometry`), min/max size constraints clamping, integration with `nested/event.rs`, `drm/input.rs`, `state.rs` (`resize_request`). Modular grab architecture (`grab/mod.rs`, `grab/move_grab.rs`, `grab/resize_grab.rs`, `grab/resize_math.rs`).
    - Step 3D.6: Workspace switching integration (`sync_layout_to_space` maps active workspace windows to space and unmaps inactive ones) + comprehensive integration tests in `crates/kx-shell-core/tests/interactive_grab.rs` (5 test scenarios covering move, resize, invalid client serial rejection, and modifier checks) and `crates/kx-shell-core/tests/xdg_shell_client.rs` (multi-window mapping, workspace isolation, moving windows across workspaces, client destroy lifecycle). ADR-0031 recorded.
  - **Live Real-World Application Validation — DONE:**
    - Integrated `wl_data_device_manager` delegation in `crates/kx-shell-core/src/data_device.rs` (`DataDeviceState`, `DataDeviceHandler`, `SelectionHandler`, `ClientDndGrabHandler`, `ServerDndGrabHandler`) required by modern GTK4/Adwaita applications at display connection time.
    - Verified nested compositor live execution with real desktop apps (`ptyxis`/`weston-terminal`, `gnome-calculator` GTK4, and `firefox`) running simultaneously on `wayland-1`.
    - Live multi-app session telemetry: 18,389 frames rendered over ~100s across apps (`ptyxis`/`weston-terminal`, `gnome-calculator`, `firefox`), smooth event dispatch, zero missed vblanks or panics, and clean shutdown (exit status 0) triggered via emergency quit shortcut.
    - Cleaned up temporary test bindings, retaining standard emergency shortcuts (`Ctrl+Shift+Escape`, `Ctrl+Alt+Backspace`, `Ctrl+Shift+Q`).
  - **Window Management UX & Interaction Enhancements — DONE:**
    - Pure-logic window decoration geometry, hit-testing, and border detection in `crates/kx-layout/src/decoration.rs` with 20 unit tests.
    - Implemented `zxdg_decoration_manager_v1` negotiation (`XdgDecorationHandler`, `XdgDecorationState`) and SolidColor titlebar + control buttons (close, maximize/restore, minimize) rendering in `crates/kx-shell-core/src/decoration.rs`.
    - Bidirectional resize cursor icons (`NsResize`, `EwResize`, `NwseResize`, `NeswResize`) on 8px window border hover, plus direct left-click border-drag resizing and titlebar dragging.
    - Vblank-synchronized, damage-driven smooth window dragging natively adapting to display refresh rate via `request_redraw()`.
    - Maintained code hygiene limits: 0 files > 500 lines, 0 functions > 60 lines, `#![forbid(unsafe_code)]`, and `just check` green.
    - Live interactive multi-app verification passed cleanly: 6,667 frames rendered across Terminal (`ptyxis`), GNOME Calculator, and Firefox, smooth 60+ FPS window dragging, border resize cursor changes, control buttons, and clean shutdown (exit code 0).
  - **Sub-phase 3F (Layer-Shell, Privilege Policy & XDG Activation) — DONE:**
    - Step 3F.1: Privileged configuration schema in `kx-config` (`[privilege.layer_shell_allow]` allow-list) with validation and unit tests (`b019290`).
    - Step 3F.2: Client credentials caching (`SO_PEERCRED` and `/proc/<pid>/exe` resolution) and layer shell policy enforcement in `kx-shell-core` (`1faed8e`).
    - Step 3F.3: Filtered `zwlr_layer_shell_v1` global (`GlobalDispatch::can_view`), per-client layer surface quota limits (`max_layer_surfaces = 8`), and exclusive zone margin recalculation updating `WindowManager::set_exclusive_margins` (`b25b83a`).
    - Step 3F.4: Multi-layer Z-order rendering in `Space` & `GlesRenderer`: `Background` -> `Bottom` -> Windows -> `Top` -> Fullscreen -> `Overlay`.
    - Step 3F.5: `xdg-activation-v1` token protocol handler (`XdgActivationState`, `XdgActivationHandler`) with token creation, quota bounding (`max_tokens = 32`), 30s TTL expiry, and focus transfer to target surface (`f5e489f`).
    - Step 3F.6: Comprehensive integration test suite in `crates/kx-shell-core/tests/layer_shell_client.rs` verifying authorized client lifecycle, global filtering for unauthorized clients, quota exhaustion disconnection, and quota reclamation upon surface destruction (`9ee6718`).
  - **Sub-phase 3G (Output Handling, Fullscreen & Rehoming) — DONE:**
    - Step 3G.1: Multi-output tracking and overlap area calculation using saturating arithmetic in `primary_output_for_window`, correctly selecting best output for windows spanning multiple displays.
    - Step 3G.2: Output-targeted fullscreening via `xdg_toplevel::set_fullscreen(target_output)` spanning target output dimensions and bypassing exclusive zones per ADR-0029.
    - Step 3G.3: Automatic SSD decoration suppression (`collect_ssd_render_elements`) and resize border hit-test suppression (`find_window_hit_test` returning `DecorationHitTest::Content`) for fullscreen windows.
    - Step 3G.4: Output unplugging and dynamic window rehoming in `migrate_windows_from_unplugged` synchronizing authoritative `WindowManager` geometry and `Space` projections per ADR-0030.
    - Step 3G.5: Integration test suite in `crates/kx-shell-core/tests/output_handling.rs` with in-process test client verifying output overlap calculation, targeted fullscreen with SSD suppression, and unplug migration.
  - **Sub-phase 3H (Hostile-Client Suite & Phase 3 Closure Gate) — DONE:**
    - Step 3H.1: Hostile-client test suite in `crates/kx-shell-core/tests/hostile_window_client.rs` with 5 scenarios: conflicting size constraints clamped safely, unsolicited configure serial flood disconnected with protocol error 4 on `xdg_wm_base`, spoofed move/resize grabs rejected, multibyte UTF-8 title/app_id string flood safely truncated to $\le 4096$ bytes, rapid state churn (maximize, fullscreen, minimize) cleanly handled.
    - Step 3H.2: Criterion benchmark for `kx-layout` in `crates/kx-layout/benches/layout_bench.rs` measuring single window map/unmap (38.86 ns), 20-window batch mapping (1.79 µs), 200 windows across 9 workspaces (127 µs), and workspace switching (73.5 ns).
    - Step 3H.3: Recorded Section 4 baseline in `benches/BASELINE.md` satisfying Rule 00 and Rule 06 budgets.
    - Step 3H.4: Full verification gate passed (`just check`, `cargo test --workspace --features fault-injection --locked`). Phase 3 exit criteria satisfied.
  - **Phase 3 Independent Audit & Findings Closure — DONE:**
    - Independent audit conducted and documented in `docs/reviews/phase-3-review.md`.
    - F-01 (High): Resolved broken intra-doc link in `crates/kx-shell-core/src/decoration.rs:231` (now `just doc` passes with `RUSTDOCFLAGS="-D warnings"`).
    - F-02 (Medium): Recorded ADR-0032 in `docs/DECISIONS.md` documenting rationale for retaining Phase 3 protocol handlers in `kx-shell-core` and Phase 4/6 extraction roadmap.
    - F-03 (Medium): Bounded popup nesting depth to `MAX_POPUP_DEPTH = 8` in `KynexState::new_popup` (`crates/kx-shell-core/src/state.rs:376`), rejecting recursive cascades via `surface.send_popup_done()` per Rules 04/05.
    - F-04 (Low): Updated `docs/SECURITY.md` privilege table classifying `zxdg_decoration_manager_v1` and `xdg_activation_v1` as Public, and added Phase 3 security review log entry.
    - Post-audit resolution addendum recorded in `docs/reviews/phase-3-review.md`. Verification suite 100% green. Phase 3 exit criteria satisfied.

  - **Milestone 4D (Text Input & IME) — DONE:**
    - Step 4D.1: `zwp_text_input_v3` protocol handler (`TextInputManagerState`, `TextInputSeat`, focus tracking on enter/leave, IME request routing).
    - Step 4D.2: `zwp_input_method_v2` protocol handler (`InputMethodManagerState`, `InputMethodHandler`, `InputMethodHandle`, non-panicking `parent_geometry`), privileged allow-list gating (`is_input_method_allowed`, `can_view` filtered global).
    - Full integration tests in `crates/kx-shell-core/tests/text_input_client.rs` (3 scenarios: manager advertisement, focus enter/leave, enable/disable roundtrips) and `crates/kx-shell-core/tests/input_method_client.rs` (3 scenarios: unprivileged hiding, privileged advertisement, non-panicking parent geometry on unmapped surface).
  - **Milestone 4E (Session Control & Telemetry) — DONE:**
    - Step 4E.1 — DONE: `ext_session_lock_v1` session locking protocol handler (`SessionLockManagerState`, `SessionLockHandler`), allow-list security filter (`is_session_lock_allowed`, `can_view` filtered global), and fail-closed guarantee per Rule 05 (compositor remains locked if client crashes or disconnects without explicit unlock). Comprehensive integration test suite in `crates/kx-shell-core/tests/session_lock_client.rs` (4 scenarios: unprivileged global hidden, privileged global advertised, lock/unlock lifecycle, and fail-closed persistence on client disconnect).
    - Step 4E.2 — DONE: `ext_foreign_toplevel_list_v1` foreign toplevel management protocol handler (`ForeignToplevelListState`, `ForeignToplevelListHandler`), allow-list security filter (`is_foreign_toplevel_allowed`, `can_view` filtered global), window map/unmap/title/app_id synchronization with `WindowManager`. Comprehensive integration test suite in `crates/kx-shell-core/tests/foreign_toplevel_client.rs` (3 scenarios: unprivileged global hidden, privileged global advertised, and full toplevel window lifecycle with title/app_id mutations and closed events).
    - Step 4E.3 — DONE: `wp_presentation` feedback with real monotonic vblank timestamps (`post_presentation_feedback` with `Time::<Monotonic>`, vsync flag, refresh duration from output mode, and feedback collection across window elements and layer surfaces). Comprehensive integration test suite in `crates/kx-shell-core/tests/presentation_client.rs` (3 scenarios: clock ID advertisement, presentation feedback presented on window surface, and discarded feedback on superseded commit).
    - Step 4E.4 — DONE: Wayland idle inhibitor protocol handler in `crates/kx-shell-core/src/idle_inhibit.rs` (`zwp_idle_inhibit_unstable_v1`) implementing `init_idle_inhibit_global`, `IdleInhibitHandler` on `KynexState`, alive surface retention, `is_idle_inhibited()` query helper, and integration tests in `crates/kx-shell-core/tests/idle_inhibit_client.rs` verifying global advertisement, inhibitor creation and destroy lifecycle, and state tracking.

## Pending-hardware items (Phase 2)
The following hardware-dependent verification items are deferred until physical test hardware is provisioned:
1. **Bare-metal multi-monitor output testing:** Verification with physical HDMI / DisplayPort / DisplayPort MST daisy chains on discrete AMD, NVIDIA, and Intel GPUs (in addition to the virtualized DRM connectors verified in UTM/QEMU).
2. **Hardware direct scanout & overlay plane assignment:** Verification of zero-copy direct scanout and video overlay plane assignments on physical atomic KMS plane hardware controllers.
3. **Bare-metal ACPI lid & power events:** Physical laptop lid switch state changes and hardware ACPI sleep/suspend/resume power cycle transitions.
4. **Physical Variable Refresh Rate (VRR / Adaptive-Sync):** Verification of dynamic vblank timing and tearing prevention on physical FreeSync / G-Sync compatible displays.
5. **Multi-GPU hybrid scanout:** Secondary GPU scanout and render offload on dual-GPU hybrid laptops (integrated Intel/AMD + discrete NVIDIA/AMD).

## In progress
- None (Phase 4 completed and verified). Ready for review and merge of `feat/phase-4f-extended-protocols` to `dev` and `main`.

## Next
- Merge `feat/phase-4f-extended-protocols` to `dev`, push `dev`, merge `dev` to `main`, and push `origin/main`.
- Begin Phase 5 — XWayland (Supervised rootless XWayland, X11 window manager, clipboard bridge, scaling).

## Verify commands
- `just check`
- `cargo test -p kx-shell-core --test alpha_modifier_client`
- `cargo test -p kx-shell-core --test idle_notify_client`
- `cargo test -p kx-shell-core --test tearing_control_client`
- `cargo test -p kx-shell-core --test content_type_client`
- `cargo test -p kx-shell-core --test single_pixel_buffer_client`
- `cargo test -p kx-shell-core --test primary_selection_client`
- `cargo test -p kx-shell-core --test idle_inhibit_client`
- `cargo test -p kx-shell-core --test presentation_client`
- `cargo test -p kx-shell-core --test foreign_toplevel_client`
- `cargo test -p kx-shell-core --test session_lock_client`
- `cargo test -p kx-shell-core --test text_input_client`
- `cargo test -p kx-shell-core --test input_method_client`
- `cargo test -p kx-shell-core --test cursor_shape_client`
- `cargo test -p kx-shell-core --test pointer_constraints_client`
- `cargo test -p kx-shell-core --test relative_pointer_client`
- `cargo test -p kx-shell-core --test viewporter_client`
- `cargo test -p kx-shell-core --test fractional_scale_client`
- `cargo test -p kx-layout`
- `cargo test -p kx-shell-core --test xdg_shell_client`
- `cargo test -p kx-shell-core --test interactive_grab`
- `cargo test -p kx-shell-core --test layer_shell_client`
- `cargo test -p kx-shell-core --test output_handling`
- `cargo test -p kx-shell-core --test hostile_window_client`
- `cargo test --workspace --features fault-injection --locked`
- `cargo check --release`
- `cargo run -p kynex-comp -- --backend nested --exit-after 2s`




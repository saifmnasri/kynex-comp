# Project State (handoff file)

> Update at the end of every task. A fresh chat session must be able to continue from this file alone.

**Current phase:** 4 — Real-App Protocol Completeness — IN PROGRESS
**Last updated:** 2026-10-14 — Phase 3 (Window Management) completed and closed. Sub-phase 3H (Hostile-Client Suite & Phase 3 Closure Gate) fully verified with 5/5 hostile-client integration tests (`hostile_window_client.rs`: conflicting size constraints clamping, unsolicited configure flood disconnects, spoofed grab rejection, multibyte title/app_id string truncation, and rapid state churn survival) and `kx-layout` Criterion benchmarks recorded in `benches/BASELINE.md` (38.86 ns single map/unmap, 1.79 µs 20-window batch, 127 µs 200 windows across 9 workspaces, 73.5 ns workspace switch). Full verification gate green (`just check`, `cargo test --workspace --features fault-injection --locked`). Phase 4 initiated.

## Done
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

## Pending-hardware items (Phase 2)
The following hardware-dependent verification items are deferred until physical test hardware is provisioned:
1. **Bare-metal multi-monitor output testing:** Verification with physical HDMI / DisplayPort / DisplayPort MST daisy chains on discrete AMD, NVIDIA, and Intel GPUs (in addition to the virtualized DRM connectors verified in UTM/QEMU).
2. **Hardware direct scanout & overlay plane assignment:** Verification of zero-copy direct scanout and video overlay plane assignments on physical atomic KMS plane hardware controllers.
3. **Bare-metal ACPI lid & power events:** Physical laptop lid switch state changes and hardware ACPI sleep/suspend/resume power cycle transitions.
4. **Physical Variable Refresh Rate (VRR / Adaptive-Sync):** Verification of dynamic vblank timing and tearing prevention on physical FreeSync / G-Sync compatible displays.
5. **Multi-GPU hybrid scanout:** Secondary GPU scanout and render offload on dual-GPU hybrid laptops (integrated Intel/AMD + discrete NVIDIA/AMD).

## In progress
- **Phase 4 (Real-App Protocol Completeness):**
  - Branch: `feat/phase-4-protocols`
  - Milestone 4A: Buffer management & explicit sync (`linux-dmabuf` v4 feedback, `linux-drm-syncobj`).

## Next
- **Milestone 4A (Buffer Management & Explicit Sync):**
  - Step 4A.1: `linux-dmabuf` protocol handler with v4 format table advertising and tranches feedback.
  - Step 4A.2: Explicit synchronization via `linux-drm-syncobj` protocol.
- **Milestone 4B (Scaling & Viewport Transformation):**
  - Step 4B.1: `wp_viewporter` implementation for cropped/scaled video buffers and games.
  - Step 4B.2: `wp_fractional_scale_v1` protocol for fractional display scales (e.g. 1.25x, 1.5x).
- **Milestone 4C (Cursor & Pointer Control):**
  - Step 4C.1: `wp_cursor_shape_v1` server-side cursor shapes.
  - Step 4C.2: `zwp_pointer_constraints_v1` (confined and locked pointer for games/FPS).
  - Step 4C.3: `zwp_relative_pointer_v1` for unaccelerated high-precision mouse input.
- **Milestone 4D (Text Input & IME):**
  - Step 4D.1: `zwp_text_input_v3` and `zwp_input_method_v2` for input methods.
- **Milestone 4E (Session Control & Telemetry):**
  - Step 4E.1: `ext_session_lock_v1` session locking protocol (fail-closed per Rule 05).
  - Step 4E.2: `ext_foreign_toplevel_list_v1` for taskbars and pagers.
  - Step 4E.3: `wp_presentation_time` feedback with real vblank timestamps.
  - Step 4E.4: `zwp_idle_inhibit_v1` and `ext_idle_notify_v1`.

## Verify commands
- `just check`
- `cargo test -p kx-layout`
- `cargo test -p kx-shell-core --test xdg_shell_client`
- `cargo test -p kx-shell-core --test interactive_grab`
- `cargo test -p kx-shell-core --test layer_shell_client`
- `cargo test -p kx-shell-core --test output_handling`
- `cargo test -p kx-shell-core --test hostile_window_client`
- `cargo test --workspace --features fault-injection --locked`
- `cargo check --release`
- `cargo run -p kynex-comp -- --backend nested --exit-after 2s`




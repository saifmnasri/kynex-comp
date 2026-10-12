# Project State (handoff file)

> Update at the end of every task. A fresh chat session must be able to continue from this file alone.

**Current phase:** 2 — Real Hardware Backend (DRM/KMS) — CLOSED (Merged into main, tagged `phase-2-complete`)
**Last updated:** 2026-10-12 — Phase 2 audit closure complete. All Critical and High audit findings resolved. Code hygiene verified across entire workspace (0 files > 500 lines, 0 production functions > 60 lines). Nested backend refactored into `crates/kx-shell-core/src/nested/`. Full verification gate passed: `just check` green (fmt, clippy `-D warnings`, nextest, cargo-deny), `cargo test --workspace --features fault-injection --locked` green, `cargo check --release` green. Phase 2 merged into `dev` and `main`, tagged `phase-2-complete`.

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

## Pending-hardware items (Phase 2)
The following hardware-dependent verification items are deferred until physical test hardware is provisioned:
1. **Bare-metal multi-monitor output testing:** Verification with physical HDMI / DisplayPort / DisplayPort MST daisy chains on discrete AMD, NVIDIA, and Intel GPUs (in addition to the virtualized DRM connectors verified in UTM/QEMU).
2. **Hardware direct scanout & overlay plane assignment:** Verification of zero-copy direct scanout and video overlay plane assignments on physical atomic KMS plane hardware controllers.
3. **Bare-metal ACPI lid & power events:** Physical laptop lid switch state changes and hardware ACPI sleep/suspend/resume power cycle transitions.
4. **Physical Variable Refresh Rate (VRR / Adaptive-Sync):** Verification of dynamic vblank timing and tearing prevention on physical FreeSync / G-Sync compatible displays.
5. **Multi-GPU hybrid scanout:** Secondary GPU scanout and render offload on dual-GPU hybrid laptops (integrated Intel/AMD + discrete NVIDIA/AMD).

## In progress
- **Phase 3 (Window Management):**
  - Branch: `feat/phase-3-window-management` (from `dev`).
  - Step 3A.1: Architecture decisions ADR-0024 through ADR-0030 recorded in `docs/DECISIONS.md`. Implementation plan approved.

## Next
- Step 3B.1: Create `kx-layout` crate scaffold (pure-logic window/workspace/focus model).
- Step 3B.2: Implement window model types with size constraints and string caps.

## Verify commands
- `just check`
- `cargo test --workspace --features fault-injection --locked`
- `cargo check --release`
- `cargo run -p kynex-comp -- --backend drm --diagnostic` (Run manually from TTY3 only)
- `cargo run -p kynex-comp -- --backend drm --exit-after 5s` (Run manually from TTY3 only)
- `cargo run -p kynex-comp -- --backend nested --exit-after 2s`


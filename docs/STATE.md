# Project State (handoff file)

> Update at the end of every task. A fresh chat session must be able to continue from this file alone.

**Current phase:** 2 — Real Hardware Backend (DRM/KMS)
**Last updated:** 2026-10-12 — Phase 2 in progress on branch `feat/phase-2-drm-backend`. Milestone 2D complete: DRM modesetting, GBM buffer allocation, EGL display/context & GLES renderer initialization, solid clear color render loop, calloop event loop wiring, Libinput input integration with emergency exit keys, and sd_notify readiness/stopping signals. `just check` green (fmt, clippy `-D warnings`, 50 tests, cargo-deny).

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
- **Phase 2 (DRM/KMS Real Hardware Backend):**
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
    - Verification: `just check` green (fmt, clippy `-D warnings`, 50 unit tests, cargo-deny).

## In progress
- Milestone 2D live hardware test drill on TTY3 (`Ctrl+Option+F3`).

## Next
- Milestone 2E: Multi-output support, hardware cursor planes, and dynamic monitor hotplug handling.
- Milestone 2F: Performance profiling, vblank pacing verification, and Phase 2 closure gate.

## Verify commands
- `just check`
- `cargo test --workspace --features fault-injection --locked`
- `cargo check --release`
- `cargo run -p kynex-comp -- --backend drm --diagnostic` (Run manually from TTY3 only)
- `cargo run -p kynex-comp -- --backend drm --exit-after 5s` (Run manually from TTY3 only)
- `cargo run -p kynex-comp -- --backend nested --exit-after 2s`

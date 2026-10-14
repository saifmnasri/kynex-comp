# Architecture Decision Records (append-only)

Format: `ADR-NNNN — title — status — context — decision — consequences`.

---
## ADR-0001 — Build on Smithay — Accepted
**Context:** Need a Rust compositor foundation that is safe, maintained, and license-compatible with closed source.
**Decision:** Use Smithay (MIT) as the Wayland/compositor toolkit. Own architecture on top; use its `anvil` example only as a reference.
**Consequences:** MIT notice must ship in third-party notices. Smithay API changes require pinned versions and careful upgrades. Some protocols may need custom work.

## ADR-0002 — Shell components are separate processes — Accepted
**Context:** Plugin systems inside the compositor process (dylibs) are ABI-fragile and turn plugin bugs into session crashes.
**Decision:** Panel, launcher, settings, lock, notifications, wallpaper run out-of-process using Wayland protocols (layer-shell, session-lock) plus versioned private IPC.
**Consequences:** Strong fault isolation and a smaller trusted base. Slightly more IPC design work. Optional WASM extensions can be revisited later.

## ADR-0003 — Single-threaded calloop event loop — Accepted
**Context:** Compositor state is large and mutable; locks add latency and deadlock risk.
**Decision:** One main thread owns all compositor state. Blocking work runs on worker threads and reports through channels.
**Consequences:** No `Arc<Mutex<State>>`. Workers must be message-based.

## ADR-0004 — Typed IDs instead of shared pointers — Accepted
**Decision:** Windows, outputs and workspaces are referenced by IDs in slot maps. No `Rc<RefCell>` in compositor state.
**Consequences:** Lookups can fail and must be handled; far fewer borrow panics and lifetime knots.

## ADR-0005 — TOML configuration, strict and versioned — Accepted
**Decision:** `serde` + `toml`, `deny_unknown_fields`, `config_version`, last-good-config fallback, atomic reload.

## ADR-0006 — No dynamic plugins in-process — Accepted (see ADR-0002)

## ADR-0007 — Proprietary license, clean-room policy — Accepted
**Decision:** Product is closed source. No GPL/LGPL/AGPL code or dependencies. GPL compositors (cosmic-comp, niri, KWin, mutter, labwc, river) are idea references only. Enforced by `cargo-deny`, rule 01, and review.

## ADR-0008 — Compositor name — Accepted
**Decision:** Product name **KynexComp**, binary `kynex-comp`, crate prefix `kx-`, session `kynex-session`.

## ADR-0009 — Config size cap 256 KiB — Accepted
**Context:** Malformed or huge configuration files fed to the parser could consume excessive memory or CPU.
**Decision:** Enforce a strict 256 KiB (262,144 bytes) size limit on configuration files in `load_from_str` before parsing.
**Consequences:** Oversized files are rejected immediately with `ConfigError::Oversized`. Legitimate desktop configs will never approach this limit.

## ADR-0010 — Manual crash report formatting without serde_json — Accepted
**Context:** During panic handling, the process state is partially degraded; allocating complex serde JSON structures in leaf `kx-common` could fail or pull in heavy dependencies.
**Decision:** Write crash reports in a structured plain text key-value format (`KynexComp Crash Report`, `version`, `pid`, `timestamp`, `location`, `message`, `Backtrace`) using standard `write!` / `writeln!`.
**Consequences:** Keeps `kx-common` free of JSON dependencies, robust during stack unwinding, and easily readable by automated tools or developers.
## ADR-0011 — Pin Smithay 0.7.0 from crates.io — Accepted
**Context:** Phase 1 requires a pinned, stable, permissive-license Wayland compositor framework compatible with `deny.toml` policies.
**Decision:** Pin `smithay = "0.7.0"` from crates.io with features `backend_winit`, `wayland_frontend`, `desktop`, `renderer_gl`. Do not use a git pin (forbidden by `deny.toml` `unknown-git = "deny"` and Rule 01).
**Consequences:** Full license compliance (MIT), reproducible builds, supported on Rust 2024 / MSRV 1.80.1. Upgrade path is explicit and gated by `just check`.

## ADR-0012 — Phase 1 crate scope (kynex-comp, kx-shell-core, kx-test-utils) — Accepted
**Context:** Rule 02 forbids speculative abstractions and empty scaffolding crates before their real requirements exist.
**Decision:** For Phase 1 (nested compositor), implement `kynex-comp` (binary and CLI), `kx-shell-core` (compositor state, backend integration, rendering loop, protocols), and `kx-test-utils` (test client harness). Crate decomposition into `kx-backend`, `kx-render`, and `kx-protocols` is deferred to Phase 2 when the DRM/KMS backend is introduced.
**Consequences:** Keeps the codebase cohesive and under active testing without premature layer fragmentation, while enforcing clean internal module separation.

## ADR-0013 — Nested Winit Output Transform (Transform::Flipped180) — Accepted
**Context:** In Smithay's `GlesRenderer`, an internal vertical reflection matrix (`flip180`) is applied to orthographic projections to compensate for DRM/KMS hardware direct scanout conventions. In the nested Winit backend, rendering targets an `EGLSurface` window rather than a hardware scanout plane, causing `Transform::Normal` to render the window content upside-down.
**Decision:** Configure the nested Winit output with `Transform::Flipped180`. This cancels out `GlesRenderer`'s internal reflection matrix ($[-1.0] \times [-1.0] = +1.0$), ensuring window contents, title bars, and pointer coordinates render right-side up. In Phase 2, real DRM/KMS outputs will use `Transform::Normal`.
**Consequences:** Nested compositor output is rendered right-side up with 1:1 input coordinate and damage tracking alignment. Clean separation of nested vs DRM transform policy.
## ADR-0014 — Defer Backend trait abstraction to Phase 3 — Accepted
**Context:** Phase 2 introduces the DRM/KMS hardware backend alongside the Phase 1 Winit nested backend. Introducing a generalized `Backend` trait abstraction across DRM and Winit before both are fully operational risks premature generalization and speculative architecture (prohibited by Rule 00 / Rule 02).
**Decision:** Implement the DRM/KMS backend as a parallel module within `kx-shell-core` (or dedicated submodules), sharing `KynexState`, `ClientState`, and protocol handlers directly. Abstract both backends behind a unified `Backend` trait in Phase 3 when multi-backend runtime switching and headless test backends are solidified.
**Consequences:** Keeps Phase 2 focused strictly on DRM/KMS hardware correctness, page flips, plane assignment, and libinput event routing without trait impedance.

## ADR-0015 — Frame clock pure-logic state machine — Accepted
**Context:** DRM frame presentation is strictly vblank-driven and requires careful state transitions (`Idle -> Queued -> WaitingForVBlank -> Idle`) to avoid double-queuing page flips and missing deadlines.
**Decision:** Model the per-output frame state machine as pure logic with an injected monotonic clock trait (`Clock` trait with `Instant` timestamps) testable with deterministic virtual time steps without needing hardware DRM devices or GPU drivers in CI.
**Consequences:** 100% unit-testable frame pacing logic, eliminating timing race conditions and edge case regressions in frame loop states.

## ADR-0016 — Pure-logic binding matcher in kx-input crate — Accepted
**Context:** Keyboard shortcuts (especially emergency exit combinations `Ctrl+Shift+Escape` and `Ctrl+Alt+Backspace`) must be evaluated before forwarding input to Wayland clients, independently of whether input originates from Winit, Libinput, or test feeds.
**Decision:** Create a dedicated pure-logic `kx-input` crate with zero dependencies on Smithay, Wayland, Calloop, or GPU libraries. The crate provides `KeyModifiers`, `KeySym`, `CompositorAction`, and `BindingMatcher`.
**Consequences:** Fast unit testing of shortcuts, strict adherence to Rule 02 (dependency DAG), and complete separation of input policy from hardware input mechanisms.

## ADR-0017 — Hand-rolled sd_notify in kx-common — Accepted
**Context:** Systemd supervision and watchdog heartbeat require sending `READY=1` and `WATCHDOG=1` over `$NOTIFY_SOCKET`. External systemd crates add unnecessary weight and transitive dependencies.
**Decision:** Implement a minimal (~30 line) safe Unix datagram socket notifier in `kx-common` (`sd_notify`). If `$NOTIFY_SOCKET` is not set or invalid, it returns `Ok(false)` gracefully.
**Consequences:** Zero external crate dependencies for systemd integration, full compatibility with systemd user services, and no risk of supply-chain bloat.

## ADR-0018 — libudev LGPL-2.1 dynamic linking approved — Accepted
**Context:** Smithay's `backend_udev` feature requires the `udev` crate, which links dynamically to system `libudev.so.1` (LGPL-2.1+). Rule 01 and `deny.toml` require explicit human authorization for LGPL dependencies.
**Decision:** User explicitly authorized dynamic linking of `libudev` via a targeted exception in `deny.toml` for crate `udev`. No proprietary code is statically linked or derived from LGPL sources.
**Consequences:** Udev device discovery and hotplug are fully functional; `cargo deny check` passes with zero violations.

## ADR-0019 — Fault-injection test harness feature — Accepted
**Context:** Testing watchdog recovery, GPU context loss recovery, and crash report generation requires deliberately triggering hangs, panics, and EGL context loss in controlled test runs. Rule 02 states "features are additive, never change semantics".
**Decision:** Add an explicit exception to Rule 02 for a `fault-injection` cargo feature under strict safety conditions: (1) hang via `std::thread::park_timeout` in a loop; (2) panic via single scoped `#[expect(clippy::panic)]` in feature-gated module; (3) `compile_error!` when enabled without `debug_assertions`; (4) release and production CI never enable `--all-features`; (5) strict, unit-tested spec parser with capped iterations; (6) strip `KYNEX_FAULT` from child environments and note `+fault-injection` in `--version` and crash logs.
**Consequences:** Compositor resilience paths can be verified in automated and manual test runs without compromising production builds or violating zero-panic rules.

## ADR-0020 — Isolated unsafe EGL driver initialization in kx-shell-core::drm::egl — Accepted
**Context:** Initializing hardware EGL contexts on a GBM device via Smithay's `EGLDisplay::new` and creating a hardware `GlesRenderer` requires calling `EGLDisplay::new`, which is marked `unsafe` by Smithay because the underlying EGL/GBM drivers execute low-level C FFI and driver pointers. Rule 00, Rule 03, and Rule 08 require `#![forbid(unsafe_code)]` by default, dedicated isolated modules for unsafe code, user approval, and `// SAFETY:` comments on every unsafe block.
**Decision:** Isolate the unsafe EGL initialization in `kx-shell-core::src::drm::egl.rs`. Add module-level `#![expect(unsafe_code, reason = "...")]` and an explicit `// SAFETY:` block documenting that the passed `GbmDevice` is a valid open GBM device with valid DRM file descriptors. Configure crate-level `[lints.rust] unsafe_code = "deny"` in `crates/kx-shell-core/Cargo.toml` so unsafe code remains forbidden everywhere else across the crate.
**Consequences:** Complies with Rule 00/03/08 unsafe policies, keeps unsafe code confined to a single 40-line function, ensures safe abstraction across all caller code in `drm::gpu` and `drm::mod`.

## ADR-0021 — Multi-output DRM horizontal layout and per-output frame pacing — Accepted
**Context:** In multi-monitor setups, connected displays have different physical resolutions, refresh rates, and CRTC assignments. Rendering multi-monitor displays on a single monolithic frame clock leads to tearing, stuttering, or stalls if one monitor has a slower refresh rate or pending page flip. Furthermore, allocating CRTCs across available connectors requires constraint satisfaction where CRTC encoders and connector bitmasks match without hardcoded indexing.
**Decision:** Implement a pure-logic allocator (`allocate_outputs` in `kx-shell-core::drm::layout`) that pairs connectors to valid CRTCs using encoder bitmasks and computes horizontal strip positions (`Point::from((x, 0))`). Drive frame pacing independently per output indexed by `crtc::Handle` in `DrmOutputPipeline`, rendering each display on its own DRM vblank event.
**Consequences:** Displays pace frames independently without cross-monitor blocking. The allocation and layout engine is pure logic and 100% unit testable without physical GPU hardware.

## ADR-0022 — Dynamic monitor hotplug and window migration via UdevBackend — Accepted
**Context:** Displays can be plugged in or unplugged at runtime. When an output is unplugged, windows mapped to its coordinates risk becoming unreachable "orphan" windows. If all outputs are unplugged, stopping the compositor kills the user session, violating Rule 00 (Stability) and Rule 06 (Degrade, do not die: "Zero outputs connected -> keep running headless; restore layout when an output returns").
**Decision:** Listen to `UdevEvent::Changed` via `UdevBackend` in Calloop. On hotplug events, re-query connector status, map new outputs into `KynexState`, and unmap disconnected outputs. When an output is unmapped, migrate any windows positioned within its geometry to the surviving primary output using `migrate_window_position`. If 0 outputs remain, maintain headless state and keep client loops alive until a monitor is re-attached.
**Consequences:** Unplug/replug survives seamlessly. Windows remain reachable at all times, preserving session continuity and fulfilling Phase 2 roadmap criteria.
## ADR-0023 — Frame timing telemetry, vblank pacing verification, and Criterion benchmarking baseline — Accepted
**Context:** Phase 2F requires verifying frame pacing against Rule 00 ($\le 4$ ms p99 render CPU time) and Rule 06 budgets, detecting missed hardware vblanks, tracking per-output timing telemetry, and establishing an automated benchmark baseline across pure-logic modules (`kx-config`, `kx-input`, `kx-shell-core`) using Criterion.
**Decision:**
1. Extend `FrameClock` in `kx-shell-core::frame_clock` with zero-allocation stack ring-buffer telemetry (`[Duration; 64]`), calculating `last_render_duration`, `p99_render_duration`, and detecting missed hardware vblanks when presentation timestamps exceed the nominal refresh interval by $> 1.5 \times \Delta t_{\text{refresh}}$.
2. Integrate periodic frame timing in `DrmOutputPipeline::log_frame_timing` at `debug` level, with `warn!` when p99 exceeds the Rule 00 4 ms budget.
3. Configure `criterion = "0.5.1"` as workspace dev-dependency across `kx-config`, `kx-input`, and `kx-shell-core`, executing pure-logic benchmarks for config parsing, shortcut matching, and frame pacing state transitions.
4. Record baseline measurements in `benches/BASELINE.md` as the official regression gate per Rule 06.
**Consequences:**
- Frame production state machine overhead is $< 9$ ns per frame ($< 0.00025\%$ of the 4 ms budget).
- Zero memory allocation during frame timing recording and percentile querying.
- Automated benchmarks verify that all pure-logic paths operate well beneath latency budgets.

## ADR-0024 — Global workspace model (Phase 3) — Accepted
**Context:** Phase 3 introduces multiple workspaces. Power users often desire per-output workspaces (where switching one monitor does not switch others), while standard desktop users expect global workspaces (switching workspaces changes all connected monitors together).
**Decision:** Adopt a global workspace model for Phase 3. The internal `WorkspaceManager` partitions windows by workspace ID (`SlotMap<WorkspaceId, Workspace>`), keeping window collections modular so per-output workspace mapping can be introduced in later phases without altering window data structures.
**Consequences:** Keeps reachability invariants, focus stack, and multi-monitor stacking clear and verifiable. Adheres to Rule 00 ("simplest design that satisfies the phase").

## ADR-0025 — Layer-shell privilege policy: deny by default with config allow-list — Accepted
**Context:** `zwlr_layer_shell_v1` surfaces can reserve exclusive screen space, display over other applications, and take keyboard focus. Allowing arbitrary Wayland clients to bind layer shell creates serious security and denial-of-service risks (Rule 05).
**Decision:** Hide the `zwlr_layer_shell_v1` global by default using `WlrLayerShellState::new_with_filter` / `GlobalDispatch::can_view`. Resolve the client executable path from `/proc/<pid>/exe` at socket connection accept time and cache it in `ClientState`. A client may only view and bind layer shell if its absolute executable path matches an entry in `[privilege.layer_shell_allow]` (or UID check passes). If path resolution fails, access is denied immediately (fail closed). Every denial logs a structured `tracing::warn!` with PID, UID, and resolved path. Re-check privilege on `get_layer_surface`.
**Consequences:** Complete isolation against unprivileged rogue clients attempting to occlude the desktop or spoof system UI.

## ADR-0026 — Server-side decorations: title-less geometry with colored square buttons — Accepted
**Context:** Server-side decorations (SSD) require drawing borders, title bars, and window control buttons. Drawing text titles requires font rasterization (`ab_glyph`), asset management, and font file licensing, adding significant dependency surface before Phase 7.
**Decision:** Implement title-less server-side decorations with solid-colored geometric buttons: red square for close, green square for maximize, yellow square for minimize. Geometry calculation and hit-testing are pure logic in `kx-layout`. Decorations are drawn via `SolidColorRenderElement`. Fullscreen windows completely hide decorations.
**Consequences:** Zero external font dependencies, zero glyph rasterization overhead, crisp UI, fully testable hit-testing.

## ADR-0027 — Minimize behavior: real minimize with Alt+Tab and recovery restore — Accepted
**Context:** Wayland clients can request minimization via `xdg_toplevel::set_minimized`. Phase exit criteria mandate that if minimize is implemented, a reliable restore path must exist.
**Decision:** Implement real minimization. Minimized windows are unmapped from Smithay's `Space` and omitted from rendering, but retained in `FocusStack` and `WindowManager`. Restoring can occur via `Alt+Tab` focus cycling (which visits and unminimizes minimized windows) or via a dedicated recovery keybinding (`Super+C` in DRM, `Ctrl+Alt+C` in dev) which unminimizes and centers the topmost minimized window.
**Consequences:** Full window lifecycle support without stranded/lost windows.

## ADR-0028 — Config schema versioning: additive version 1 with last-good retention — Accepted
**Context:** Phase 3 introduces new configuration sections: `[keybindings]`, `[privilege]`, `[decorations]`, and `[workspaces]`. The current config version is `1`.
**Decision:** Maintain `config_version = 1`. New sections are purely additive with `#[serde(default)]` and sensible defaults, preserving compatibility with existing Phase 1 and 2 configs. If a malformed configuration or invalid keybinding combination is provided on config hot-reload, the compositor logs a structured error, retains the last known good configuration in memory, and continues running.
**Consequences:** Zero breaking changes to existing deployments, robust hot-reload resilience.

## ADR-0029 — Fullscreen Z-order and interaction rules — Accepted
**Context:** Windows in `Fullscreen` state must interact properly with layer surfaces (status bars, docks, wallpapers) and server-side decorations.
**Decision:**
1. Fullscreen windows sit above the `Top` layer surface layer (between `Top` and `Overlay`).
2. Server-side decorations are hidden while a window is in `Fullscreen` state.
3. Layer-surface exclusive zones are ignored when positioning and rendering fullscreen windows (the window occupies the full display mode bounds).
4. Compositor emergency exit (`Ctrl+Alt+Shift+Q`) always preempts fullscreen windows.
**Consequences:** Proper immersive fullscreen gaming/video presentation without dock occlusion or decoration artifacts.

## ADR-0030 — Authoritative WindowManager and Space projection synchronization — Accepted
**Context:** In Smithay, `Space<Window>` handles window spatial placement and rendering element iteration. Having business logic directly mutate `Space` without a central model leads to desynchronized workspace, focus, and reachability state.
**Decision:** `WindowManager` in `kx-layout` is the sole authoritative source of truth for window geometry, state, focus, and workspace assignment. Smithay's `Space` is treated as a pure projection updated exclusively through a single synchronization function: `sync_space(&mut self)`. Every model mutation calls `sync_space()` and schedules a frame (`request_frame()`). An integration test asserts that `WindowManager` and `Space` are in complete agreement after every operation.
**Consequences:** Clear architectural boundary, eliminate desync bugs, fully testable state machine independent of Smithay rendering.

## ADR-0031 — Pointer grab architecture for interactive move and resize — Accepted
**Context:** Wayland clients can request interactive move and resize via `xdg_toplevel.move` and `xdg_toplevel.resize`. The compositor also initiates move and resize via modifier clicks (Mod+Left click for move, Mod+Right click for resize). Untrusted client requests can supply bogus serials, and manual dragging can pull windows entirely off-screen or violate size bounds.
**Decision:**
1. Validate client serials against pointer button down serial (`pointer.button_down_serial()`); reject requests with mismatched or stale serials without crashing or entering a grab.
2. Implement `MoveSurfaceGrab` and `ResizeSurfaceGrab` using Smithay's `PointerGrab` trait, encapsulated in modular sub-units (`grab/move_grab.rs`, `grab/resize_grab.rs`, `grab/resize_math.rs`).
3. Clamp window positions during moves to retain at least titlebar reachability height (32px) and margin (32px) within display bounds.
4. Detect resize edges from relative pointer position using an 8-zone perimeter margin (12px) if unassigned, and clamp resized geometry against window `min_size` / `max_size` constraints and minimum physical limits (64x64px).
5. Compositor Mod+click grabs (`try_start_compositor_grab`) raise the window, activate focus, and initiate the corresponding grab.
**Consequences:** Complete protection against client grab spoofing; windows can never be dragged completely off-screen; resize operations strictly preserve application size constraints.

## ADR-0032 — Placement of Phase 3 Wayland protocol handlers in `kx-shell-core` — Accepted
**Context:** Rule 02 outlines a dedicated `kx-protocols` crate for Wayland protocol handlers. Sub-phases 3E and 3F introduced handlers for `zxdg_decoration_manager_v1`, `zwlr_layer_shell_v1`, and `xdg_activation_v1`. Amendment A6 permitted implementing these protocol handlers in `kx-shell-core` temporarily, provided an ADR is recorded justifying why they remain there rather than in a separate crate.
**Decision:** Keep the protocol state fields, handler trait implementations, and `delegate_*!` macro expansions for `zxdg_decoration`, `zwlr_layer_shell`, and `xdg_activation` in `kx-shell-core` throughout Phase 3. Extracting them to a separate `kx-protocols` crate before all core protocols (such as `linux-dmabuf` and `fractional-scale` in Phase 4) are finalized would introduce artificial trait abstraction churn and borrow-checker impedance between `KynexState` and the protocol crate while compositor state is actively evolving. Extraction into a dedicated `kx-protocols` crate is scheduled for Phase 4 or Phase 6 when protocol boundaries and shared compositor state interfaces are fully stabilized.
**Consequences:** Keeps Phase 3 changes cohesive within `kx-shell-core`, eliminates speculative glue traits, and satisfies Amendment A6 without compromising crate boundary modularity.

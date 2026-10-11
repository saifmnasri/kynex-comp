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

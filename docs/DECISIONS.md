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

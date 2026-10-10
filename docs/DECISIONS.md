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

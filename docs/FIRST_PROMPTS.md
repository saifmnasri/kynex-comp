# Prompt Playbook (copy-paste, in order)

Rules for you (the human):
- One prompt at a time. Wait for the agent's summary. Run `just check` yourself and read the diff.
- Start every **new chat** with `/resume`. Never rely on chat history; `docs/STATE.md` is the memory.
- Use a stronger model (if your plan offers one) for architecture decisions and `/security-review`; use the fast model for implementation.
- Commit after every green prompt.

---
## Prompt 0.1 — Verify the environment
```
/resume
Task: verify the development environment only. Do not write feature code.
1. Check rustc/cargo versions, and whether just, cargo-nextest, cargo-deny, cargo-audit are installed (do not install anything; list what is missing and the exact install command).
2. Run `cargo generate-lockfile` and `just check`.
3. Fix only what is broken in the starter kit itself (config typos, lint errors in kx-common). Ask before touching deny.toml or rust-toolchain.toml.
4. Update docs/STATE.md. Report what you verified and what you could not.
```

## Prompt 0.2 — Logging, panic hook, config skeleton (pure logic)
```
Phase 0. Create crate `kx-config` using /new-crate (pure logic, no smithay).
- Typed config structs with serde + toml, deny_unknown_fields, `config_version = 1`, defaults, validation returning typed errors (thiserror).
- Function `load_from_str(&str) -> Result<Config, ConfigError>` with a 256 KiB size cap.
- Tests: valid file, unknown field rejected, oversized rejected, defaults applied, proptest that arbitrary strings never panic.
Ask me before adding any dependency; propose the exact list with licenses first.
```

## Prompt 0.3 — Observability foundation
```
Phase 0. In kx-common add logging initialisation using tracing + tracing-subscriber (env-filter, compact format), and a panic-hook installer that writes a structured crash report to $XDG_STATE_HOME/kynex/crash/ (mode 0600) with message, location, backtrace, version. Keep it free of unsafe and of any Smithay dependency. Add tests that use a temp dir injected as a parameter (no real HOME access). Propose dependency additions first.
```

## Prompt 1.1 — Choose and pin Smithay (decision only)
```
Phase 1 start. Do not write compositor code yet.
1. Check crates.io for the latest Smithay release and its MSRV, enabled features needed for: winit backend, wayland frontend, desktop helpers, GLES renderer, DRM/libinput/session later.
2. Compare "latest release from crates.io" vs "a pinned git revision" and recommend one, with the license check (cargo deny) results.
3. Propose the exact `[workspace.dependencies]` entries. WAIT for my approval, then add them and record ADR-0009 in docs/DECISIONS.md.
```

## Prompt 1.2 — Nested window with clear colour
```
Phase 1. Create crate `kynex-comp` (binary) and `kx-backend` (winit backend only). Goal: open a winit window under the current Wayland session, set up the calloop event loop, create the Wayland display socket, render a solid colour with Smithay's renderer, exit cleanly on window close and on SIGINT/SIGTERM.
Follow rule 04 rule zero: before each Smithay call, show me the file path in the pinned source where you verified it. No wl_compositor yet. Add a `--backend winit` CLI flag (no extra CLI crate unless approved). Update STATE.md.
```

## Prompt 1.3 — Core protocols so a terminal appears
```
Phase 1. Implement in kx-protocols, one module each, verified against the pinned Smithay and the anvil example for reference only: wl_compositor, wl_subcompositor, wl_shm, xdg_shell (toplevels first), wl_seat with keyboard+pointer from winit input, wl_output. Introduce kx-shell-core with a `State` split into sub-structs and typed WindowId/OutputId (slotmap, with approval).
Acceptance: `WAYLAND_DISPLAY=<ours> foot` shows a window that accepts keyboard input and resizes. Include a protocol test via kx-test-utils (create it as needed) and a hostile-client test (zero-size buffer, bad ack_configure serial). Report what you did NOT verify.
```

## Prompt 1.4 — Damage tracking, frame callbacks, popups, clipboard
```
Phase 1. Add damage-tracked rendering, correct frame callbacks (only after the frame showing the surface), xdg popups with grabs, and wl_data_device (clipboard + DnD) with the focused-client-only rule from rule 05. Tests for each. Then run /security-review on the diff and show me the findings table before fixing anything.
```

## Prompt template for all later prompts
```
Phase <N>. Task: <one concern, one crate>.
Context: <what exists>. Constraints: <rules that matter here>.
Acceptance criteria: <observable, testable>.
Process: plan first, verify APIs in pinned source, small diff, tests (incl. hostile-client if protocol), `just check`, update docs/STATE.md, summarise and propose next prompt.
```

# KynexComp — Agent Entry Point

You are working on **KynexComp** (`kynex-comp`), the closed-source, commercial Wayland
compositor of **KynexOS**. Language: Rust. Foundation: Smithay. Goal: enterprise-grade
stability, security, speed and modularity, competitive with Wayfire and KDE KWin.

## Read this first, every new session

1. `docs/STATE.md`    — what is done, what is in progress, known issues (the handoff file).
2. `docs/ROADMAP.md`  — the phase plan. Work only on the current phase.
3. `docs/DECISIONS.md`— architecture decisions already made. Do not re-litigate them.

## Rule files (in `.agents/rules/`)

| File | Activation | Purpose |
|------|-----------|---------|
| `00-identity-and-prime-directives.md` | Always On | Priorities and non-negotiables |
| `01-licensing-and-ip.md` | Always On | Closed-source / no-GPL guardrails |
| `08-agent-protocol.md` | Always On | HOW you must work (verify, small steps, handoff) |
| `02-architecture-and-modularity.md` | Glob `**/*.rs`, `**/Cargo.toml` | Crate layout, boundaries |
| `03-rust-standards.md` | Glob `**/*.rs` | Rust style, errors, unsafe policy |
| `04-smithay-and-wayland.md` | Glob `**/*.rs` | Smithay/Wayland patterns and protocol list |
| `05-security.md` | Glob `**/*.rs` | Threat model and hardening |
| `06-performance-and-stability.md` | Glob `**/*.rs` | Frame loop, budgets, crash resilience |
| `07-testing-and-ci.md` | Model Decision | Tests, fuzzing, CI |
| `09-git-docs-release.md` | Model Decision | Commits, ADRs, releases |

Workflows (type `/name` in chat): `/check`, `/resume`, `/new-crate`, `/add-protocol`, `/security-review`.

## The five rules you must never forget

1. Never invent a Smithay or Wayland API. Verify it in the pinned source before using it.
2. Never copy code from GPL/LGPL/AGPL projects (cosmic-comp, niri, KWin, mutter, labwc, river...).
3. No `unwrap`/`expect`/`panic!`/unchecked indexing in runtime code. No `unsafe` without approval.
4. Never block the main event loop.
5. After every change run `just check`. Never say "done" until it passes.

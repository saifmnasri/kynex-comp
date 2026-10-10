# KynexComp — Starter Kit

Closed-source Rust Wayland compositor for **KynexOS**, built on Smithay. This kit contains the rules, workflows,
workspace config and docs that make an AI agent (Antigravity IDE) build it safely and consistently.

## Install into Antigravity

1. Create your project folder and copy **everything in this kit** into its root (keep the dot-folders).
2. Open the folder in Antigravity as a workspace (it should be the Git root).
3. Open the **Customizations** panel (the "..." menu at the top of the agent panel) → **Rules**.
   - Confirm all `.agents/rules/*.md` files are listed. If they are not, rename `.agents/` to `.agent/`
     (older versions use that path) or recreate them with **+ Workspace**.
   - Confirm activation modes: `00`, `01`, `08` = **Always On**; `02`-`06` = **Glob** (`**/*.rs`);
     `07`, `09` = **Model Decision**. If the panel shows different modes, set them there — the panel wins.
   - Each rule file is under the 12,000-character limit. Keep it that way when you edit.
4. `AGENTS.md` at the root is also read by Antigravity (v1.20.3+); it is a short index plus the five critical rules.
5. Workflows: type `/check`, `/resume`, `/new-crate`, `/add-protocol`, `/security-review` in the agent chat.
6. `git init`, install the Rust tools listed in `docs/DEV-SETUP.md`, then start with `docs/FIRST_PROMPTS.md` → Prompt 0.1.

## Layout

```
AGENTS.md                  entry point + index
.agents/rules/             10 rule files
.agents/workflows/         5 workflows
docs/                      STATE (handoff), ROADMAP, DECISIONS, ARCHITECTURE, SECURITY, DEV-SETUP, FIRST_PROMPTS
Cargo.toml, justfile, clippy.toml, rustfmt.toml, deny.toml, rust-toolchain.toml   quality gates
crates/kx-common/          minimal starting crate (everything else is created phase by phase)
```

## Honest limits

Rules reduce risk; they cannot guarantee a bug-free or unhackable compositor. Plan for human review of every
security-critical diff, fuzzing, an external security audit before release, and a lawyer-reviewed license/EULA.

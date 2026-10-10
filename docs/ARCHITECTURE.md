# KynexComp Architecture

(Authoritative rules: `.agents/rules/02-architecture-and-modularity.md`. This file describes data flow and is updated when boundaries change.)

## Process model

```
 KynexOS session
 ├── kynex-comp              ← this repo: compositor (trusted, privileged-ish, never crashes)
 │     ├── Xwayland          ← supervised child, rootless, restartable
 │     └── IPC socket        ← $XDG_RUNTIME_DIR/kynex/ipc.sock (0600, peer-cred checked)
 ├── kynex-panel / launcher / lock / wallpaper / notifications   ← separate processes (layer-shell, session-lock)
 └── user apps (Wayland clients, sandboxed ones via security-context)
```

## Main-loop data flow

```
 calloop ─┬─ Wayland display fd ─► kx-protocols handlers ─► kx-shell-core State (windows, outputs, workspaces)
          ├─ libinput fd ────────► kx-input (bindings, gestures, focus) ─► State / focused client
          ├─ DRM vblank events ──► frame state machine (per output) ─► kx-render ─► kx-backend (queue frame)
          ├─ timers / animations ► kx-anim ─► State ─► damage ─► schedule frame
          ├─ IPC socket ─────────► kx-ipc ─► State commands (policy-checked)
          ├─ config watcher ─────► kx-config ─► validate ─► atomic apply
          └─ worker channels ────► results of blocking work (decode, spawn, D-Bus)
```

## Frame lifecycle (per output)

1. Damage arrives (client commit, input, animation tick, output change) → output marked `Queued`.
2. Wait until the estimated latest-safe start time before vblank.
3. Build scene: collect render elements top to bottom, compute damage, attempt plane assignment (direct scanout / overlays / cursor).
4. Render only damaged regions if composition is needed; submit to KMS (atomic commit).
5. On vblank: send presentation feedback and frame callbacks, return to `Idle` (or `Queued` if more damage).

## Crates

See the layout and dependency DAG in rule 02. Add one line per crate here when it is created:

| Crate | Layer | Pure logic? | Responsibility |
|-------|-------|-------------|----------------|
| `kx-common` | leaf | yes | shared leaf types, logging init, structured panic hook |
| `kx-config` | config | yes | typed configuration, validation, defaults, two-pass parsing |

# KynexComp Security Model

Authoritative rules: `.agents/rules/05-security.md`. This file records the threat model, the privilege table, and known limitations.

## Privilege table (fill in as protocols are implemented)

| Global / IPC command | Class | Who may use it | Consent needed |
|----------------------|-------|----------------|----------------|
| `wl_compositor`, `wl_shm`, `xdg_wm_base`, `zxdg_decoration_manager_v1`, `xdg_activation_v1`, `zwp_linux_dmabuf_v1`, `zwp_linux_dmabuf_feedback_v1` | Public | any client | no |
| `zwlr_layer_shell_v1`, idle-inhibit, foreign-toplevel list | Restricted | allow-listed or shell clients | no |
| screencopy / image-copy-capture | Privileged | portal backend only | yes (portal prompt) |
| data-control, virtual input, output-management, input-method, gamma | Privileged | allow-listed executables only | policy |
| `ext_session_lock_v1` | Privileged | lock client only; fail closed | policy |

## Known limitations

- X11 clients under XWayland can observe and interact with each other (inherent to X11). Sandboxed apps must use Wayland.
- Identification by executable path is best-effort and racy; privileged access must also rely on allow-list ownership checks and security-context.
- Kernel/root compromise is out of scope.

## Review log

- **2026-10-14 (Phase 3 Security & Window Management Review):** Scope: Phase 3 window management, layer shell, xdg-activation, SSD decoration, and hostile-client protection. Outcome: PASS. Verified deny-by-default privilege evaluation for `zwlr_layer_shell_v1` (`can_view` filtered), socket peer credential caching (`SO_PEERCRED` and `/proc/<pid>/exe` resolution), activation token bounding (32 tokens, 30s TTL), resource limits per client (`max_surfaces = 64`, `max_layer_surfaces = 8`), popup depth boundary (`MAX_POPUP_DEPTH = 8`), string truncation ($\le 4096$ bytes), and clean grab lifecycle management.

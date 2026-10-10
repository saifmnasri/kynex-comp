# KynexComp Security Model

Authoritative rules: `.agents/rules/05-security.md`. This file records the threat model, the privilege table, and known limitations.

## Privilege table (fill in as protocols are implemented)

| Global / IPC command | Class | Who may use it | Consent needed |
|----------------------|-------|----------------|----------------|
| `wl_compositor`, `wl_shm`, `xdg_wm_base`, ... | Public | any client | no |
| `zwlr_layer_shell_v1`, idle-inhibit, foreign-toplevel list | Restricted | allow-listed or shell clients | no |
| screencopy / image-copy-capture | Privileged | portal backend only | yes (portal prompt) |
| data-control, virtual input, output-management, input-method, gamma | Privileged | allow-listed executables only | policy |
| `ext_session_lock_v1` | Privileged | lock client only; fail closed | policy |

## Known limitations

- X11 clients under XWayland can observe and interact with each other (inherent to X11). Sandboxed apps must use Wayland.
- Identification by executable path is best-effort and racy; privileged access must also rely on allow-list ownership checks and security-context.
- Kernel/root compromise is out of scope.

## Review log

(Append the date, scope, and outcome of each `/security-review` here.)

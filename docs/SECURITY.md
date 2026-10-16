# KynexComp Security Model

Authoritative rules: `.agents/rules/05-security.md`. This file records the threat model, the privilege table, and known limitations.

## Privilege table (fill in as protocols are implemented)

| Global / IPC command | Class | Who may use it | Consent needed |
|----------------------|-------|----------------|----------------|
| `wl_compositor`, `wl_shm`, `xdg_wm_base`, `zxdg_decoration_manager_v1`, `xdg_activation_v1`, `zwp_linux_dmabuf_v1`, `zwp_linux_dmabuf_feedback_v1`, `wp_linux_drm_syncobj_manager_v1`, `wp_viewporter`, `wp_fractional_scale_manager_v1`, `wp_cursor_shape_manager_v1`, `zwp_relative_pointer_manager_v1`, `zwp_text_input_manager_v3`, `wp_presentation` | Public | any client | no |
| `zwlr_layer_shell_v1`, `zwp_pointer_constraints_v1` (focus gated), idle-inhibit, `ext_foreign_toplevel_list_v1` | Restricted | allow-listed or shell clients / active focus | no |
| `screencopy` / image-copy-capture | Privileged | portal backend only | yes (portal prompt) |
| `zwp_input_method_manager_v2`, data-control, virtual input, output-management, gamma | Privileged | allow-listed executables only | policy |
| `ext_session_lock_v1` | Privileged | lock client only; fail closed | policy |

## Known limitations

- X11 clients under XWayland can observe and interact with each other (inherent to X11). Sandboxed apps must use Wayland.
- Identification by executable path is best-effort and racy; privileged access must also rely on allow-list ownership checks and security-context.
- Kernel/root compromise is out of scope.

## Review log

- **2026-10-14 (Phase 3 Security & Window Management Review):** Scope: Phase 3 window management, layer shell, xdg-activation, SSD decoration, and hostile-client protection. Outcome: PASS. Verified deny-by-default privilege evaluation for `zwlr_layer_shell_v1` (`can_view` filtered), socket peer credential caching (`SO_PEERCRED` and `/proc/<pid>/exe` resolution), activation token bounding (32 tokens, 30s TTL), resource limits per client (`max_surfaces = 64`, `max_layer_surfaces = 8`), popup depth boundary (`MAX_POPUP_DEPTH = 8`), string truncation ($\le 4096$ bytes), and clean grab lifecycle management.
- **2026-10-15 (Step 4E.1 Session Lock Security Review):** Scope: `ext_session_lock_v1` privileged global filter and fail-closed lifecycle. Outcome: PASS. Verified `can_view` global filter hiding `ext_session_lock_manager_v1` from unauthorized clients, allow-list peer credential gating (`is_session_lock_allowed`), and fail-closed persistence per Rule 05 (compositor remains locked if lock client disconnects or crashes unexpectedly without `UnlockAndDestroy`).
- **2026-10-16 (Step 4E.2 Foreign Toplevel Security Review):** Scope: `ext_foreign_toplevel_list_v1` restricted global filter and window state leakage protection. Outcome: PASS. Verified `can_view` global filter hiding `ext_foreign_toplevel_list_v1` from unauthorized clients, allow-list peer credential gating (`is_foreign_toplevel_allowed`), safe string delivery (title/app_id bounded by compositor bounds), and clean unmap/closed event dispatch preventing handle retention.
- **2026-10-16 (Step 4E.3 Presentation Time Feedback Security Review):** Scope: `wp_presentation` global and feedback callback dispatch. Outcome: PASS. Classified as Public global (per protocol roadmap, available to any client requesting presentation timing/vsync synchronization), monotonic clock ID advertisement (CLOCK_MONOTONIC = 1), bounded feedback object lifecycle, and safe feedback dispatch via Window and LayerSurface traversals without unwrap/panics.
- **2026-10-16 (Step 4E.4 Idle Inhibit Security Review):** Scope: `zwp_idle_inhibit_unstable_v1` protocol handler and active inhibitor tracking. Outcome: PASS. Verified safe inhibitor registration (`inhibit`) and uninhibiting (`uninhibit`), dead surface filtration via `Resource::is_alive`, safe lifecycle without panics/unwraps, and complete removal of inhibitors upon object destruction.


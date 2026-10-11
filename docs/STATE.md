# Project State (handoff file)

> Update at the end of every task. A fresh chat session must be able to continue from this file alone.

**Current phase:** 1 — Minimal Nested Compositor (Completed, ready for Phase 2)
**Last updated:** 2026-10-11 — Phase 1 completed and verified: Smithay 0.7.0 pinned, Winit nested backend with GLES2 virgl acceleration, emergency exit keybindings, core Wayland globals & XDG shell, seat input routing (keyboard, pointer motion, click-to-focus, window drag), anti-DoS surface quotas, hostile client resilience suite, live nested run verified in Fedora ARM64 UTM session, `just check` green (fmt, clippy -D warnings, 32 tests, cargo deny).

## Done
- **Phase 0 (Foundation):**
  - Pinned toolchain (Rust 2024 / 1.99.0) and dependencies.
  - `crates/kx-config`: typed configuration, defaults, 256 KiB size boundary, two-pass version parser, proptests.
  - `crates/kx-common`: `init_logging` with `EnvFilter`, `install_panic_hook` with structured crash reports and safe file permissions.
  - Fedora Workstation 44 ARM64 VM environment verified and dev setup documented.
- **Phase 1 (Minimal Nested Compositor):**
  - **Milestone 1A (Dependencies & CLI):** Pinned Smithay 0.7.0 from crates.io with features `backend_winit`, `wayland_frontend`, `desktop`, `renderer_gl`. CLI options `--backend <winit|nested|drm|headless>`, `--config <path>`, `--max-frames <count>`, `--help`, `--version`.
  - **Milestone 1B (Event Loop & Window):** Winit nested backend, calloop event loop integration, EGL/GLES2 virgl rendering, emergency exit shortcuts (Ctrl+Shift+Escape & Ctrl+Alt+Backspace).
  - **Milestone 1C (Display & Globals):** Listening Wayland socket auto-binding (`wayland-1`), core Wayland globals (`wl_compositor`, `wl_subcompositor`, `wl_shm`, `wl_seat`, `wl_output`), per-client `ClientState` tracking with quota limits (`ClientLimits`).
  - **Milestone 1D (XDG Shell & Damage Tracking):** `xdg_wm_base` (surfaces, toplevels, popups), `Space<Window>` window management, Smithay `OutputDamageTracker` with selective region compositing, throttled `wl_surface.frame` callbacks.
  - **Milestone 1E (Seat & Input Routing):** Winit keyboard and pointer event routing, xkbcommon keymap loading, pointer focus and window-under-cursor hit detection, click-to-focus and raise, interactive window dragging with clamped coordinates.
  - **Milestone 1F (Hostile Clients & Anti-DoS):** Strict per-client surface quota enforcement with protocol error 2 (`wl_display::Error::NoMemory`) and client disconnect, automatic quota reclamation on surface destroy, hostile client integration suite (quota exhaustion, churn connect/disconnect drop, unconfigured commits), live nested run verified in Fedora ARM64 UTM GNOME Wayland session (`target/debug/kynex-comp --backend nested --max-frames 10`).

- **Post-Phase 1 Fixes:**
  - **Nested Output Orientation (Y-axis Inversion):** Configured `Transform::Flipped180` on the nested Winit output to cancel `GlesRenderer`'s internal `flip180` projection matrix (which assumes DRM hardware scanout), ensuring window contents and input coordinates render 100% right-side up. Verified visually with `weston-terminal`.

## In progress
- (none — Phase 1 and post-phase-1 nested orientation fix complete and visually verified live with weston-terminal)

## Next
1. Human review and merge of `feat/phase-1-nested-compositor` into `main`.
2. Phase 2 — DRM/KMS Backend & Real Hardware:
   - Scaffold `kx-backend` and separate backend traits (`Backend`, `OutputManager`).
   - Integrate `libseat` (session management via logind/seatd without root privileges).
   - DRM/KMS scanout, GBM allocation, mode setting, page flip events, hardware plane assignments (overlay/cursor planes).
   - Libinput device discovery and event handling.
   - Multi-monitor output topology and hotplug support.

## Known issues
- `cargo deny check` warns on duplicate `windows-sys` and `syn` versions pulled transitively by upstream dependencies, but passes advisory, ban, license, and source checks.

## Open questions for the human
- Target hardware and display configuration for Phase 2 DRM testing (virtual DRM device / virgl vs bare-metal GPU passthrough).
- Desired default keybinding scheme for workspace navigation and window management in Phase 2.

## Verify commands
- `just check`
- `cargo test --workspace --all-features --locked`
- `just run-nested` or `cargo run -p kynex-comp -- --backend nested --max-frames 30`

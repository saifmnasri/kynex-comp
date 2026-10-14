# Independent Audit Report: Phase 3 — Window Management

**Reviewer Role:** Independent Skeptical Auditor  
**Date:** 2026-10-14  
**Target:** KynexComp (`kynex-comp`)  
**Scope:** Commits since `phase-2-complete` (`d0424665d4635d6a11b08590e48026a5e2cd9e26..6142dce823cc128afe06ea1948f83d18dc809a85`, 54 commits)  
**Standard of Review:** `AGENTS.md`, `.agents/rules/` (Rules 00-09), `docs/ROADMAP.md`, `docs/DECISIONS.md`, `docs/STATE.md`, Phase 3 Implementation Plan and Amendments A1–A11.

---

## 1. Executive Verdict & Top 5 Risks

### Verdict: **CLOSE WITH CONDITIONS**

Phase 3 establishes an exceptionally solid, mathematically proven, and robust window management foundation. The functional core (`kx-layout`) is 100% pure logic with zero panics, zero unwrap/expect, and exhaustive proptests. However, closure is conditional on resolving 1 High finding (`just doc` failure) and 2 Medium findings before advancing to Phase 4.

### Top 5 Risks
1. **Broken Documentation Build (High):** `just doc` fails with exit code 101 due to an unresolved intra-doc hyperlink in `crates/kx-shell-core/src/decoration.rs:231:67`.
2. **Protocol Monolith Tech Debt (Medium):** Protocol handlers (`layer_shell`, `activation`, `decoration`) were added directly to `kx-shell-core` instead of a separate `kx-protocols` crate without the ADR required by Amendment A6.
3. **Uncapped Popup Cascades (Medium):** `new_popup` in `state.rs:376` relies on Smithay's `PopupManager` without enforcing an explicit compositor-level popup nesting depth cap or sending `popup_done`.
4. **Documentation Drift in Security Model (Low):** `docs/SECURITY.md` privilege table does not reflect `zxdg_decoration_manager_v1` or `xdg_activation_v1`, and the security review log is empty.
5. **Deferred Physical Hardware Verification (Low):** Bare-metal DRM multi-monitor MST daisy-chaining and hardware overlay direct scanout remain deferred to Phase 9.

---

## 2. Commit Range & Command Verifications

### Commit Range
`d0424665d4635d6a11b08590e48026a5e2cd9e26..6142dce823cc128afe06ea1948f83d18dc809a85` (54 commits total).

### Verification Commands & Results

| Command | Exit Code | Result Summary & Relevant Output |
|:---|:---:|:---|
| `just check` | **0** | **PASS:** Format check, clippy (`-D warnings`), all workspace tests, and `cargo deny check` passed cleanly. |
| `just doc` | **101** | **FAIL:** `error: unresolved link to \`xdg_toplevel::ResizeEdge\`` at `crates/kx-shell-core/src/decoration.rs:231:67`. |
| `cargo deny check` | **0** | **PASS:** Advisories, bans, licenses, and sources all verified against `deny.toml`. |
| `cargo audit` | **101** | **NOT VERIFIED:** Binary not installed (`cargo install cargo-audit`). |
| `cargo check --workspace --release --locked` | **0** | **PASS:** Clean release build without warnings. |
| `cargo check --workspace --release --locked --features fault-injection` | **101** | **PASS (Guard Verified):** Failed with `compile_error!("Fault-injection harness must never be enabled in release builds per Rule 06.")` in `crates/kx-shell-core/src/fault.rs:12:1`. |

---

## 3. Findings Table

| ID | Severity | Area | Rule Violated | File:Line | Evidence | Suggested Fix | Effort |
|:---|:---|:---|:---|:---|:---|:---|:---|
| **F-01** | **High** | Build & Docs | Rule 00, Rule 08 | `crates/kx-shell-core/src/decoration.rs:231` | `just doc` fails with exit code 101 on unresolved link ``[`xdg_toplevel::ResizeEdge`]`` | Replace link with ``[`smithay::wayland::shell::xdg::ResizeEdge`]`` or plain backticks | 5 min |
| **F-02** | **Medium** | Architecture | Rule 02, Amendment A6 | `crates/kx-shell-core/src/` | Protocol modules remain in `kx-shell-core` without ADR justifying why `kx-protocols` was not created | Record ADR-0032 in `docs/DECISIONS.md` documenting placement justification | 15 min |
| **F-03** | **Medium** | Protocol Anti-DoS | Rule 04, Rule 05 | `crates/kx-shell-core/src/state.rs:376` | `new_popup` delegates to `PopupManager` without explicit popup nesting depth counter or `popup_done` on exceed | Add nesting depth validation in `ClientState` / `KynexState` and send `popup_done` if limit exceeded | 30 min |
| **F-04** | **Low** | Documentation | Rule 05, SECURITY.md | `docs/SECURITY.md:7-14, 23` | Privilege table omits `zxdg_decoration` and `xdg_activation`; review log empty | Update privilege table with Public classification and append Phase 3 review log entry | 10 min |
| **F-05** | **Low** | Tooling | Rule 08 | Environment | `cargo-audit` command missing | Run `cargo install cargo-audit` to verify database | 5 min |

---

## 4. Detailed Area Evaluations (A through K)

### Area A: Architecture & Modularity — PASS WITH NOTES
- **DAG & Isolation:** Crate DAG strictly maintained: `kx-common` $\leftarrow$ `kx-config` $\leftarrow$ `kx-input` $\leftarrow$ `kx-layout` $\leftarrow$ `kx-shell-core` $\leftarrow$ `kynex-comp`.
- **Pure-Logic Core:** `kx-layout`, `kx-config`, `kx-input`, and `kx-common` contain zero dependencies on `smithay`, `wayland-server`, `calloop`, or GPU/EGL crates.
- **Authoritative Model:** `WindowManager` in `kx-layout` is strictly authoritative. `Space<WindowElement>` in Smithay is treated solely as a projection synchronized via `sync_layout_to_space()`.
- **Hygiene Limits:** 0 files $> 500$ lines, 0 functions $> 60$ lines across all crates.
- *Note:* Protocols were added to `kx-shell-core` rather than a separate `kx-protocols` crate; requires ADR-0032 per Amendment A6 (Finding F-02).

### Area B: Window Management — PASS
- **Lifecycle & Protocols:** Complete XDG toplevel configure/ack serialization, size constraints clamping, min/max dimensions enforcement, and title/app_id truncation ($\le 4096$ bytes).
- **Window States:** Real minimization with `FocusStack` retention and restore paths via `Alt+Tab` focus cycling and `Super+C` recovery keybinding. Fullscreen and maximization store and restore original floating geometry.
- **Workspaces & Focus:** Global workspace manager with `MAX_WINDOWS_PER_WORKSPACE = 256` anti-DoS limit. Focus stack maintains MRU order with click-to-focus and new-window focus rate-limiting.

### Area C: Reachability — PASS
- **On-Screen Invariant:** Titlebar and window geometry strictly clamped via `clamp_to_reachability` requiring $\ge 32\text{px}$ titlebar grab height and $\ge 32\text{px}$ width on-screen.
- **Recovery:** `CompositorAction::RecoverTopmostWindow` unminimizes, clamps, and centers topmost windows.
- **Output Migration:** Hotplug monitor removal triggers `migrate_windows_from_unplugged`, rehoming windows onto remaining outputs without stranded surfaces.
- **Fullscreen Z-Order:** Fullscreen windows sit above `Top` layer surfaces, hide server-side decorations, and bypass exclusive margins per ADR-0029.

### Area D: Input System — PASS
- **Binding Matcher:** Keybindings validated and configurable via `kx-config` with atomic fallback.
- **Emergency Exits:** Hard-coded, unrebindable emergency shortcuts (`Ctrl+Shift+Escape`, `Ctrl+Alt+Backspace`, `Ctrl+Shift+Q`, `Ctrl+Alt+Shift+Q`) matched before any client forwarding and unaffected by client grabs or shortcut inhibitors.
- **Interactive Grabs:** `MoveSurfaceGrab` and `ResizeSurfaceGrab` strictly validate client pointer button serials against `button_down_serial()`; invalid/spoofed serials are rejected.
- **Grab Cleanup:** Grabs terminate cleanly on pointer release, focus loss, surface destroy, or monitor unplug.

### Area E: Security & Threat Model (Rule 05) — PASS WITH NOTES
- **Privilege Policy:** Deny-by-default privilege evaluation. Client executable path resolved once from `/proc/<pid>/exe` at socket accept time via `SO_PEERCRED` and cached in `ClientState`.
- **Hidden Globals:** `zwlr_layer_shell_v1` global is filtered via `GlobalDispatch::can_view` and completely invisible to unauthorized clients.
- **Activation Tokens:** Bounded by `max_activation_tokens = 32`, single-use consumption, and 30s TTL expiry.
- **Resource Quotas:** Per-client limits enforced: `max_surfaces = 64`, `max_layer_surfaces = 8`, `max_activation_tokens = 32`. Exceeding limits triggers protocol error 2 (`wl_display::Error::NoMemory`) and client disconnect.
- **Log Privacy:** Zero window titles, clipboard contents, or client text strings logged at `info` or higher.
- *Note:* `docs/SECURITY.md` table needs update for `zxdg_decoration` and `xdg_activation` (Finding F-04).

### Area F: Protocol Correctness — PASS WITH NOTES
- **Handlers:** `xdg_shell`, `zxdg_decoration_manager_v1`, `zwlr_layer_shell_v1`, and `xdg_activation_v1` handlers correctly reject unconfigured commits and double-assigned roles.
- **Protocol Errors:** Unsolicited configure serial flood posts protocol error 4 on `xdg_wm_base`. Quota exhaustion posts protocol error 2 on `wl_surface`.
- *Note:* Missing compositor-level popup nesting depth limit and `popup_done` response when exceeding depth cap (Finding F-03).

### Area G: Frame Loop & Performance (Rule 06) — PASS
- **Event-Driven:** No polling or busy loops. Every state mutation schedules a redraw via `request_frame()` / `request_redraw()`.
- **Zero Allocations:** Hot-path hit-testing and frame pacing telemetry use fixed-size stack ring-buffers.
- **Benchmarks Established:** Criterion benchmarks in `crates/kx-layout/benches/layout_bench.rs` and documented in `benches/BASELINE.md`:
  - `map_unmap_single_window`: **38.86 ns**
  - `map_20_windows_batch`: **1.79 µs**
  - `map_200_windows_distributed`: **127.06 µs** (well under 1 ms main thread budget)
  - `switch_workspace_with_20_windows`: **73.54 ns**

### Area H: Test Suite & Invariants — PASS
- **Hostile-Client Suite:** 5 comprehensive hostile client integration tests in `hostile_window_client.rs` verifying conflicting size clamping, unsolicited configure flood disconnects, spoofed grab rejection, multibyte string flood truncation, and rapid state churn survival.
- **Property Tests:** 5 property tests in `crates/kx-layout/tests/layout_properties.rs` running directly in `just check` without `#[ignore]` covering reachability clamping, exclusive margins math, size constraints, arbitrary byte fuzzing, and WindowManager invariant preservation.
- **Regression Check:** Zero tests deleted or weakened since Phase 2. All 54 commits maintained strict test assertions.

### Area I: Documentation Integrity — PASS WITH NOTES
- **State & Decisions:** `docs/STATE.md` accurately tracks all completed sub-phases. `docs/DECISIONS.md` has ADR-0024 through ADR-0031.
- *Note:* Broken intra-doc link in `decoration.rs` and security table omissions noted in Findings F-01 and F-04.

### Area J: Scope & Intellectual Property (Rule 01) — PASS
- **Licensing:** Proprietary headers present on all `.rs` files. Zero GPL/LGPL code copied. Clean-room implementation based solely on Wayland XML protocol specifications.
- **Scope Discipline:** No premature features from Phases 4–7 (no tiling, shaders, blur, animations, or XWayland scaffolding).

### Area K: Roadmap Exit Criteria — PASS WITH NOTES
- **Automated Verification:** All programmatic Phase 3 exit criteria in `docs/ROADMAP.md` are **VERIFIED** by automated test evidence.
- **Manual Verification:** Real-world multi-app execution verified with Terminal, Firefox, and Calculator in nested session (human-reported). Bare-metal DRM multi-monitor MST testing remains marked **PENDING HARDWARE**.

---

## 5. Exit Criteria Verification Table

| Roadmap Phase 3 Criterion | Status | Evidence Type & Reference |
|:---|:---:|:---|
| Window abstraction & Authoritative WindowManager | **VERIFIED** | Unit & integration tests (`kx-layout::WindowManager`, `sync_layout_to_space`) |
| Global workspace model with isolation | **VERIFIED** | `crates/kx-shell-core/tests/xdg_shell_client.rs:test_multiple_windows_and_workspace_switching` |
| Floating layout & cascading placement | **VERIFIED** | `crates/kx-layout/src/engine.rs`, `crates/kx-layout/tests/manager.rs` |
| Focus policy & MRU focus stack | **VERIFIED** | `crates/kx-layout/tests/manager.rs:test_focus_cycling_and_recovery` |
| Interactive move & resize with serial validation | **VERIFIED** | `crates/kx-shell-core/tests/interactive_grab.rs` (5 integration tests) |
| Popups & grabs | **PARTIALLY** | Protocol verified; missing explicit compositor popup nesting depth cap (F-03) |
| Server-side decorations (`xdg-decoration`) | **VERIFIED** | `zxdg_decoration_manager_v1` handler, solid geometric buttons, 6,667 frame live test |
| Privileged layer shell (`zwlr_layer_shell_v1`) | **VERIFIED** | `crates/kx-shell-core/tests/layer_shell_client.rs` (4 integration tests) |
| XDG activation (`xdg-activation-v1`) | **VERIFIED** | `crates/kx-shell-core/src/activation_tests.rs` (3 unit tests) |
| Fullscreen, maximize, and minimize with restore | **VERIFIED** | `crates/kx-shell-core/tests/xdg_shell_client.rs:test_xdg_toplevel_maximize_and_fullscreen` |
| Multi-output windows and unplug migration | **VERIFIED** | `crates/kx-shell-core/tests/output_handling.rs:test_output_unplug_window_migration` |
| Windows never become unreachable | **VERIFIED** | `crates/kx-layout/tests/layout_properties.rs:reachability_clamping_property` (proptest) |
| Hostile-client test suite passed | **VERIFIED** | `crates/kx-shell-core/tests/hostile_window_client.rs` (5 integration tests) |
| Benchmarks established in `benches/BASELINE.md` | **VERIFIED** | Section 4 in `benches/BASELINE.md` (38.86 ns map/unmap, 1.79 µs 20-win, 127 µs 200-win) |
| Daily-usable desktop ergonomics | **HUMAN-REPORTED** | 18,389 frames live nested test with Terminal, Calculator, and Firefox |

---

## 6. Docs-Versus-Code Discrepancies

1. **`crates/kx-shell-core/src/decoration.rs:231` vs `cargo doc`:** Code references ``[`xdg_toplevel::ResizeEdge`]`` which cannot be resolved by rustdoc, breaking `just doc`.
2. **`docs/SECURITY.md` vs Code:** The privilege table does not list `zxdg_decoration_manager_v1` (Public) or `xdg_activation_v1` (Public/Rate-limited). The `/security-review` log is unpopulated.
3. **`docs/DECISIONS.md` vs Amendment A6:** Protocol handlers were added directly to `kx-shell-core` instead of creating `kx-protocols`. Amendment A6 allowed this provided an ADR was recorded; no ADR was added.

---

## 7. Recommended Fix Order

### Must-Do Before Starting Phase 4 (Effort: ~30 minutes total)
1. **Fix Broken Doc Link (F-01):** Fix `crates/kx-shell-core/src/decoration.rs:231` so `just doc` passes cleanly.
2. **Record Protocol Placement ADR (F-02):** Add ADR-0032 in `docs/DECISIONS.md` explaining that protocol handlers reside in `kx-shell-core` during Phases 1–3 to minimize crate boundary overhead, with extraction to `kx-protocols` scheduled for Phase 4 or 6.
3. **Update Security Documentation (F-04):** Update `docs/SECURITY.md` privilege table and append the Phase 3 audit entry to the review log.

### What Can Wait for Phase 4 / Phase 8
1. **Popup Depth Cap & `popup_done` (F-03):** Can be addressed in Phase 4 when subsurfaces and popup grab trees are hardened.
2. **Tooling Installation (F-05):** Run `cargo install cargo-audit` in developer environment.
3. **Bare-Metal Hardware Tests:** Deferred to Phase 9 release verification.

---

## 8. Manual-Test Checklist for User

The following items cannot be fully verified in headless CI and should be manually confirmed by the developer:

- [ ] **Interactive Window Resizing:** Start `./target/debug/kynex-comp --backend nested`, run `weston-terminal`, hover over all 8 window borders and corners, confirm cursor icon changes (`NsResize`, `EwResize`, `NwseResize`, `NeswResize`), and resize smoothly.
- [ ] **SSD Button Hit-Testing:** Click the red button to close, green button to maximize/restore, yellow button to minimize.
- [ ] **Recovery Keybinding:** Minimize a window, press `Super+C` (or `Ctrl+Alt+C` in dev mode), and confirm the window unminimizes and centers on-screen.
- [ ] **Alt+Tab Focus Cycling:** Launch 3 windows, hold `Alt` and tap `Tab` to cycle forward, hold `Alt+Shift` and tap `Tab` to cycle backward.
- [ ] **Emergency Exit Preemption:** While a window is in fullscreen mode, press `Ctrl+Shift+Escape` or `Ctrl+Alt+Backspace` and confirm immediate, clean compositor exit.

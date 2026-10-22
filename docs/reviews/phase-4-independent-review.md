# Phase 4 Independent Audit Report: Real-App Protocol Completeness

**Review Type:** Independent, Skeptical Read-Only Audit  
**Auditor:** Antigravity Independent Systems Reviewer  
**Date:** 2026-10-16  
**Target:** KynexComp (`kynex-comp`) Wayland Compositor (KynexOS)  
**Evaluation Standard:** `AGENTS.md`, `.agents/rules/` (Rules 00–09), `docs/ROADMAP.md`, `docs/DECISIONS.md`, `docs/SECURITY.md`, `docs/STATE.md`, approved Phase 4 plan.  
**Scope:** Commit range `b66f219..00a540a` (26 commits from Phase 3 audit closure `b66f219` to Phase 4 completion report `00a540a`).  
**Working Tree State:** Clean (read-only; no modifications to production files, no formatting, no commits).

---

## 1. Verdict & Top 5 Risks

### Verdict: NOT READY

Phase 4 cannot be closed in its current state. While protocol surface definitions, Smithay handler delegates, and headless protocol roundtrips are in place, critical security, architectural, and runtime flaws exist where protocols are disconnected from the compositor's rendering and input loops, privileges are dangerously conflated, and the completion report makes multiple demonstrably false claims.

### Top 5 Risks:

1. **Session Lock Bypass & Input Snooping (Sev-1 / Area G):** `is_session_locked` and `lock_surfaces` are never referenced in either the nested or DRM rendering loops or the input dispatch loops. Desktop windows continue rendering, receiving pointer clicks, and gaining keyboard focus while the session is locked; lock surfaces are never drawn.
2. **Global Privilege Conflation (Sev-1 / Area F, I):** `PrivilegeConfig` contains only `layer_shell_allow`. `inspect_and_cache_privilege` maps layer-shell, input-method, session-lock, and foreign-toplevel all to this single list. Any allowed panel or status bar is automatically authorized to lock the screen, capture all keystrokes as an IME, and inspect all windows.
3. **Dead Idle Notifier Event Loop (Sev-2 / Area H):** `ext_idle_notifier_v1` creates an isolated Calloop event loop that is never registered with or dispatched by the main compositor loop, and user input events never invoke `notify_idle_activity`. Idle timers will never fire in production.
4. **Missing DRM Presentation Feedback (Sev-2 / Area C, J):** `post_presentation_feedback` is called in nested mode but completely omitted from the DRM hardware output pipeline (`crates/kx-shell-core/src/drm/output.rs`). Clients waiting for vblank presentation events on real hardware or VMs will hang or drop frames.
5. **Incomplete Phase 4 Roadmap & Functional Gaps (Sev-3 / Area A, D, E):** Tablet, touch, gestures, and shortcut inhibition protocols promised in `docs/ROADMAP.md` Phase 4 are absent; fractional scale is not updated on output changes or window migration; pointer constraints never re-activate upon focus acquisition.

---

## 2. Commit Range & Verification Scope

### Scope:
- **Baseline Commit:** `b66f219` (`docs: record Phase 3 audit findings closure in STATE`)
- **HEAD Commit:** `00a540a` (`docs: record Phase 4 comprehensive audit and closure report`)
- **Total Commits in Scope:** 26 commits across branch `dev`.

### Commands Run and Verifications:

1. **`just check`**  
   - **Command:** `just check` (runs `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`)  
   - **Exit Code:** `0`  
   - **Output Summary:** All 12 crates formatted; Clippy passed with zero warnings; 85 integration and unit tests passed across workspace; `cargo-deny` passed all license, advisory, and ban checks.

2. **`just doc`**  
   - **Command:** `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`  
   - **Exit Code:** `0`  
   - **Output Summary:** Documentation generated with zero rustdoc warnings and zero broken intradoc links.

3. **`cargo deny check`**  
   - **Command:** `cargo deny check`  
   - **Exit Code:** `0`  
   - **Output Summary:** `advisories ok, bans ok, licenses ok, sources ok`.

4. **`cargo audit`**  
   - **Command:** `cargo audit`  
   - **Exit Code:** `101`  
   - **Output:** `error: no such subcommand: 'audit'`  
   - **Status:** **NOT VERIFIED** (Tool missing on host system; install command: `cargo install cargo-audit`).

5. **Release Build**  
   - **Command:** `cargo build --release --workspace --locked`  
   - **Exit Code:** `0`  
   - **Output Summary:** Clean compilation with LTO and optimizations in 1m 07s; artifacts created in `target/release/`.

6. **Fault-Injection Release Guard Verification**  
   - **Command:** `cargo build --release --features fault-injection`  
   - **Exit Code:** `101`  
   - **Output Snippet:**  
     ```text
     error: Feature 'fault-injection' is strictly forbidden in release builds (requires debug_assertions)
       --> crates/kx-shell-core/src/fault.rs:12:1
        |
     12 | compile_error!("Feature 'fault-injection' is strictly forbidden in release builds (requires debug_assertions)");
        | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
     ```  
     Confirmed that compile-guard prevents fault-injection feature in production releases per ADR-0019.

7. **Dependency Tree & License Audit**  
   - **Command:** `cargo tree -d` (detect duplicate crates in graph)  
   - **Exit Code:** `0` (Zero duplicate crates).  
   - **Git Diff of `Cargo.lock` (`b66f219..00a540a`):** Zero new crates added. Dev-dependency `wayland-protocols` in `crates/kx-shell-core/Cargo.toml:26` had feature `"staging"` enabled. All crate licenses strictly adhere to the allow-list (MIT / Apache-2.0 / BSD-3-Clause).

8. **Greps over Non-Test Runtime Code:**  
   - `unsafe`: 0 matches in non-test runtime code outside pre-approved `crates/kx-shell-core/src/drm/egl.rs:37` (ADR-0020).  
   - `unwrap()` / `expect()`: 0 in non-test runtime code.  
   - `panic!` / `todo!` / `unimplemented!` / `unreachable!`: 0 in non-test runtime code.  
   - `println!` / `eprintln!` / `dbg!`: 0 across workspace.  
   - `thread::sleep`: 0 across non-test runtime code.  
   - `process::exit`: 0 across libraries (only permitted in CLI `main.rs`).  
   - `Rc<RefCell<_>>` / `Arc<Mutex<_>>`: 0 in compositor state.  
   - `static mut`: 0 across workspace.  
   - `TODO` comments: 0 untagged TODOs.

9. **Lint Integrity Verification:**  
   - Inspected `[lints] workspace = true` across all crates: present in all 12 crates.  
   - Injected temporary `let _ = Some(1).unwrap();` into non-test runtime code in `crates/kx-common/src/lib.rs`.  
   - Ran `cargo clippy -p kx-common -- -D clippy::unwrap-used`: failed immediately with exit code `101` citing `-D clippy::unwrap-used`.  
   - Cleanly reverted file; `git status` verified clean.

10. **File and Function Size Limits (Rule 02 & Rule 03):**  
    - **Files $\ge 400$ lines (Limit: 500 lines):**  
      - `crates/kx-layout/src/manager.rs`: 499 lines  
      - `crates/kx-shell-core/src/drm/output.rs`: 494 lines  
      - `crates/kx-input/src/binding.rs`: 487 lines  
      - `crates/kx-shell-core/src/state.rs`: 470 lines  
      - `crates/kx-shell-core/src/drm/mod.rs`: 457 lines  
      - `crates/kx-shell-core/src/nested/event.rs`: 453 lines  
      - `crates/kx-shell-core/src/window.rs`: 448 lines  
      - `crates/kx-shell-core/src/frame_clock.rs`: 445 lines  
      - `crates/kx-shell-core/src/client.rs`: 408 lines  
      - `crates/kynex-comp/src/main.rs`: 400 lines  
      *Note:* No file exceeds 500 lines, but multiple files sit between 450–499 lines, dangerously close to the boundary.  
    - **Functions $> 60$ lines (Rule 03 violation: Functions must be $\le 60$ lines):**  
      - `crates/kx-config/src/load.rs:244` (`semantic_validation`): **73 lines** (VIOLATION)  
      - `crates/kx-shell-core/src/drm/output.rs:394` (`scan_connected_connectors`): **65 lines** (VIOLATION)  
      - `crates/kx-shell-core/src/drm/output.rs:330` (`initialize_output_entry`): **63 lines** (VIOLATION)  
      - `crates/kx-shell-core/src/state.rs:145` (`KynexState::new_with_output`): **61 lines** (VIOLATION)  
      *Finding:* The completion report claims "Functions $\le 60$ lines" across the board. This claim is **FALSE**.

### What Could NOT Be Verified:
- **`cargo audit`:** Executable not present in environment. Marked unverified.
- **Physical DRM Hardware / GPU Scanout:** DRM execution on host hardware is strictly prohibited by Rule 05/08 in this environment without physical seat isolation.
- **Interactive App Validation (Firefox, Chromium, Electron, Games, Video Players):** GUI application execution claims in the author's report cannot be verified autonomously by the audit tool and are classified as unverified claims.

---

## 3. Findings Table

| ID | Severity | Area | Rule Violated | File:Line | Evidence | Suggested Fix | Effort |
|:---|:---:|:---:|:---:|:---|:---|:---|:---:|
| **SEC-01** | **Sev-1** | Area G (Session Lock) | Rule 05 (Session Lock Fail Closed), Rule 00 | `crates/kx-shell-core/src/nested/event.rs:328-340`, `nested/event.rs:214-225`, `crates/kx-shell-core/src/session_lock.rs:43-73` | `is_session_locked` and `lock_surfaces` are never referenced in either render loops or input dispatch. Desktop windows render normally and receive input while locked. Lock surfaces are never drawn. | In `nested/event.rs` and `drm/output.rs`, check `is_session_locked`. When locked, render ONLY lock surfaces or black screen. Gating input loop: route pointer/keyboard focus strictly to lock surfaces; suppress window clicks. | Medium (2-3 days) |
| **SEC-02** | **Sev-1** | Area F, Area I (Privilege Model) | Rule 05 (Privilege Model), Rule 00 | `crates/kx-config/src/types.rs:152-159`, `crates/kx-shell-core/src/client.rs:195-202`, `crates/kx-shell-core/src/nested/event.rs:385` | `PrivilegeConfig` contains only `layer_shell_allow`. `inspect_and_cache_privilege` sets all 4 privileges (layer shell, input method, session lock, foreign toplevel) to the identical boolean. Any status bar can lock session or capture keystrokes. | Add separate allow-lists in `PrivilegeConfig` (`session_lock_allow`, `input_method_allow`, `foreign_toplevel_allow`). Evaluate each privilege independently against peer credentials in `inspect_and_cache_privilege`. | Small (0.5 day) |
| **BUG-03** | **Sev-2** | Area H (Idle Notify) | Rule 02 (Event Loop), Rule 06 | `crates/kx-shell-core/src/idle_notify.rs:45`, `nested/event.rs`, `drm/mod.rs` | `init_idle_notify_global` creates an isolated Calloop event loop stored as `idle_loop`. `dispatch_idle_timers` is only called in tests, never on the compositor loop. User input never calls `notify_idle_activity`. Timers never fire. | Register `IdleNotifierState` timers directly onto the main compositor Calloop event loop handle instead of a detached loop. Hook input events in `nested/event.rs` and `drm/mod.rs` to call `notify_idle_activity`. | Small (1 day) |
| **BUG-04** | **Sev-2** | Area C, Area J (Frame Loop) | Rule 04 (Rendering), Rule 06 | `crates/kx-shell-core/src/drm/output.rs:200-235` | `post_presentation_feedback` is called in nested mode but completely omitted from DRM `render_frame`. Clients waiting for vblank feedback on DRM will never receive `presented` events. | Call `post_presentation_feedback` in `DrmOutputPipeline::render_frame` upon successful page flip completion. | Small (0.5 day) |
| **BUG-05** | **Sev-2** | Area E (Pointer Constraints) | Rule 04 (Protocol Correctness) | `crates/kx-shell-core/src/pointer_constraints.rs:43`, `nested/event.rs`, `drm/mod.rs` | `maybe_activate_pointer_constraint` is implemented and exported in `lib.rs:72`, but never invoked anywhere in input dispatch or window focus change. Constraints requested before surface focus are never activated when focus arrives. | Invoke `maybe_activate_pointer_constraint` inside `focus_changed` or whenever a window/surface receives pointer focus. | Small (0.5 day) |
| **BUG-06** | **Sev-2** | Area D (Fractional Scale) | Rule 04 (Protocol Correctness) | `crates/kx-shell-core/src/fractional_scale.rs:35-46` | `update_surface_fractional_scale` is only invoked during `new_fractional_scale`. It is never called when an output scale changes or when a window migrates to a different output. | Call `update_surface_fractional_scale` during output scale changes and window migration across outputs. | Small (0.5 day) |
| **SEC-07** | **Sev-3** | Area H (Idle Inhibit) | Rule 05 (Privilege & DoS), Rule 00 | `crates/kx-shell-core/src/idle_inhibit.rs:48`, `crates/kx-shell-core/src/idle_inhibit.rs:25` | `init_idle_inhibit_global` creates an unconstrained public global without a `can_view` filter (despite docs claiming it is Restricted), and `self.idle_inhibitors` lacks a per-client count limit (unbounded collection). | Add `can_view` check in `init_idle_inhibit_global` (or update SECURITY.md if intentional public), and cap the number of active inhibitors per client in `ClientState`. | Small (0.5 day) |
| **SPEC-08** | **Sev-3** | Area A (Inventory) | Rule 04 (Protocol Roadmap), Rule 00 | `docs/ROADMAP.md:27-31` | Roadmapped protocols from Phase 4 are missing: `tablet` (`wp_tablet_manager_v2`), `touch` (`wl_touch`), `gestures (pinch/swipe/hold)` (`pointer_gestures_v1`), `keyboard-shortcuts-inhibit`, `xdg-dialog`, `xdg-toplevel-icon`, `xdg-system-bell`. | Implement missing protocols or update `ROADMAP.md` via an ADR deferring them to Phase 6/8. | Large (1-2 weeks) |
| **ARCH-09** | **Sev-3** | Area B (Architecture) | Rule 02, Rule 03 | `crates/kx-shell-core/src/`, `crates/kx-shell-core/src/state.rs:145`, `crates/kx-config/src/load.rs:244` | All 19 protocols implemented in `kx-shell-core` without an ADR for Phase 4 (ADR-0032 only covered Phase 3). Functions exceed 60 lines (`semantic_validation` 73 lines, `scan_connected_connectors` 65 lines, `initialize_output_entry` 63 lines, `KynexState::new_with_output` 61 lines). | Refactor functions under 60 lines. Extract protocols into `kx-protocols` or write an ADR justifying retention in `kx-shell-core`. | Medium (2-3 days) |
| **PERF-10** | **Sev-3** | Area C (Buffers & Scanout) | Rule 06 (Frame loop) | `crates/kx-shell-core/src/dmabuf.rs:30`, `crates/kx-shell-core/src/drm/output.rs:198` | Only `Modifier::Linear` advertised in dmabuf. Feedback tranches contain only default device tranche (zero render/scanout tranches). Direct scanout is hardcoded disabled (`ALLOW_DIRECT_SCANOUT` absent). | Query DRM/GBM format modifiers dynamically; add render and scanout tranches to dmabuf feedback; enable direct scanout testing for fullscreen surfaces. | Medium (3-5 days) |
| **DOC-11** | **Sev-4** | Area 5, Area L (Docs Integrity) | Rule 00, Rule 08 | `docs/reviews/phase-4-review.md:34`, `crates/kx-shell-core/Cargo.toml:36` | Completion report claims `#![forbid(unsafe_code)]` enforced across all workspace crates. In reality, `crates/kx-shell-core` sets `unsafe_code = "deny"` to permit `drm/egl.rs` per ADR-0020. | Correct statement in Phase 4 review and documentation to accurately reflect ADR-0020. | Trivial (10 mins) |

---

## 4. Area-by-Area Review (A–N)

### A. Protocol Inventory: PASS WITH NOTES
- **Reconciliation:** The author's report text claims "18 standard and extension Wayland protocols", but its implementation matrix and test matrix list 19 protocols. Reconciled: exactly 19 protocol globals are registered in `KynexState`.
- **Roadmap Discrepancy:** `docs/ROADMAP.md` Phase 4 explicitly lists `tablet`, `touch`, `gestures (pinch/swipe/hold)`, `keyboard-shortcuts-inhibit`, `xdg-dialog`, `xdg-toplevel-icon`, and `xdg-system-bell`. None of these protocols are implemented in `kx-shell-core`. Either an ADR must formally defer them, or they must be marked unfulfilled.

### B. Architecture & Modularity: FAIL
- **Violation of Rule 02:** Amendment A6 and ADR-0032 permitted placing Phase 3 protocols (`zxdg_decoration`, `zwlr_layer_shell`, `xdg_activation`) in `kx-shell-core` temporarily. However, Phase 4 added 19 additional protocols directly into `crates/kx-shell-core/src/` without creating `kx-protocols` and without recording an ADR justifying why Phase 4 protocols remained in `kx-shell-core`.
- **Violation of Rule 03 (Function Budgets):** Four functions exceed the strict 60-line ceiling:
  - `crates/kx-config/src/load.rs:244` (`semantic_validation`): 73 lines
  - `crates/kx-shell-core/src/drm/output.rs:394` (`scan_connected_connectors`): 65 lines
  - `crates/kx-shell-core/src/drm/output.rs:330` (`initialize_output_entry`): 63 lines
  - `crates/kx-shell-core/src/state.rs:145` (`KynexState::new_with_output`): 61 lines

### C. Buffers and Sync: PASS WITH NOTES
- **Implementation:** `zwp_linux_dmabuf_v1` and `wp_linux_drm_syncobj_manager_v1` are functional. Dimensions and plane counts are validated with checked arithmetic (`MAX_DMABUF_DIMENSION = 16_384`).
- **Shortcomings:** Advertises only `Modifier::Linear` for ARGB8888, XRGB8888, ABGR8888, XBGR8888. Feedback tranches contain only default device tranche; zero render or scanout tranches exist. Direct scanout is hardcoded disabled in `drm/output.rs:198` (`ALLOW_DIRECT_SCANOUT` absent).

### D. Scaling and Cursors: PASS WITH NOTES
- **Implementation:** `wp_viewporter` and `wp_cursor_shape_v1` correctly enforce error handling (`ViewportExists`, `BadSize`, `BadValue`). Cursor shapes map to standard desktop cursor themes.
- **Shortcoming:** `wp_fractional_scale_v1` advertises preferred scale in 120ths, but `update_surface_fractional_scale` is only called inside `new_fractional_scale`. It is never called when an output scale changes or when a window migrates across outputs.

### E. Pointer Constraints: FAIL
- **Defect:** `zwp_pointer_constraints_v1` correctly tracks constraint state, but `maybe_activate_pointer_constraint` is never called on pointer focus change. A client that creates a constraint before acquiring focus will never have its constraint activated when focus moves to it.

### F. Input Method & Text Input: FAIL
- **Defect:** `zwp_input_method_v2` is gated by `is_input_method_allowed`, but due to privilege conflation in `client.rs:197`, any process allowed in `layer_shell_allow` is granted `input_method` access. This allows rogue UI panels to bind input method and intercept all system keystrokes.
- **Implementation:** `zwp_text_input_v3` correctly synchronizes focus enter/leave in `SeatHandler::focus_changed`. Surrounding text is capped at 4096 bytes.

### G. Session Lock (Highest Risk): FAIL
- **CRITICAL VULNERABILITY:** `crates/kx-shell-core/src/session_lock.rs` implements `ext_session_lock_manager_v1`. However, `is_session_locked` and `lock_surfaces` are never wired into the frame rendering loops (`nested/event.rs:328-340`, `drm/output.rs:200-205`) or input handling (`nested/event.rs:214-225`).
- While locked:
  1. Desktop windows and server-side decorations continue to render normally.
  2. Clicking on an application window raises it and gives it keyboard focus.
  3. Lock surfaces are never composited to the screen.
- The claim in the author's report that "ext_session_lock_v1 renders only lock surfaces and provides fail-closed screen blanking on locker crash" is **completely untrue**.

### H. Telemetry & Misc Protocols: FAIL
- **Defects:**
  1. `ext_idle_notifier_v1` creates a separate Calloop event loop (`idle_loop`) that is never registered with or dispatched by the compositor event loop. User activity never hooks `notify_idle_activity`. Timers will never fire.
  2. `zwp_idle_inhibit_manager_v1` is initialized without a `can_view` filter (unrestricted public global despite docs claiming Restricted), and `self.idle_inhibitors` has no per-client bounds (anti-DoS violation).
  3. `wp_presentation` feedback is omitted from the DRM hardware backend (`drm/output.rs`).
- **Implementation:** `wp_single_pixel_buffer_v1`, `wp_content_type_v1`, `wp_tearing_control_v1`, and `wp_alpha_modifier_v1` correctly validate parameters and reject duplicates with protocol errors.

### I. Privilege Classification: FAIL
- **Defect:** While `docs/SECURITY.md` documents a 3-tier privilege hierarchy (Public, Restricted, Privileged), in `crates/kx-shell-core/src/client.rs:195-202`, all four privileges (`is_layer_shell_allowed`, `is_input_method_allowed`, `is_session_lock_allowed`, `is_foreign_toplevel_allowed`) are assigned the same boolean evaluated against `layer_shell_allow`. This conflates all system privileges into one flag, directly violating Rule 05.

### J. Frame Loop & Performance: PASS WITH NOTES
- **Implementation:** Event-driven, damage-tracked rendering. Zero per-frame allocations in hot paths. Fault-injection guard works.
- **Defect:** DRM output pipeline misses `post_presentation_feedback`.

### K. Test Integrity: PASS WITH NOTES
- **Implementation:** Dedicated headless integration tests exist for all 19 protocols. Zero tests deleted or weakened. Zero panics in tests.
- **Shortcomings:** `tests/session_lock_client.rs` only asserts that the state flag `is_session_locked` is toggled; it never tests that rendering is blocked or that inputs are rejected. `tests/idle_notify_client.rs` manually calls `dispatch_idle_timers` and `notify_idle_activity`, masking the fact that the compositor never executes them.

### L. Docs and ADRs: FAIL
- **Documentation Discrepancies:**
  1. Multiple claims in `docs/reviews/phase-4-review.md` and `docs/STATE.md` are unproven or false (detailed in Section 6).
  2. No ADR was recorded for retaining Phase 4 protocols in `kx-shell-core`.
  3. `docs/THIRD_PARTY_NOTICES.md` is present but has not been updated since Phase 3.

### M. Exit Criteria: FAIL
- `docs/ROADMAP.md` Phase 4 exit criteria mandate verification that games, browsers (Firefox/Chromium), Electron apps, GTK and Qt apps, video players, and fractional scaling behave correctly. The author provided only a nested terminal run and automated protocol unit tests. Interactive multi-app stability remains unverified.

### N. Scope and IP: PASS
- Zero Phase 5 code scaffolded. Zero GPL/LGPL/AGPL code copied. All Rust files have proprietary headers. All third-party dependencies are permissively licensed.

---

## 5. Exit-Criteria Assessment Table

| Phase 4 Exit Criterion (ROADMAP.md) | Audit Status | Evidence Type | Notes / Missing Evidence |
|:---|:---:|:---:|:---|
| **`linux-dmabuf` (+feedback) & explicit sync** | **VERIFIED** | Integration Tests | `dmabuf_client.rs`, `drm_syncobj_client.rs` pass. Linear modifier only. |
| **`fractional-scale` & `viewporter`** | **PARTIALLY** | Integration Tests | Protocol errors pass. Output scale change/migration updates missing. |
| **`cursor-shape`, constraints, relative pointer** | **PARTIALLY** | Integration Tests | Relative pointer passes. Constraint focus re-activation missing. |
| **IME (`text-input` / `input-method`)** | **PARTIALLY** | Integration Tests | Text input passes. Input method privilege conflated with layer shell. |
| **`primary-selection` & clipboard gating** | **VERIFIED** | Integration Tests | Keyboard focus gating verified in `primary_selection_client.rs`. |
| **`idle-inhibit` & `idle-notify`** | **PARTIALLY** | Integration Tests | Inhibit works. Idle notify Calloop loop disconnected from main loop. |
| **`ext-session-lock`** | **NOT MET** | Source Audit | Session lock completely disconnected from rendering and input loops. |
| **`foreign-toplevel` list** | **VERIFIED** | Integration Tests | Restricted visibility and title length caps verified in tests. |
| **Presentation time & feedback** | **PARTIALLY** | Integration Tests | Nested mode verified. DRM hardware output pipeline missing feedback. |
| **Content-type, tearing, single-pixel, alpha** | **VERIFIED** | Integration Tests | Parameter checking and duplicate attachment rejection verified. |
| **Desktop Apps: Firefox / Chromium** | **PENDING MANUAL** | Human-Reported | Author claimed Firefox ran, but no automated or repeatable evidence exists. |
| **Desktop Apps: Electron apps** | **PENDING MANUAL** | None | No tests or execution logs provided. |
| **Desktop Apps: GTK and Qt apps** | **PENDING MANUAL** | Human-Reported | Author claimed GNOME Calculator ran; Qt unverified. |
| **Video players (mpv) & 3D Games** | **PENDING MANUAL** | None | No test logs or manual evidence for video playback or 3D gaming. |
| **HiDPI + Fractional Scaling Interactive** | **PENDING MANUAL** | None | Protocol reporting verified; visual scaling across mixed DPI unverified. |
| **Zero Panics in Runtime Code** | **VERIFIED** | Static Audit & CI | Workspace passes with zero `unwrap`/`expect`/`panic!` in runtime code. |

---

## 6. False and Unproven Claims in Completion Report

The author's report (`docs/reviews/phase-4-review.md`) contains several assertions that are contradicted by the source code:

1. **Claim:** *"ext_session_lock_manager_v1 renders only lock surfaces... fail-closed screen blanking on disconnect."* (Review Sections 2, 3, 5)  
   **Fact:** **FALSE.** `is_session_locked` and `lock_surfaces` are never referenced in `nested/event.rs` or `drm/output.rs`. Normal application windows continue to render and receive pointer focus clicks while locked. Lock surfaces are never displayed.
2. **Claim:** *"Authorized exclusively via peer PID verification (SO_PEERCRED), symlink canonicalization against /proc/<pid>/exe, root ownership validation, and path comparison against [privilege] allow-lists."* (Review Section 5)  
   **Fact:** **FALSE.** `PrivilegeConfig` contains only `layer_shell_allow`. All privileges (`is_layer_shell_allowed`, `is_input_method_allowed`, `is_session_lock_allowed`, `is_foreign_toplevel_allowed`) are set to the same boolean evaluated against `layer_shell_allow`. No independent allow-lists exist for session lock, input method, or foreign toplevel.
3. **Claim:** *"wp_fractional_scale_manager_v1 dynamically updates surfaces via update_surface_fractional_scale upon output migration."* (Review Section 3 Table)  
   **Fact:** **FALSE.** `update_surface_fractional_scale` is only invoked in `new_fractional_scale` when the protocol object is created. It is never called during output migration or output scale reconfiguration.
4. **Claim:** *"Strict File & Function Budgets: Functions $\le 60$ lines."* (Review Section 2 Table)  
   **Fact:** **FALSE.** Four functions exceed 60 lines: `semantic_validation` (73 lines), `scan_connected_connectors` (65 lines), `initialize_output_entry` (63 lines), and `KynexState::new_with_output` (61 lines).
5. **Claim:** *"Unsafe Policy: #![forbid(unsafe_code)] enforced across all workspace crates."* (Review Section 2 Table)  
   **Fact:** **FALSE.** `crates/kx-shell-core/Cargo.toml:36` configures `unsafe_code = "deny"`, not `forbid`, to allow `src/drm/egl.rs` per ADR-0020.
6. **Claim:** *"18 standard and extension Wayland protocols implemented."* (Review Section 1)  
   **Fact:** **DISCREPANCY.** Exactly 19 protocols are implemented; the author's prose states 18 while its table lists 19.

---

## 7. Prioritized Fix Order

### Must-Do Before Phase 5:
1. **Fix Session Lock (SEC-01):** Wire `is_session_locked` and `lock_surfaces` into `nested/event.rs` and `drm/output.rs`. When locked, render strictly lock surfaces (or solid black if none exist). Suppress all application window rendering and input events. Ensure disconnect fails closed to a black screen.
2. **Deconflate Privilege Configuration (SEC-02):** Add `session_lock_allow`, `input_method_allow`, and `foreign_toplevel_allow` to `PrivilegeConfig` in `kx-config`. Update `client.rs` to evaluate each privilege independently against its dedicated allow-list.
3. **Connect Idle Notifier Event Loop (BUG-03):** Register `IdleNotifierState` timers on the main Calloop event loop handle. Hook user activity in `nested/event.rs` and `drm/mod.rs` to invoke `notify_idle_activity`.
4. **Wire DRM Presentation Feedback (BUG-04):** Invoke `post_presentation_feedback` in `DrmOutputPipeline::render_frame` upon page flip completion.
5. **Activate Pointer Constraints on Focus (BUG-05):** Call `maybe_activate_pointer_constraint` whenever a window or surface receives pointer focus.
6. **Update Fractional Scale on Output Migration (BUG-06):** Call `update_surface_fractional_scale` when a surface moves to an output with a different scale or when output mode/scale changes.
7. **Refactor Oversized Functions (ARCH-09):** Split functions exceeding 60 lines (`semantic_validation`, `scan_connected_connectors`, `initialize_output_entry`, `KynexState::new_with_output`) to satisfy Rule 03.
8. **Clarify Missing Roadmap Protocols (SPEC-08):** Add an ADR formally deferring `tablet`, `touch`, `gestures`, and `keyboard-shortcuts-inhibit` to Phase 6 or 8, or implement them before declaring Phase 4 closed.

### Can Wait for Phase 6 or Hardening:
- Dynamic DRM format modifier enumeration and non-linear (tiled/compressed) modifier advertisement.
- Direct scanout enablement for fullscreen surfaces (`FrameFlags::ALLOW_DIRECT_SCANOUT`).
- Multiple feedback tranches (render and scanout tranches) in `linux-dmabuf`.

---

## 8. Manual-Test Verification Checklist for User

Since interactive desktop applications cannot be verified headlessly, execute the following manual tests on the nested compositor (`cargo run --bin kynex-comp`):

- [ ] **Firefox / Chromium:** Launch Firefox (`MOZ_ENABLE_WAYLAND=1 firefox`) and Chromium (`chromium --ozone-platform=wayland`). Verify window creation, tab dragging, smooth scrolling, and video playback without stutter or protocol disconnects.
- [ ] **Electron Application:** Launch VS Code (`code --ozone-platform=wayland`) or Slack. Verify dropdown menus, window resizing, and clipboard copy/paste.
- [ ] **GTK4 / Adwaita:** Launch a modern GTK4 application (e.g. `gnome-calculator` or `gnome-text-editor`). Verify client-side decorations, fractional scaling rendering crispness, and popup positioning.
- [ ] **Qt5 / Qt6 Application:** Launch a Qt application (e.g. `vlc` or `kwrite` with `QT_QPA_PLATFORM=wayland`). Verify menus and text input.
- [ ] **Video Player (mpv):** Run `mpv --vo=dmabuf-wayland <video-file>` or `mpv --vo=gpu-next --gpu-context=wayland <video-file>`. Verify presentation-time frame pacing and zero dropped vblanks.
- [ ] **3D Game / OpenGL Demo:** Run `vkcube` or `glxgears` via Wayland. Test pointer lock and confinement (FPS controls), and verify tearing-control behavior if enabled.
- [ ] **IME (fcitx5 or ibus):** Configure `fcitx5` or `ibus`. Verify that candidates appear at cursor coordinates and preedit text commits cleanly.
- [ ] **Fractional Scaling:** Set output scale to `1.25` or `1.5` in configuration or resize nested window. Verify that Wayland clients utilizing `wp_fractional_scale_v1` render at crisp native resolution without bilinear blur.
- [ ] **DRM Backend on VM / Hardware:** Run `kynex-comp --backend drm` on a virtual console (TTY) in a QEMU/KVM VM with `virtio-gpu`. Verify multi-monitor hotplug (if multi-display VM), emergency exit key (`Ctrl+Shift+Escape`), and clean VT switching (`Ctrl+Alt+F1` / `Ctrl+Alt+F2`).

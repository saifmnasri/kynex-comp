# KynexComp Roadmap

Work only on the **current phase** (see `STATE.md`). A phase is finished only when every exit criterion is met
and `just check` is green. Do not start the next phase early.

## Phase 0 — Foundation
Workspace, lint/CI gate, logging, panic hook, config skeleton, `docs/` baseline.
**Exit:** `just check` green on a clean clone; `kx-common` and `kx-config` have tests; `Cargo.lock` committed.

## Phase 1 — Nested compositor (winit backend)
Pin Smithay. Event loop (calloop), Wayland socket, `wl_compositor`, `wl_shm`, `xdg_shell`, `wl_seat`, `wl_output`,
`wl_data_device`. Render clients inside a winit window with damage tracking. Basic input and focus. Frame callbacks.
**Exit:** `foot` (or any simple Wayland terminal) and a browser window run nested; protocol tests with a headless
client pass; a hostile-client test suite exists; no panics under a 10-minute stress script.

## Phase 2 — Real hardware backend — COMPLETE
libseat session, udev, DRM/KMS with plane-aware compositor, GBM/EGL GLES renderer, libinput, VT switching,
multi-monitor, hotplug, **emergency-exit key first**, systemd notify + watchdog skeleton, frame telemetry, Criterion baseline.
**Exit:** boots to a working session on a VM (virtio-gpu) and on one real laptop; unplug/replug monitor survives;
suspend/resume survives; no root required; Rule 00 budget verified ($\le 4$ ms p99 render CPU time). All exit criteria met.

## Phase 3 — Window management — COMPLETE
Window abstraction, workspaces, floating layout, focus policy, move/resize, popups and grabs, `xdg-decoration`
(server-side decorations), `layer-shell`, `xdg-activation`, fullscreen/maximize/minimize, multi-output windows.
**Exit:** daily-usable floating desktop; layout property tests pass; windows never become unreachable after hotplug. All exit criteria met. Hostile-client test suite passed. Benchmarks established in `benches/BASELINE.md`.

## Phase 4 — Real-app protocol completeness
`linux-dmabuf` (+feedback), explicit sync, `fractional-scale`, `viewporter`, `cursor-shape`, pointer constraints,
relative pointer, IME (`text-input`/`input-method`), primary selection, idle/inhibit, `ext-session-lock`,
foreign-toplevel list, presentation-time, tablet/touch/gestures, tearing control.
**Exit:** games, browsers (Firefox/Chromium), Electron apps, GTK and Qt apps, video players behave correctly; HiDPI + fractional scaling verified.

## Phase 5 — XWayland
Supervised rootless XWayland, X11 window manager (ICCCM/EWMH), clipboard bridge, scaling.
**Exit:** common X11 apps (Steam, Wine apps, legacy toolkits) work; XWayland crash does not kill the session.

## Phase 6 — Shell services and management
Private IPC (versioned), out-of-process panel/launcher/lock/wallpaper clients via layer-shell, screencopy and
portal backend (consent-gated), output management, privilege policy fully enforced, `wp_security_context`.
**Exit:** screen sharing works with consent; privileged globals hidden from unprivileged clients; shell client crash keeps session alive.

## Phase 7 — Effects and animations
`kx-anim` engine, window open/close/move animations, workspace transitions, shadows, rounded corners, blur,
all behind a GPU-budget governor with a "reduce effects" fallback.
**Exit:** effects hold the frame budget on reference low-end hardware; disabling effects restores baseline numbers.

## Phase 8 — Hardening
Fuzzing in CI, soak tests, security review of every privileged path, performance tuning against budgets,
crash-recovery drills (GPU reset, output loss, shell crash), memory/fd leak gates.
**Exit:** 72 h soak clean; fuzzers run N hours with no findings; all Sev-1/Sev-2 closed.

## Phase 9 — KynexOS integration and release
Session files, systemd user units, packaging, signed releases, SBOM, third-party notices, installer integration,
documentation, update/rollback strategy, opt-in crash reporting.
**Exit:** clean install on KynexOS; release checklist (`.agents/rules/09-git-docs-release.md`) fully ticked.

## Standing goals across all phases

Zero panics in runtime code · bounded memory · every privileged feature behind policy · every bug gets a regression test.

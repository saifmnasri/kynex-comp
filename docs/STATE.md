# Project State (handoff file)

> Update at the end of every task. A fresh chat session must be able to continue from this file alone.

**Current phase:** 0 — Foundation (Completed, ready for Phase 1)
**Last updated:** 2026-10-11 — Migration to Fedora Linux 44 verified: pinned toolchain 1.99.0 and system packages installed, stale Darwin Mach-O artifacts cleared, DEV-SETUP updated, full `just check` green (fmt, clippy, 16 tests, deny).

## Done
- **Pre-Step 0:** Toolchain and environment verified, git repo initialized on `main`, `feat/phase-0-foundation` branch created.
- **Step 1:** Pinned Rust toolchain 1.99.0 in `rust-toolchain.toml`, generated `Cargo.lock`.
- **Step 2:** Resolved Clippy doc lint in `kx-common`, added `[licenses.private] ignore = true` in `deny.toml`.
- **Step 3:** Pinned Phase 0 workspace dependencies (`serde`, `toml`, `thiserror`, `tracing`, `tracing-subscriber`, `proptest`, `tempfile`).
- **Step 4:** Implemented `crates/kx-config`: typed structs (`GeneralConfig`, `InputConfig`, `AppearanceConfig`, `Config`), defaults, semantic range validation, 256 KiB size cap, two-pass `config_version` parser.
- **Step 5:** Added unit tests and property-based tests (`proptest`) for `kx-config` covering minimal/full configs, unknown fields, boundary tests at 256 KiB and +1 byte, deeply nested inputs, and invalid versions.
- **Step 6:** Implemented `crates/kx-common` subsystems: `init_logging` (`tracing-subscriber` with `EnvFilter`), `install_panic_hook` with structured crash reports (`crash-{pid}-{timestamp}.log`), Unix 0o700/0o600 permissions at creation, `force_capture()` backtraces, previous hook chaining, and unit/integration tests.
- **Step 7:** Fedora Linux 44 migration verified: system tools (`gcc`, `make`, `just`, Wayland headers) and `rustup` configured, `docs/DEV-SETUP.md` updated with Fedora `dnf` package list, `target/` purged of stale Mach-O binaries.
- **Verification:** All 16 workspace tests pass (`cargo test`), `cargo fmt`, `cargo clippy -D warnings`, `cargo deny check` all green via `just check` on Fedora Linux.

## In progress
- (none — Phase 0 complete)

## Next
1. Human review and merge of `feat/phase-0-foundation` into `main`.
2. Phase 1 — Minimal Nested Compositor: choose & pin Smithay version, scaffold `kx-backend`, `kx-render`, `kx-shell-core`, and `kynex-comp` binary with winit nested backend.

## Known issues
- `cargo deny check` warns on duplicate `syn` versions (v2.0.119 pulled by `tracing-attributes`, v3.0.7 pulled by `serde_derive`/`thiserror-impl`), but passes advisory, ban, license, and source checks.

## Open questions for the human
- Reference development machine and test hardware list (CPU/GPU, laptop model).
- Final license text / EULA (needs a lawyer).
- Smithay version to pin for Phase 1 (evaluate latest stable on crates.io).

## Verify commands
- `just check`
- `cargo test --workspace --all-features --locked`
- `just run-nested` (from Phase 1)

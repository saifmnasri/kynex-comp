# Development Setup

## System packages (names vary by distro)

Debian/Ubuntu-style: `build-essential pkg-config libwayland-dev libxkbcommon-dev libinput-dev libudev-dev libseat-dev libgbm-dev libegl-dev libgles-dev libdrm-dev libdisplay-info-dev libpixman-1-dev`
Arch-style: `base-devel pkgconf wayland libxkbcommon libinput systemd-libs seatd mesa libdrm libdisplay-info pixman`
Test clients: `foot` (terminal), `weston-info`, `wev`, `wl-clipboard`, a browser.

## Rust tools

```
rustup default stable
cargo install --locked just cargo-nextest cargo-deny cargo-audit cargo-llvm-cov cargo-fuzz cargo-about cargo-cyclonedx
cargo generate-lockfile        # once, then commit Cargo.lock
just check
```

## Running during development

- **Always develop nested first:** `just run-nested` runs Kynex as a window inside your current Wayland session (winit backend).
  Set `WAYLAND_DISPLAY` to the socket name Kynex prints, then start clients with it.
- Move to the DRM backend only in Phase 2, **inside a VM first** (QEMU with virtio-gpu), then on real hardware from a spare TTY.
- Keep a second device or SSH session available. If a TTY session hangs: `ssh` in and `pkill kynex-comp`, or switch VT with `Ctrl+Alt+F3`.

## Emergency exit (implemented in Phase 2, before the first DRM test)

Hard-coded combination: **Ctrl+Alt+Shift+Q** (quits Kynex cleanly even with a broken config). Document any change here.

## Log locations

- Logs: stderr/journal (`journalctl --user -u kynex`); crash reports: `$XDG_STATE_HOME/kynex/crash/`.
- Verbosity: `RUST_LOG=kynex=debug,smithay=info`.

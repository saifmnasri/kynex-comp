set shell := ["bash", "-euo", "pipefail", "-c"]

default: check

# The gate. Run after every change.
check: fmt-check lint test deny

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

test:
    if command -v cargo-nextest >/dev/null 2>&1; then cargo nextest run --workspace --all-features --locked; else cargo test --workspace --all-features --locked; fi

deny:
    cargo deny check

audit:
    cargo audit

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

bench:
    cargo bench --workspace --locked

lock:
    cargo generate-lockfile

# Available from Phase 1: runs Kynex nested inside your current Wayland session.
run-nested:
    cargo run -p kynex-comp --locked -- --backend winit

ci: check doc audit

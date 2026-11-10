set shell := ["bash", "-c"]

default:
    @just --list

check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo check --workspace

test:
    cargo test --workspace

# Includes tests that need real YouTube Music credentials and network access.
test-all:
    cargo test --workspace -- --include-ignored

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Run the legacy TUI client; the daemon entrypoint arrives with #1.
dev:
    cargo run -p ytermusic

# Build/serve the web UI; arrives with #14.
web:
    @echo "web UI not implemented yet (see #14)"
    @exit 1

# Smoke-test a running daemon; arrives with #10.
smoke:
    ./scripts/smoke.sh

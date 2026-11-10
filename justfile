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

# Run the daemon with the default bind/port.
dev:
    cargo run -p ytermusicd -- serve

# Build/serve the web UI; arrives with #14.
web:
    @echo "web UI not implemented yet (see #14)"
    @exit 1

# Smoke-test a running daemon; search/SSE checks require #12/#13.
smoke:
    ./scripts/smoke.sh

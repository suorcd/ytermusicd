set shell := ["bash", "-c"]

default:
    @just --list

check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo check --workspace

test:
    cargo test --workspace
    @if [ -d crates/ytermusicd-web/node_modules ]; then cd crates/ytermusicd-web && npm test; else echo "web tests skipped (run 'just web-install')"; fi

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

# Install web UI dependencies.
web-install:
    cd crates/ytermusicd-web && npm ci

# Type-check and build the web UI.
web-check:
    cd crates/ytermusicd-web && npm run check && npm run build

# Run the Vite dev server; /api is proxied to the daemon.
web:
    cd crates/ytermusicd-web && npm run dev

# Smoke-test a running daemon; search/SSE checks require #12/#13.
smoke:
    ./scripts/smoke.sh

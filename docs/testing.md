# Testing and local development

This project uses a Nix dev shell for reproducible tooling. If Nix is not
available, install the tools listed in the README manually.

```sh
nix develop
```

## Common commands

| Command | What it does |
| --- | --- |
| `just check` | `cargo fmt --check`, clippy with `-D warnings`, and `cargo check` |
| `just test` | Unit tests that need no credentials, network, audio device or DBus |
| `just test-all` | Includes ignored tests that need real YouTube Music credentials/network |
| `just fmt` | Format the workspace |
| `just clippy` | Clippy only |
| `just dev` | Run the daemon with the default bind/port |
| `just web-install` | Install web UI dependencies (`npm ci`) |
| `just web` | Run the Vite dev server with an `/api` proxy |
| `just web-check` | Type-check and build the web UI |
| `just smoke` | Smoke-test a running daemon (`scripts/smoke.sh`) |

`just` is provided by the Nix dev shell and listed in the README prerequisites
for non-Nix setups.

## Test layers

1. **Unit tests** live next to the code (`#[cfg(test)]`). They cover parsing,
   string helpers, queue/state transitions, persistence and other pure logic.
2. **Offline integration tests** use fixtures and mock implementations for the
   YouTube client, audio sink, downloader and database. They must not require
   cookies, a network connection, an audio device or DBus.
3. **Credentialed tests** are marked `#[ignore]`. The `ytpapi2` tests are the
   current example: they read a real `headers.txt` and call YouTube Music.
4. **Web tests** use Vitest and Testing Library in `crates/ytermusicd-web`.
5. **Smoke tests** run against a real daemon and are scripted in
   `scripts/smoke.sh`.

### Credentialed tests

By default, credentialed tests are skipped:

```sh
cargo test --workspace
```

Run them explicitly with a headers file:

```sh
YTERMUSICD_HEADERS="$HOME/.config/ytermusicd/headers.txt" \
  cargo test -p ytpapi2 -- --include-ignored
```

Never commit `headers.txt`, `account_id.txt` or derived cookies. The repository
root `headers.txt` is already gitignored.

### Fixtures and mocks

Current offline coverage is in `crates/ytpapi2`, `crates/ytermusicd-core`,
`crates/ytermusicd` and `crates/ytermusicd-web`:

- `string_utils` tests cover cookie/header parsing helpers.
- `json_extractor` tests cover search, playlist, continuation, album and
  nested video-ID parsing with inline JSON fixtures.
- `ytermusicd-core` tests cover auth/config/paths, queue transitions,
  repeat/shuffle restoration, download-failure handling, playback position and
  event-bus replay using a mock audio sink.
- `ytermusicd` integration tests start the router on an ephemeral port and
  cover every REST endpoint, SSE replay, download retry, static serving and
  upstream error envelopes.
- Web tests cover the API client, event stream, store, keymap, track table,
  queue, now playing, search, feedback banners and the app shell.

The test harness provides:

- a mock YouTube client and no-op download control for API tests;
- a mock audio sink (no device required);
- an in-memory event bus and a temp directory for persistence tests;
- an API/SSE test client;
- Vitest component tests with a fake EventSource.

## Regression-test policy

Every bug fix or behavior change must include a test that demonstrates the gap:

1. Add the test first and confirm it **fails on the current code**.
2. Implement the fix and confirm the test **passes**.
3. Keep the test as a regression guard.

If a deterministic test is impractical (real audio device, DBus/MPRIS, live
YouTube network), document the manual reproduction steps and add the closest
automated smoke test instead. Pull requests without this evidence are
incomplete.

## CI

Every push and pull request runs `.github/workflows/ci.yml`:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check --workspace --all-targets`
- `cargo test --workspace`

The `web` job detects `crates/ytermusicd-web/package.json` and builds the web UI
once that crate exists. The release workflow builds `ytermusicd` for Linux,
Windows and macOS, bundles web assets when present, publishes SHA-256
checksums, and can be dry-run with `workflow_dispatch`.

## Smoke tests

Start the daemon:

```sh
cargo run -p ytermusicd -- serve
```

Then, in another shell:

```sh
just smoke
# or
YTERMUSICD_URL=http://127.0.0.1:8080 ./scripts/smoke.sh
```

The script checks `/api/status`, search, at least one SSE event, and MPRIS via
`playerctl` when available. The daemon must have valid headers for the search
check; without them it returns 502 and the smoke test fails. Run it against a
daemon started with temporary state, or skip the search check by inspecting the
individual curl commands in [docs/api.md](api.md).

## Current gaps

The daemon, REST/SSE API, web UI, MPRIS and systemd/Nix packaging exist. The
MCP proxy is still in progress; `ytermusicd mcp` exits with a "not implemented"
error until #21.

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
| `just dev` | Run the current client (TUI until the daemon exists) |
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
4. **Smoke tests** run against a real daemon and are scripted in
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

Current offline coverage is in `crates/ytpapi2`:

- `string_utils` tests cover cookie/header parsing helpers.
- `json_extractor` tests cover search, playlist, continuation, album and
  nested video-ID parsing with inline JSON fixtures.

As the daemon and web UI land, extend this with:

- temp XDG config/cache/state directories per test;
- a mock YouTube client with search/playlist fixtures;
- a mock audio sink (no device required);
- a fake downloader or `yt-dlp` shim;
- an in-memory/temp database;
- an API/SSE test client and an MCP mock daemon;
- Vitest component tests for the web UI.

## Regression-test policy

Every bug fix or behavior change must include a test that demonstrates the gap:

1. Add the test first and confirm it **fails on the current code**.
2. Implement the fix and confirm the test **passes**.
3. Keep the test as a regression guard.

If a deterministic test is impractical (real audio device, DBus/MPRIS, live
YouTube network), document the manual reproduction steps and add the closest
automated smoke test instead. Pull requests without this evidence are
incomplete.

## Smoke tests

Start the daemon (once #10 lands):

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
`playerctl` when available.

## Current gaps

The daemon, REST/SSE API, web UI and MCP proxy do not exist yet. The `just dev`,
`just web` and `just smoke` recipes are placeholders until issues #1, #10, #14
and #21 land.

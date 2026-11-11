# ytermusicd

> **Work in progress.** `ytermusicd` is a headless daemon fork of
> [ytermusic](https://github.com/ccgauche/ytermusic), adding a web UI, MPRIS and
> MCP. The daemon skeleton and `/api/status` exist; playback/queue, the full
> API, web UI and MCP are in progress.

[Build plan](https://sb.ser.ink/llm-wiki/concepts/ytermusicd-build-plan) ·
[Roadmap](https://github.com/suorcd/ytermusicd/issues/25) ·
[Issues](https://github.com/suorcd/ytermusicd/issues)

## What it is

`ytermusicd` keeps YouTube Music playback, queue and downloads in a long-running
daemon. Clients are thin:

- **Web UI** (planned) — library, search, queue and now-playing views.
- **MCP** (planned) — a stdio proxy exposing search/playback tools to assistants.
- **MPRIS** — desktop media controls (`playerctl`, shell media widgets).
- **Legacy TUI** — the existing `ytermusic` client remains available.

The API is the contract. State and DTOs live in the daemon/API crates; the web
UI, MCP proxy and any future TUI are clients.

## Status

| Area | State |
| --- | --- |
| Workspace, daemon skeleton, Nix dev shell | ✅ |
| Local dev/test workflow (`just`, offline tests) | ✅ |
| `ytermusicd serve` + `GET /api/status` | 🚧 skeleton |
| Auth/config/path extraction | Planned |
| Playback, queue, downloads, event bus, MPRIS | Planned |
| Full REST + SSE API | Planned |
| Web UI | Planned |
| MCP proxy | Planned |
| systemd/Nix package/first release | Planned |

See the [roadmap](https://github.com/suorcd/ytermusicd/issues/25) for the
ordered backlog.

## Quickstart

Requirements: Nix with flakes (recommended), or Rust 1.99+, Node 22+ and the
native libraries listed in the [dev shell](flake.nix).

```sh
git clone git@github.com:suorcd/ytermusicd.git
cd ytermusicd
nix develop

cargo run -p ytermusicd -- --help
cargo run -p ytermusicd -- serve
```

In another shell:

```sh
curl http://127.0.0.1:8080/api/status
```

`serve` binds `127.0.0.1:8080` by default. Override it with `--bind`, `--port`,
`YTERMUSICD_BIND` or `YTERMUSICD_PORT`. It shuts down gracefully on
SIGINT/SIGTERM.

> The status payload is currently a skeleton. Auth, playback and queue values
> are filled in by issues #6 and #7.

## Authentication

`ytermusicd` uses YouTube Music browser cookies. Create a `headers.txt` with:

```
Cookie: <cookie>
User-Agent: Mozilla/5.0 ...
```

1. Open https://music.youtube.com in your browser.
2. Open developer tools (F12).
3. In the Network tab, open the page request for `/`.
4. Copy the `Cookie` header (Firefox: enable **Raw** first).
5. Copy the `User-Agent` header.
6. Save both to `headers.txt` in the config directory.

The current client checks `./headers.txt` and the XDG config directory. The
daemon resolves headers in this order:

1. `--headers <path>` / `YTERMUSICD_HEADERS`
2. `$XDG_CONFIG_HOME/ytermusicd/headers.txt`
3. `$XDG_CONFIG_HOME/ytermusic/headers.txt` (legacy, with a warning)

It validates the cookie and `SAPISID` at startup and exits with an actionable
error instead of prompting on stdin. The legacy TUI shares the validation and
still checks `./headers.txt` first.

Brand accounts: copy the account ID from `myaccount.google.com` (the number
after `/b/`) into `account_id.txt` next to `headers.txt`.

**Never commit `headers.txt`, `account_id.txt`, cookies or logs containing
them.** They are gitignored.

## Configuration

The daemon reads `$XDG_CONFIG_HOME/ytermusicd/config.toml` (usually
`~/.config/ytermusicd/config.toml`). See
[config.example.toml](config.example.toml) for the available fields and
defaults. CLI flags and environment variables override the config file:

| Option | Environment |
| --- | --- |
| `--bind` | `YTERMUSICD_BIND` |
| `--port` | `YTERMUSICD_PORT` |
| `--headers` | `YTERMUSICD_HEADERS` |
| `--config` | `YTERMUSICD_CONFIG` |

`config.toml` parse errors, invalid values and malformed headers are reported
with the file path; they are never silently replaced with defaults.

## Development

```sh
nix develop
just check
just test
```

- [docs/testing.md](docs/testing.md) — local workflow, fixtures, regression policy
- [CONTRIBUTING.md](CONTRIBUTING.md) — contribution rules
- [AGENTS.md](AGENTS.md) — worktree-per-issue workflow for agents
- [docs/architecture.md](docs/architecture.md) — daemon/API/client design
- [docs/decisions](docs/decisions/) — architecture decision records
- [0004 — upstream strategy](docs/decisions/0004-upstream-strategy.md) — fork-first with selective upstreaming

### Workspace layout

- `crates/ytermusicd` — daemon binary (`serve`, `mcp`; in progress)
- `crates/ytermusicd-core` — headless paths, config, auth, playback/queue and event bus
- `crates/ytermusic` — legacy TUI client
- `crates/ytpapi2` — YouTube Music API client
- `crates/player` — audio playback
- `crates/database` — local download database
- `crates/download-manager` — download queue
- `crates/common-structs` — shared status types

Planned: `crates/ytermusicd-api` (shared DTOs) and `crates/ytermusicd-web`.

## API

The v1 contract is defined in the build plan. Current endpoint:

- `GET /api/status`

Planned: playlists, search, queue, playback commands and SSE. See
[docs/architecture.md](docs/architecture.md) and the roadmap.

## MCP

`ytermusicd mcp` will be a stdio proxy to the daemon (issue #21). It is not
implemented yet.

## Legacy TUI

The upstream TUI is still built as `ytermusic`. See
[docs/legacy-tui.md](docs/legacy-tui.md) for its features, installation and
keybindings.

## License and attribution

Apache-2.0. This project is a fork of
[ccgauche/ytermusic](https://github.com/ccgauche/ytermusic); see
[NOTICE](NOTICE), [LICENCE](LICENCE) and
[docs/upstream-changelog.md](docs/upstream-changelog.md).

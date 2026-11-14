# ytermusicd

> **v0.1.0.** `ytermusicd` is a headless daemon fork of
> [ytermusic](https://github.com/ccgauche/ytermusic), adding a web UI, MPRIS and
> MCP. The daemon, REST/SSE API, web UI, MPRIS, MCP and packaging work today.
> Playlist editing is post-v1 ([#51](https://github.com/suorcd/ytermusicd/issues/51)).

[Build plan](https://sb.ser.ink/llm-wiki/concepts/ytermusicd-build-plan) ·
[Roadmap](https://github.com/suorcd/ytermusicd/issues/25) ·
[Issues](https://github.com/suorcd/ytermusicd/issues)

## What it is

`ytermusicd` keeps YouTube Music playback, queue and downloads in a long-running
daemon. Clients are thin:

- **Web UI** — library, search, queue, now playing and vim-style keys.
- **MCP** — a stdio proxy exposing search/playback tools to assistants.
- **MPRIS** — desktop media controls (`playerctl`, shell media widgets).
- **Legacy TUI** — the existing `ytermusic` client remains available.

The API is the contract. State and DTOs live in the daemon/API crates; the web
UI, MCP proxy and any future TUI are clients.

## Status

| Area | State |
| --- | --- |
| Workspace, daemon skeleton, Nix dev shell | ✅ |
| Local dev/test workflow (`just`, offline tests) | ✅ |
| Auth/config/path extraction | ✅ |
| Playback, queue, downloads, event bus, MPRIS | ✅ |
| Full REST + SSE API | ✅ |
| Web UI (library, search, queue, now playing, vim keys) | ✅ |
| systemd unit, Nix package/module | ✅ |
| MCP proxy (13 tools) | ✅ |
| v0.1.0 release | ✅ |

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

Or run the packaged daemon directly:

```sh
nix run .#ytermusicd -- serve
```

In another shell:

```sh
curl http://127.0.0.1:8080/api/status
```

`serve` binds `127.0.0.1:8080` by default. Override it with `--bind`, `--port`,
`YTERMUSICD_BIND` or `YTERMUSICD_PORT`. It shuts down gracefully on
SIGINT/SIGTERM. Open `http://127.0.0.1:8080` for the web UI. See
[docs/api.md](docs/api.md) for the full endpoint reference, SSE stream and curl
examples.

The repository build embeds the web app. If the UI shows a "not built" page,
run `just web-install && just web-check`, then restart the daemon. For frontend
development, use `just web` and the Vite proxy.

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
| `--url` (`mcp` only) | `YTERMUSICD_URL` |

`config.toml` parse errors, invalid values and malformed headers are reported
with the file path; they are never silently replaced with defaults.

## Development

```sh
nix develop
just check
just test
```

- [docs/testing.md](docs/testing.md) — local workflow, fixtures, regression policy
- [docs/api.md](docs/api.md) — REST/SSE reference, curl examples and reconnect semantics
- [docs/web.md](docs/web.md) — web UI development, keymap and failure states
- [docs/mcp.md](docs/mcp.md) — MCP client setup and the 13 v1 tools
- [docs/release.md](docs/release.md) — release checklist and smoke tests
- [CHANGELOG.md](CHANGELOG.md) — release notes
- [CONTRIBUTING.md](CONTRIBUTING.md) — contribution rules
- [AGENTS.md](AGENTS.md) — worktree-per-issue workflow for agents
- [docs/architecture.md](docs/architecture.md) — daemon/API/client design
- [docs/deployment.md](docs/deployment.md) — systemd user service and environment
- [docs/decisions](docs/decisions/) — architecture decision records
- [0004 — upstream strategy](docs/decisions/0004-upstream-strategy.md) — fork-first with selective upstreaming

### Workspace layout

- `crates/ytermusicd` — daemon binary (`serve`, `mcp`)
- `crates/ytermusicd-core` — headless paths, config, auth, playback/queue and event bus
- `crates/ytermusicd-api` — shared DTOs and generated TypeScript bindings
- `crates/ytermusicd-web` — Svelte 5 web UI (embedded in the daemon)
- `crates/ytermusic` — legacy TUI client
- `crates/ytpapi2` — YouTube Music API client
- `crates/player` — audio playback
- `crates/database` — local download database
- `crates/download-manager` — download queue
- `crates/common-structs` — shared status types

## API

The daemon implements the full v1 REST + SSE contract. Examples:

```sh
curl http://127.0.0.1:8080/api/status
curl http://127.0.0.1:8080/api/playlists
curl 'http://127.0.0.1:8080/api/search?q=civilization'
curl -N http://127.0.0.1:8080/api/events
```

See [docs/api.md](docs/api.md) for every endpoint, request/response shapes,
error handling, SSE event types and `Last-Event-ID` replay. Playing a playlist
or track automatically queues the audio for download; playback starts when the
cached file is ready, and failed downloads can be retried from the API or UI.
The daemon serves the web UI at `/` and the API under `/api`.

## MCP

`ytermusicd mcp` is a stdio proxy to the daemon. It exposes 13 tools:
`search`, `list_playlists`, `get_playlist`, `play_playlist`, `play_track`,
`queue_add`, `now_playing`, `pause`, `resume`, `next`, `previous`, `volume` and
`download_status`.

```sh
ytermusicd mcp
YTERMUSICD_URL=http://127.0.0.1:9000 ytermusicd mcp
```

See [docs/mcp.md](docs/mcp.md) for client configuration, tool arguments and
error behavior. Playlist editing, lyrics and HTTP transport are post-v1.

## Troubleshooting

**The daemon exits with a cookie or `SAPISID` error.** Refresh `headers.txt`.
See [Authentication](#authentication). The daemon never prompts on stdin.

**The API returns 502.** YouTube Music rejected the request. Check the cookie,
the network and the daemon log. The web UI shows the message in a toast.

**The web UI shows "not built".** Run `just web-install && just web-check`.
Then restart the daemon.

**There is no audio.** Check the ALSA or PipeWire device. Run `aplay -l` and
check the daemon log. The first start can wait for a download.

**MPRIS does not appear in `playerctl -l`.** Check the D-Bus session and set
`dbus = true` in `config.toml`. The daemon continues without MPRIS if D-Bus is
not available.

**The port is in use.** Set `--port` or `YTERMUSICD_PORT`.

**A state file is corrupt.** The daemon moves it to `state.json.bad` and starts
with an empty queue. A log line shows the path.

**More logs.** Set `RUST_LOG=ytermusicd=debug`.

## Migrating from ytermusic

The legacy TUI and the daemon use different XDG directories. The daemon reads a
legacy `com/ccgauche/ytermusic/headers.txt` with a warning. It does not import
the TUI playlist file or the TUI queue. Downloaded files are not moved.

To migrate:

1. Copy the cookie headers to `$XDG_CONFIG_HOME/ytermusicd/headers.txt`.
2. Copy downloaded files to `$XDG_CACHE_HOME/ytermusicd/downloads/`.
3. Start the daemon and select a playlist in the web UI.

The legacy `ytermusic` binary still works.

## Legacy TUI

The upstream TUI is still built as `ytermusic`. See
[docs/legacy-tui.md](docs/legacy-tui.md) for its features, installation and
keybindings.

## License and attribution

Apache-2.0. This project is a fork of
[ccgauche/ytermusic](https://github.com/ccgauche/ytermusic); see
[NOTICE](NOTICE), [LICENCE](LICENCE) and
[docs/upstream-changelog.md](docs/upstream-changelog.md).

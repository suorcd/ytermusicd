# Architecture

## Goals

- A single long-running daemon owns playback, queue, downloads and auth state.
- Clients (web UI, MCP proxy, MPRIS, future TUI) are thin and stateless.
- The HTTP/JSON/SSE API is the contract between the daemon and clients.
- The daemon works headless: no terminal, window or interactive prompt required.
- Media controls work through MPRIS; deployment works through systemd/Nix.

## Process model

`ytermusicd serve` starts one process with:

- a multi-threaded Tokio runtime and one HTTP listener;
- one audio sink and one queue service;
- one download manager;
- one event bus;
- one MPRIS/D-Bus client when enabled.

The daemon binds to `127.0.0.1:8080` by default and shuts down gracefully on
SIGINT/SIGTERM. State is owned by the daemon, not by an individual client.

## Components

| Component | Status | Responsibility |
| --- | --- | --- |
| `crates/ytermusicd` | Current | CLI, HTTP/SSE server, MPRIS integration, MCP proxy entrypoint |
| `ytermusicd-core` | Current (#6/#7) | Auth, config, paths, queue/playback and event bus |
| `ytermusicd-api` | Current (#11) | Shared DTOs, event types and generated TypeScript |
| `crates/ytermusicd-web` | Current (#14–#17) | Svelte 5 web UI (embedded at build time) |
| `crates/ytpapi2` | Current | YouTube Music API client |
| `crates/player` | Current | Audio decoding/playback |
| `crates/database` | Current | Local download database |
| `crates/download-manager` | Current | Download queue |
| `crates/common-structs` | Current | Shared status types |
| `crates/ytermusic` | Current | Legacy TUI client |

## Data flow

```
client ──REST──▶ daemon ──command──▶ core service ──▶ event bus
                    ▲                                  │
                    └──────── snapshot ◀───────────────┘
                                                       │
web / MCP ◀────────────── SSE ─────────────────────────┘
                                                       │
                                                     MPRIS
```

Commands are validated, applied to the core state, and then published as typed
events. New SSE clients receive a snapshot followed by incremental events.
Clients render from the snapshot/event stream and never poll for the current
track.

The web UI is a Svelte 5 app. It is built with Vite and embedded into the
daemon with `rust-embed`. The daemon serves it at `/` with a client-side
routing fallback. See [docs/web.md](web.md).

## State and storage

- **Config**: `config.toml` under the XDG config directory.
- **Auth**: `headers.txt` (`Cookie` + `User-Agent`) and optional
  `account_id.txt`; never committed or logged.
- **Cache**: downloaded `<video-id>.mp4` and `<video-id>.json` files plus
  `db.bin`.
- Runtime state: queue, current index, position, volume, shuffle/repeat,
  written atomically after commands, periodically while the daemon runs and on
  shutdown, then restored on startup (invalid state is quarantined as
  `state.json.bad`).
- **Legacy**: `last-playlist.json` from the TUI.

The daemon uses the `com/suorcd/ytermusicd` directories and falls back to the
legacy `com/ccgauche/ytermusic` headers file with a warning. The legacy TUI
keeps its existing cache/config directories.

## Downloads

The daemon owns a `DownloadManager` with `parallel_downloads` workers. Playing
a playlist or track and enqueueing a track records the metadata and adds the
audio to the download queue. Playback waits for the cache file to appear and
starts automatically when the manager emits `Downloaded`; failures skip to the
next track and surface an error event. `/api/status` reports queued/active/
completed/retrying/failed counts, and `/api/downloads/retry` re-queues a failed
track. The web UI and MCP proxy never download directly.

## API

The full contract and curl examples live in [docs/api.md](api.md). DTOs come
from `ytermusicd-api`, which also generates the TypeScript bindings consumed by
the web UI and MCP proxy.

- REST: status, playlists, playlist detail, search, queue CRUD/reorder, play
  playlist/track, pause/resume, next/previous, volume, seek, shuffle, repeat.
- Every command returns the updated `Status` or `Queue`, so clients can render
  without waiting for the event stream.
- SSE: an initial snapshot followed by playback, queue, download, track-ended,
  error and library events. Event ids are monotonic and a 256-event ring
  supports `Last-Event-ID` replay; stale clients get a fresh snapshot.
- Errors use one JSON envelope. The API is loopback-only by default and does not
  enable wildcard CORS.

## Media controls (MPRIS)

On Linux the daemon starts a `souvlaki` MPRIS client on a dedicated thread when
`player.dbus = true` (the default). Clients see the identity `ytermusic` and
display name `ytermusicd`. `playerctl -l` lists it while the daemon runs.

- Supported: play/pause/toggle, next, previous, stop, seek forward/backward by
  the requested amount, set position and set volume.
- Metadata and playback progress come from the event bus, so shell widgets and
  the HTTP API always agree.
- `OpenUri` and `Raise` are explicitly ignored (logged at info/debug level)
  instead of panicking: the daemon has no window to raise and does not accept
  arbitrary URIs.
- If D-Bus is unavailable, startup logs a warning and the daemon continues
  headless; the HTTP API and playback are unaffected. Set `player.dbus = false`
  to skip MPRIS entirely.
- Non-Linux platforms compile a no-op MPRIS stub; no winit window is required.

## MCP

`ytermusicd mcp` is a stdio proxy to the REST API. It uses `rmcp` and holds no
playback state. The proxy has 13 tools: search, playlists, playback transport,
queue add, now playing, volume and download status. It reads the daemon URL from
`--url` or `YTERMUSICD_URL`, applies a 30-second timeout, and exits when stdin
closes. See [docs/mcp.md](mcp.md). Playlist editing, lyrics and HTTP transport
are post-v1.

## Security

- Cookies/SAPISID must never appear in logs or API responses.
- The API binds to loopback unless explicitly configured otherwise.
- The systemd unit runs as a user service with hardening.
- MCP is a local stdio proxy; it holds no playback state.

## Decisions

Initial decisions are recorded in [docs/decisions](decisions/):

- the daemon owns playback/queue state;
- the API is loopback-only by default;
- the web UI uses Svelte + Vite + TypeScript by default.

## Open questions

- v1 playlist editing scope (#19).
- Upstream fork/PR strategy (#5).

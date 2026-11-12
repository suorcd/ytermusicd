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
| `crates/ytermusicd` | Current | CLI, HTTP server, MCP proxy entrypoint |
| `ytermusicd-core` | Current (#6/#7) | Auth, config, paths, queue/playback and event bus; downloads/MPRIS planned |
| `ytermusicd-api` | Current (#11) | Shared DTOs, event types and generated TypeScript |
| `crates/ytermusicd-web` | Planned (#14) | Svelte web UI |
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

## State and storage

- **Config**: `config.toml` under the XDG config directory.
- **Auth**: `headers.txt` (`Cookie` + `User-Agent`) and optional
  `account_id.txt`; never committed or logged.
- **Cache**: downloaded `<video-id>.mp4` and `<video-id>.json` files plus
  `db.bin`.
- Runtime state (#18): queue, current index, position, volume, shuffle/repeat,
  written atomically and restored on startup.
- **Legacy**: `last-playlist.json` from the TUI.

The daemon uses the `com/suorcd/ytermusicd` directories and falls back to the
legacy `com/ccgauche/ytermusic` headers file with a warning. The legacy TUI
keeps its existing cache/config directories.

## API

The v1 contract is documented in the build plan and will be implemented in
#12/#13 using DTOs from `ytermusicd-api` (#11):

- status, playlists, playlist detail, search, queue;
- play playlist/track, enqueue, pause/resume, next/previous, volume;
- SSE events for playback, queue, downloads, errors and library changes.

Errors use one JSON envelope. The API is loopback-only by default and does not
enable wildcard CORS.

## Security

- Cookies/SAPISID must never appear in logs or API responses.
- The API binds to loopback unless explicitly configured otherwise.
- The systemd unit (planned #22) runs as a user service with hardening.
- MCP is a local stdio proxy; it holds no playback state.

## Decisions

Initial decisions are recorded in [docs/decisions](decisions/):

- the daemon owns playback/queue state;
- the API is loopback-only by default;
- the web UI uses Svelte + Vite + TypeScript by default.

## Open questions

- v1 playlist editing scope (#19).
- Upstream fork/PR strategy (#5).

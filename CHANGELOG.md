# Changelog

## 0.1.0 — 2026-11-13

First public release.

### Added

- Add the `ytermusicd` daemon skeleton with `serve` and `mcp` subcommands.
- Add `GET /api/status` with configurable bind/port and graceful shutdown.
- Include auth, playback and queue snapshots in `/api/status`.
- Add `ytermusicd-core` for paths, config, header validation and auth status.
- Add the headless queue/playback state machine, mock audio sink and typed event bus.
- Add shared API DTOs and generated TypeScript bindings in `ytermusicd-api`.
- Persist queue, playback position, volume and shuffle/repeat state.
- Harden downloads with atomic files, parsed progress, retries, cancellation-safe state and a status summary.
- Add an optional `config.toml` and `--headers`/`--config` startup options.
- Add a Nix flake with a reproducible development shell.
- Add the local dev/test workflow (`just`, offline tests, smoke script).
- Add `AGENTS.md`, `CONTRIBUTING.md` and architecture docs.
- Add a systemd user unit and deployment documentation.
- Add a Nix package, NixOS module and home-manager module.
- Add the full REST API, SSE replay and download retry endpoint.
- Wire downloads, persistence and live playback progress into the daemon.
- Add the Svelte 5 web UI with library, search, queue, now playing, vim keys,
  mouse actions, error banners and download retries.
- Add Linux MPRIS media controls.
- Add the `ytermusicd mcp` stdio proxy with 13 tools.
- Add CI, the release workflow with checksums, and offline test suites.

### Known limitations

- Playlist editing is post-v1. See
  [#51](https://github.com/suorcd/ytermusicd/issues/51).
- Lyrics and MCP HTTP transport are post-v1.
- The web UI needs a built bundle inside the daemon package.
- The daemon API trusts local clients. Use a reverse proxy for remote access.

Release history inherited from the upstream project is preserved in
[docs/upstream-changelog.md](docs/upstream-changelog.md).

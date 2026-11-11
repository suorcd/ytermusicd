# Changelog

## Unreleased

- Add the `ytermusicd` daemon skeleton with `serve` and `mcp` subcommands.
- Add `GET /api/status` with configurable bind/port and graceful shutdown.
- Include auth, playback and queue snapshots in `/api/status`.
- Add `ytermusicd-core` for paths, config, header validation and auth status.
- Add the headless queue/playback state machine, mock audio sink and typed event bus.
- Add an optional `config.toml` and `--headers`/`--config` startup options.
- Add a Nix flake with a reproducible development shell.
- Add the local dev/test workflow (`just`, offline tests, smoke script).
- Add `AGENTS.md`, `CONTRIBUTING.md` and architecture docs.

Release history inherited from the upstream project is preserved in
[docs/upstream-changelog.md](docs/upstream-changelog.md).

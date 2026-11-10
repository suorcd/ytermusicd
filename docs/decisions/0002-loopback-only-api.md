# 0002 — The API is loopback-only by default

**Status:** Accepted

**Date:** 2026-11-10

## Context

The daemon can control playback and read the user's YouTube Music library. It
also holds browser cookies. Binding to all interfaces without authentication
would expose both to the local network.

## Decision

`ytermusicd serve` binds to `127.0.0.1:8080` by default. Binding to another
address requires an explicit `--bind`/`YTERMUSICD_BIND` override. CORS is
same-origin by default; wildcard CORS is not enabled.

## Consequences

- Local web UI, MCP proxy and curl smoke tests work without extra auth.
- Remote access requires a reverse proxy with authentication/TLS, documented in
  the deployment guide (`#22`/`#24`).
- The API must never return cookies or raw auth headers.

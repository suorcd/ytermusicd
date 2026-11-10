# 0001 — The daemon owns playback and queue state

**Status:** Accepted

**Date:** 2026-11-10

## Context

The upstream TUI keeps the queue, current index, playback sink, download status
and MPRIS state inside the UI process. The web UI, MCP proxy and future clients
need one authoritative source of truth, and the daemon must keep playing when
no client is attached.

## Decision

`ytermusicd` owns playback, queue, download and auth state. Clients send
commands and subscribe to events; they do not own or mutate state directly.

## Consequences

- The TUI becomes a client of the daemon (or remains a legacy standalone
  client until parity).
- The core service needs a command/event API (`#7`) and a snapshot model.
- SSE and MCP can be thin adapters over the same event stream.
- Tests can run headless with a mock audio sink.

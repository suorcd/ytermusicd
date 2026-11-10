# 0003 — The web UI uses Svelte + Vite + TypeScript

**Status:** Accepted

**Date:** 2026-11-10

## Context

The build plan lists Svelte as the default frontend, with React as an
alternative. The UI is a client of the JSON/SSE API and does not need a
server-side rendering framework.

## Decision

Use Svelte 5 with Vite and TypeScript in `crates/ytermusicd-web`.

## Consequences

- The daemon embeds the production build and serves it at `/` (`#14`).
- Shared API types are generated from `ytermusicd-api` (`#11`).
- The framework decision can be revisited before the scaffold lands, but not
  after UI work starts.

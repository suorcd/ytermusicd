# 0005 — v1 is read-only for playlists

**Status:** Accepted

**Date:** 2026-11-13

## Context

The v1 API and web UI can read playlists and control playback. Playlist editing
is an open question. The `ytpapi2` crate has no mutation methods. A create,
rename, delete or track edit needs the authenticated YouTube Music
`browse/edit_playlist` endpoint.

Playlist mutations have these risks:

- They change the user's YouTube Music account.
- The endpoint is not part of the public API.
- Rate limits and cookie expiry need careful error handling.
- Destructive actions need confirmation and a recovery path.

The v1 release priorities are playback, the API contract, the web UI, MPRIS and
MCP.

## Decision

Version 1 is read-only for playlists. These actions are post-v1:

- create, rename and delete a playlist;
- add, remove and reorder playlist tracks;
- MCP tools for playlist changes.

The follow-up is tracked in [#51](https://github.com/suorcd/ytermusicd/issues/51).

## Consequences

- No v1 API endpoint changes YouTube Music playlists.
- The web UI shows library and playlist content only.
- The MCP proxy has 13 tools and no playlist edit tool.
- The README and web docs state the limitation.
- A later release must add a mutation spike, API DTOs, confirmation UI, tests
  and docs.

## Alternatives

Option B was to implement editing in v1. We rejected it because the mutation
endpoint is undocumented, the work is large and a failure can change user data.

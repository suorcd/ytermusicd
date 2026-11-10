# 0004 — Fork-first with selective upstreaming

**Status:** Accepted

**Date:** 2026-11-10

## Context

`ytermusicd` is a fork of [ccgauche/ytermusic](https://github.com/ccgauche/ytermusic).
The fork adds a daemon, HTTP/SSE API, web UI, MCP proxy and packaging on top of
the upstream TUI. The build plan lists "fork-first vs upstream draft PR" as an
open question.

Some changes are generic and useful upstream (download manager fixes, player
fixes, `ytpapi2` parser/auth fixes, CI/clippy cleanup). The daemon architecture
and web UI are large, opinionated changes that need product decisions and
should not block on upstream review.

## Decision

- **Primary development happens in the fork** (`suorcd/ytermusicd`).
- **Upstream generic, self-contained fixes when practical.** Open a focused
  upstream PR or issue for changes that stand alone and do not depend on the
  daemon/API/web.
- **Do not open an upstream draft PR for the daemon/API/web/MCP yet.** Revisit
  once the core is stable and there is maintainer interest.
- **Sync upstream regularly** (monthly and before a release) instead of
  maintaining a long-lived divergent branch.

## Upstream remote and sync procedure

Configure the remote once:

```sh
git remote add upstream https://github.com/ccgauche/ytermusic.git
git fetch upstream
```

Sync through a dedicated branch and PR; do not rewrite `master`:

```sh
git fetch origin
git fetch upstream
git switch -c sync/upstream-$(date +%Y-%m)
git merge upstream/master
# resolve conflicts, then:
nix develop
just check
just test
git push -u origin HEAD
gh pr create --base master
```

Feature branches should be rebased onto the latest `master` before merging.
Check upstream activity before syncing:

```sh
git log --oneline --no-merges master..upstream/master
gh pr list -R ccgauche/ytermusic --state open
```

## Change classification

| Change | Destination |
| --- | --- |
| Download manager fixes | Upstreamable when self-contained |
| Player/audio fixes | Upstreamable when self-contained |
| `ytpapi2` parser/auth fixes | Upstreamable when self-contained |
| CI, formatting, clippy cleanup | Upstreamable if it does not conflict with the fork |
| Daemon, API, SSE, web UI, MCP | Fork only for now |
| Packaging/systemd/Nix module | Fork only for now |

## Cadence

- Sync upstream monthly and before a release.
- Open upstream PRs early for generic fixes, but keep them independent of the
  daemon so either side can merge without blocking the other.
- Re-evaluate upstreaming the daemon after the v1 core stabilizes.

## Consequences

- The fork can move quickly without waiting on upstream review.
- Divergence grows; generic fixes must be kept isolated and documented so they
  are easy to upstream later.
- Upstream merges may conflict with the legacy TUI; CI and the test workflow
  guard the merge.
- Attribution and license obligations remain in [NOTICE](../../NOTICE) and
  [LICENCE](../../LICENCE).

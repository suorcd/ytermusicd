# Agent workflow

## Work in separate worktrees

All work must be done in a separate git worktree, one per issue or PR. Do not
edit or commit directly in the primary `master` checkout.

Create a worktree from an up-to-date `master`:

```sh
git fetch origin
git worktree add <repo>/issue-<number>-<slug> \
  -b issue-<number>-<slug> origin/master
```

- Use a descriptive branch name, for example `issue-6-auth-core`.
- Keep one issue per worktree; do not mix unrelated changes.
- For stacked work, base the worktree on the parent issue branch and target
  that branch in the pull request. Retarget/rebase after the parent merges.
- Remove the worktree after the PR merges:

```sh
git worktree remove <repo>/issue-<number>-<slug>
```

## Before committing

Run the checks from the worktree:

```sh
nix develop
just check
just test
```

For bug fixes and behavior changes, add a regression test that fails before the
fix and passes after it. If a deterministic test is impractical, document the
manual reproduction and add the closest smoke test.

Never commit `headers.txt`, `account_id.txt`, browser cookies, or logs that
contain them.

## Pull requests

- Open the PR from the issue branch to `master` (or the parent branch for
  stacked work).
- Reference the issue with `Closes #...` or `Part of #...`.
- Include the test plan and any manual verification commands.

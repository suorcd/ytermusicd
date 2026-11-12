# Writing standard

All new and changed prose must follow ASD-STE100 Simplified Technical
English. The standard is Issue 9, released 2025-01-15. The standard is at
https://www.asd-ste100.org/.

This standard covers this prose:

- `AGENTS.md`, `README.md`, `CONTRIBUTING.md`, and `docs/`
- commit messages and pull request descriptions
- doc comments (`///` and `//!`) and code comments
- CLI help, error messages, and web UI copy

This standard does not cover names in code:

- identifiers, module paths, CLI flags, JSON keys, HTTP headers, config
  keys, and MCP tool names
- quoted third-party text, license text, and verbatim command output

Rules (a subset of the 53 writing rules):

- Write one instruction in one sentence. Use the active voice.
- Use 20 words or fewer in a procedural sentence. Use 25 words or fewer in
  a descriptive sentence.
- Use 6 sentences or fewer in a paragraph.
- Write the action as a verb. Write "delete the file", not "perform the
  deletion".
- Use one word for one thing. The daemon is the "daemon" in all files, not
  the "server".
- Do not use marketing adjectives, idioms, or phrasal verbs.
- Do not use "simply", "just", or "obviously".
- Use American English spelling: behavior, color, initialize, organize.

Do not change names in code to obey these rules. Keep the prose term and the
identifier equal in meaning, so the documents cannot drift from the API.

Do not rewrite unrelated text in a change that has another purpose.

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

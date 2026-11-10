# Contributing

## Development environment

Use the Nix dev shell when possible:

```sh
nix develop
```

See [docs/testing.md](docs/testing.md) for the full local development and
testing guide.

## Before opening a pull request

```sh
just check
just test
```

For bug fixes and behavior changes, add a test that fails before the fix and
passes after it. If the failure cannot be tested deterministically (audio
device, DBus/MPRIS, live YouTube network), document the manual reproduction and
add a smoke test where practical.

## Secrets

Never commit `headers.txt`, `account_id.txt`, browser cookies, or logs that
contain them. Use `YTERMUSICD_HEADERS` to point tests at a file outside the
repository.

## Scope

This project is a fork of [ytermusic](https://github.com/ccgauche/ytermusic).
Keep generic fixes reusable and call out changes that should be upstreamed.
See [0004 — upstream strategy](docs/decisions/0004-upstream-strategy.md) for the
sync procedure and change classification.

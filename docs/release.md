# Release checklist

The release version is `0.1.0`. The tag is `v0.1.0`.

## Before the tag

1. Start from a clean `master` checkout.
2. Run the checks:

   ```sh
   nix develop
   just check
   just test
   just web-check
   ```

3. Build the package:

   ```sh
   nix build .#ytermusicd
   ```

4. Set a real headers file and a free port. Run the local smoke test:

   ```sh
   YTERMUSICD_HEADERS="$HOME/.config/ytermusicd/headers.txt" \
     just smoke-local
   ```

5. Start the daemon and check each item:

   ```sh
   curl http://127.0.0.1:8080/api/status
   curl 'http://127.0.0.1:8080/api/search?q=civilization'
   curl -N --max-time 2 http://127.0.0.1:8080/api/events
   playerctl -l
   ```

6. Open `http://127.0.0.1:8080` and check library, playlist, play, queue,
   search, `?` help and the connection banner.
7. Check the MCP proxy:

   ```sh
   npx @modelcontextprotocol/inspector ytermusicd mcp
   ```

8. Check the systemd unit:

   ```sh
   systemctl --user restart ytermusicd
   systemctl --user status ytermusicd
   ```

9. Update `CHANGELOG.md` and `Cargo.toml` versions. Commit the changes.

## Tag and publish

```sh
git tag -a v0.1.0 -m "ytermusicd v0.1.0"
git push origin v0.1.0
```

Create a GitHub release from the tag. The release workflow builds:

- `ytermusicd-v0.1.0-x86_64-unknown-linux-gnu.tar.gz`
- `ytermusicd-v0.1.0-aarch64-apple-darwin.tar.gz`
- `ytermusicd-v0.1.0-x86_64-apple-darwin.tar.gz`
- `ytermusicd-v0.1.0-x86_64-pc-windows-msvc.zip`
- a SHA-256 file for each archive

## After the tag

1. Download one archive and run `sha256sum -c` or `shasum -a 256 -c`.
2. Extract the archive and run `ytermusicd --version`.
3. Run `ytermusicd serve` and open the web UI.
4. Check that the archive contains the embedded web UI:
   `curl http://127.0.0.1:8080/ | grep 'id="app"'`.
5. Close the roadmap issue when all checks pass.

## Rollback

Delete the release and the tag. Revert the version commit. Do not reuse the tag
for a different build.

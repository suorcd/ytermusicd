# Web UI

The web UI is a Svelte 5, Vite and TypeScript app in
`crates/ytermusicd-web`. It uses the REST and SSE API only. It keeps no state
of its own that the daemon does not own.

## Development

```sh
nix develop
just web-install
just web
```

Vite serves the app at `http://127.0.0.1:5173` and proxies `/api` to
`http://127.0.0.1:8080`. Start the daemon in another shell:

```sh
just dev
```

Type-check and build:

```sh
just web-check
# or
cd crates/ytermusicd-web
npm run check
npm run build
```

## Production serving

The daemon embeds `crates/ytermusicd-web/dist` at build time with `rust-embed`.
It serves the app at `/` and falls back to `index.html` for client-side routes.
Unknown files under `/assets/` return 404. Unknown `/api` paths return 404 and
never return `index.html`.

The build script writes a placeholder `dist/index.html` when no web build
exists. This keeps `cargo check` and `cargo test` working on a fresh checkout.
Run `npm run build` to replace the placeholder with the real app. The Nix
package and release workflow build the web UI before the Rust binary, so their
artifacts contain the real app.

## Views and state

- **Library** shows the playlist collection and a reload button.
- **Playlist** shows the track table and a *Play all* action.
- **Queue** shows current and upcoming tracks.
- **Search** is a modal palette.
- **Now playing** shows the current track, controls, seek, volume and download
  state.

The store subscribes once to `/api/events`. It applies the snapshot and
incremental playback, queue, download, track-ended, error and library events.
The app reconnects with exponential backoff and sends `last_event_id` in the
query. The daemon also accepts the standard `Last-Event-ID` header. The
connection banner shows connecting, reconnecting and offline states. The retry
button replaces the stream.

Volume, seek, playback and queue actions update the view before the request
finishes. A failed request restores the previous state and shows a toast.

## Theme

`src/app.css` defines these tokens:

`--bg`, `--bg-raised`, `--bg-sunken`, `--fg`, `--fg-muted`, `--fg-bright`,
`--accent`, `--accent-2`, `--accent-3`, `--danger`, `--warning`, `--success`,
`--selection`, `--border`.

The font stack is `JetBrainsMono Nerd Font`, then `ui-monospace, monospace`.
The app loads no network fonts and uses no CDN.

## Keys and mouse

| Input | Action |
| --- | --- |
| `j` / `↓` | Next row |
| `k` / `↑` | Previous row |
| `g` `g` | First row |
| `G` | Last row |
| `Ctrl+D` / `Ctrl+U` | Half page down / up |
| `/` or `Ctrl+K` | Open search |
| `Enter` | Play the selected item |
| `Space` | Play or pause |
| `x` | Queue the selected track |
| `d` `d` | Remove the selected queue item |
| `?` | Open help |
| `Esc` | Close a panel |
| Click | Select a row |
| Double click | Play a track |
| Right click | Open the track menu |
| Drag | Move a queue item |

Shortcuts do not run while an input has focus. The `?` overlay shows the same
table.

## Failure states

- The connection banner shows a lost event stream. The app retries the stream.
- The auth banner shows the `headers.txt` path and sign-in steps. It never
  shows cookie values.
- A download failure shows a *retry* action. The action calls
  `/api/downloads/retry`.
- API errors use one envelope and become dismissible toasts with an actionable
  message.
- A Svelte error boundary catches component failures and shows a retry button.

## Tests

```sh
cd crates/ytermusicd-web
npm test
```

Vitest and Testing Library cover the API client, the event stream, the store,
the keymap, the track table, the queue, now playing, search, the feedback
banners and the app shell. The tests use no network, audio device or daemon.

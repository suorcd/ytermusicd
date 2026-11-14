# MCP proxy

`ytermusicd mcp` is a stdio MCP proxy. It connects to a running daemon and
exposes the v1 tool set. It holds no playback state.

```sh
ytermusicd mcp
ytermusicd mcp --url http://127.0.0.1:9000
YTERMUSICD_URL=http://127.0.0.1:9000 ytermusicd mcp
```

The default URL is `http://127.0.0.1:8080`. The proxy uses the same REST API as
the web UI. Every request has a 30 second timeout. The proxy exits when stdin
closes.

## Client configuration

Claude Desktop and other MCP clients use a JSON server entry:

```json
{
  "mcpServers": {
    "ytermusicd": {
      "command": "ytermusicd",
      "args": ["mcp"],
      "env": {
        "YTERMUSICD_URL": "http://127.0.0.1:8080"
      }
    }
  }
}
```

For the MCP inspector:

```sh
npx @modelcontextprotocol/inspector ytermusicd mcp
```

For clients that accept a command tuple:

```sh
your-mcp-client --server "ytermusicd mcp"
```

## Tools

| Tool | Arguments | Result |
| --- | --- | --- |
| `search` | `query` | Tracks and playlists |
| `list_playlists` | none | Library playlists |
| `get_playlist` | `playlist_id` | Playlist detail and tracks |
| `play_playlist` | `playlist_id` | Updated status |
| `play_track` | `track` | Updated status |
| `queue_add` | `track` | Updated queue |
| `now_playing` | none | Status with current playback |
| `pause` | none | Updated status |
| `resume` | none | Updated status |
| `next` | none | Updated status |
| `previous` | none | Updated status |
| `volume` | `volume` (0-100) | Updated status |
| `download_status` | `video_id` (optional) | Download summary and queue tracks |

`track` is the object from `search` or `get_playlist`:
`video_id`, `title`, `artist`, `album` and `duration`.

Tool results include `structuredContent` with a JSON schema. The schema comes
from the shared `ytermusicd-api` DTOs.

## Errors

The proxy maps daemon errors to MCP errors:

- A stopped daemon returns `Cannot reach the daemon at <url>. Start it with:
  ytermusicd serve`.
- An upstream failure keeps the daemon message and the error code.
- An unknown playlist or video ID returns an invalid-parameter error.
- A bad volume returns an invalid-parameter error before the request.

The proxy does not crash on daemon errors. The client shows the message.

## Scope

The v1 proxy has the 13 tools above. These items are post-v1:

- playlist create, rename, delete and track edits;
- lyrics;
- streamable HTTP MCP transport;
- resources and prompts.

## Tests

```sh
cargo test -p ytermusicd mcp::
cargo test -p ytermusicd --test mcp_stdio
```

The unit tests use an in-process mock daemon. The stdio test starts the real
binary, performs the MCP handshake, lists 13 tools, calls `now_playing` and
checks that the process exits when stdin closes.

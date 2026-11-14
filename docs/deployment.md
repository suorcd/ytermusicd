# Deployment

## Nix package

```sh
nix build .#ytermusicd
nix profile install .#ytermusicd
nix run .#ytermusicd -- serve
```

The package wraps the binary with `yt-dlp` and `ffmpeg` on `PATH` and installs
the systemd user unit under `$out/lib/systemd/user/`. The package builds the web
UI and embeds it in the binary.

## Reverse proxy and TLS

The daemon API trusts local clients. Bind it to `127.0.0.1` and use a reverse
proxy for remote access. The proxy must add authentication and TLS.

Caddy example:

```caddy
music.example.com {
  reverse_proxy 127.0.0.1:8080
  basicauth {
    user $2a$14$replace-with-a-real-hash
  }
}
```

nginx example:

```nginx
location / {
  proxy_pass http://127.0.0.1:8080;
  proxy_http_version 1.1;
  proxy_set_header Host $host;
  proxy_buffering off;
}
```

`proxy_buffering off` keeps the SSE stream live. The daemon sends
`X-Accel-Buffering: no` for the same reason. Do not expose the daemon port
without a proxy.

## NixOS module

The flake exposes `nixosModules.default` for headless/server deployments:

```nix
{
  inputs.ytermusicd.url = "github:suorcd/ytermusicd";

  outputs = { self, nixpkgs, ytermusicd, ... }: {
    nixosConfigurations.host = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ytermusicd.nixosModules.default
        {
          services.ytermusicd = {
            enable = true;
            bind = "127.0.0.1";
            port = 8080;
            environmentFile = "/run/secrets/ytermusicd.env";
          };
        }
      ];
    };
  };
}
```

The system service runs as the `ytermusicd` system user with `HOME` under
`/var/lib/ytermusicd`; put `headers.txt` in
`/var/lib/ytermusicd/.config/ytermusicd/headers.txt` or point
`YTERMUSICD_HEADERS` at it via `environmentFile`. For desktop/MPRIS use the
home-manager or systemd user unit instead, so the daemon runs inside the user
session.

## home-manager module

```nix
{
  imports = [ inputs.ytermusicd.homeManagerModules.default ];
  services.ytermusicd = {
    enable = true;
    environmentFile = "/home/you/.config/ytermusicd/env";
  };
}
```

## systemd user service

The repository ships a user unit at
[`packaging/systemd/ytermusicd.service`](../packaging/systemd/ytermusicd.service).

```sh
# Install the binary somewhere on PATH (Nix profile shown here)
nix profile install .#ytermusicd

mkdir -p ~/.config/systemd/user
cp packaging/systemd/ytermusicd.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now ytermusicd

systemctl --user status ytermusicd
journalctl --user -u ytermusicd -f
```

The unit runs `ytermusicd serve` from a controlled `PATH` that includes
`~/.nix-profile/bin`, `/run/current-system/sw/bin` (NixOS) and the usual system
paths. Install the binary in one of those locations, or override `PATH` in the
environment file.

### Environment

Copy the example environment file and uncomment options:

```sh
cp packaging/systemd/ytermusicd.env.example ~/.config/ytermusicd/env
$EDITOR ~/.config/ytermusicd/env
```

The unit creates `~/.config/ytermusicd`, `~/.cache/ytermusicd` and
`~/.local/state/ytermusicd` via systemd's `ConfigurationDirectory`,
`CacheDirectory` and `StateDirectory`.

Authentication defaults to `~/.config/ytermusicd/headers.txt` (with a legacy
`~/.config/ytermusic/headers.txt` fallback). See the README for how to create
`headers.txt`; never commit it.

### Smoke tests

```sh
curl http://127.0.0.1:8080/api/status
curl http://127.0.0.1:8080/ | grep 'id="app"'
playerctl -l
```

`playerctl` lists `ytermusic` while the daemon runs.

### Troubleshooting

- **No audio**: the service runs in the user session. Make sure the user has
  access to the session audio server (PipeWire/PulseAudio), or add the user to
  the `audio` group on systems that use direct ALSA access.
- **DBus/MPRIS**: the user manager must have `DBUS_SESSION_BUS_ADDRESS` set;
  this is normal for `systemctl --user` sessions.
- **Service exits at startup**: run `ytermusicd serve` in a terminal with the
  same environment to see the actionable auth/config error.
- **Port conflict**: set `YTERMUSICD_PORT` in the environment file or change
  `--port` in `ExecStart`.

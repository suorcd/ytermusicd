# Deployment

## systemd user service

The repository ships a user unit at
[`packaging/systemd/ytermusicd.service`](../packaging/systemd/ytermusicd.service).

```sh
# Install the binary somewhere on PATH (Nix profile shown here)
nix profile install .#ytermusicd   # available once #23 lands

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
playerctl -l
```

`playerctl` should list `ytermusic` once MPRIS integration (#9) is merged.

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

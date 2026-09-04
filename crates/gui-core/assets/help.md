# SSH Tunnel Manager — Help

## Getting started

SSH Tunnel Manager connects to the daemon using
`~/.config/ssh-tunnel-manager/cli.toml`. For a local daemon, enable and start the
user service. On first launch, the GUI detects the generated connection snippet
and offers to import it; if none is available, it opens manual connection setup:

```bash
systemctl --user enable --now ssh-tunnel-daemon
```

The GUI shows **Checking daemon…** until its first health request completes.
Use **Retry** or press **F5** if the daemon is offline. Starting, restarting, or
stopping the daemon from the GUI is currently WIP; use `systemctl --user`.

## Profiles

Choose **New profile** or press **Ctrl+N** to create a profile. A local-forwarding
profile needs:

- a unique name;
- SSH host, port, and user;
- a local bind address and port;
- the destination host and port; and
- an SSH key or password-based authentication method.

The main list exposes the real action available for each structured tunnel
status: **Connect**, **Cancel**, **Disconnect**, or **Retry**. Profiles remain
readable while the daemon is offline, but tunnel actions are disabled.

Open a profile's actions menu to edit, duplicate, delete, pin, or change
auto-reconnect. Drag handles reorder profiles in Manual order. Keyboard users
can use **Move up** and **Move down** in the same menu; moves stay within the
pinned or unpinned section. Name sorting intentionally disables manual moves.

Profiles can be edited while connecting or connected. Saving does not alter the
current tunnel: the new settings apply on its next connection. After a successful
save, choose **OK** to leave the current connection alone or **Reconnect now** to
stop it and start it again with the saved settings.

Remote and dynamic/SOCKS profiles are preserved and displayed, but their daemon
execution is unsupported. The GUI marks that limitation and never silently
converts them to local forwarding. Importing `~/.ssh/config` is also WIP.

## Authentication and credentials

For a local daemon, an SSH key can be selected from the local filesystem. For a
remote daemon, the key path refers to the daemon host; the GUI does not upload
or validate that remote file locally.

**Save on this client** stores a password or key passphrase through Secret
Service. Existing daemon-host or file-storage settings are preserved, but
changing those credentials is WIP until the daemon exposes a typed write
capability. The daemon's displayed `credential_store` value is diagnostic text
only and never enables an action.

Authentication dialogs are selected only from structured daemon request codes.
Password and passphrase input is hidden when the structured request says it is
hidden. Keyboard-interactive requests use a generic response field unless the
daemon sends the dedicated two-factor code. Host-key prompts show the daemon's
notice as unparsed explanatory text; verify it independently before accepting.

Cancelling an authentication dialog stops that tunnel. A submitted answer stays
in a verifying state until a structured event or inventory update completes the
request.

## Daemon information

Open daemon status from the status indicator or press **Ctrl+2**. The page shows
only values returned by the structured daemon information response, including
version, uptime, active tunnel count, process ID, listener settings, access
policy, paths, process user, credential-store diagnostic, and the last
structured heartbeat timestamp.

Health **Refresh** is real. Daemon start, restart, and shutdown controls remain
marked WIP under the current GUI scope and do not report false success.

## Keyboard shortcuts

- **Ctrl+F** — focus profile search
- **Ctrl+N** — create a profile
- **F5** or **Ctrl+R** — refresh profiles and daemon health
- **Ctrl+1** — open profiles
- **Ctrl+2** — open daemon information
- **Escape** — close dialogs or return from a navigation page where supported

All controls remain reachable through normal Tab and arrow-key navigation.

## Configuration files

Files are stored under `${XDG_CONFIG_HOME:-~/.config}/ssh-tunnel-manager/`:

- `cli.toml` — client-to-daemon connection settings;
- `ui.toml` — GUI-only pinned/order/filter preferences;
- `profiles/*.toml` — tunnel profiles;
- `daemon.toml` — daemon configuration;
- `daemon.token` — daemon API authentication token;
- `cli-config.snippet` — protected daemon-generated client setup snippet;
- `known_hosts` — daemon host-key database; and
- `server.crt` and `server.key` — daemon TLS material when HTTPS is configured.

`ui.toml` is separate from profile and daemon connection configuration. A
missing UI preferences file uses safe defaults; malformed optional preferences
produce a recoverable warning.

## Troubleshooting

Check the daemon and its logs:

```bash
systemctl --user status ssh-tunnel-daemon
journalctl --user -u ssh-tunnel-daemon -f
```

For a remote daemon, confirm the HTTPS host/port, authentication token, TLS
fingerprint, firewall access, and that SSH keys exist on the daemon host. For a
client credential error, make sure the desktop Secret Service is available and
unlocked.

Profile and daemon error strings are displayed for diagnosis, but they never
select a UI action. Unknown or contradictory protocol codes fail closed.

## More information

- Project documentation: `docs/`
- Repository: <https://github.com/SchirmForge/ssh-tunnel-manager>
- Issues: <https://github.com/SchirmForge/ssh-tunnel-manager/issues>

SSH Tunnel Manager v0.6.0 · released 2026-09-03 · Apache-2.0

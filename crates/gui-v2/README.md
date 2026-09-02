# SSH Tunnel Manager GUI

This is the production GTK 4/libadwaita application produced by the GUI redesign.
It is a root-workspace default member whose package and binary are named
`ssh-tunnel-gui`; its application ID is `io.github.schirmforge.SshTunnelManager`.
Validation and cutover evidence is recorded in
[`PHASE7_VALIDATION.md`](PHASE7_VALIDATION.md).

The profile list supports persisted pinning, manual ordering, name sorting,
connected-only filtering, local search, selection, and the structured shared
connect/cancel/disconnect/retry actions. Profile details, create/edit/duplicate,
confirmed deletion, local key selection, and client-held credential storage are
wired through `gui-core`. Human authentication uses one request-ID-keyed modal
at a time, follows the core FIFO queue, obeys structured request/input codes,
and waits for structured daemon confirmation after submit or cancel. Host-key
copy is displayed without parsing. The daemon page renders structured
checking, online, offline, and information states; health refresh/retry is real.
Daemon start, restart, and shutdown plus SSH configuration import are explicit
WIP actions and report that state instead of reporting success.

Before its runtime starts, the application validates `cli.toml`. If the file is
missing or invalid, a non-blocking first-launch flow can import the daemon's
`cli-config.snippet` or collect Unix socket, HTTP, or HTTPS settings manually.
The daemon API token is redacted from debug state, and the completed file is
replaced atomically with mode `0600`. This configures a connection to an
existing daemon; it does not start or restart the daemon process.

Build on Fedora 44 with GTK 4.22, libadwaita 1.9, and GLib 2.88 development
packages installed:

```console
cargo check --package ssh-tunnel-gui --locked
```

The UI follows the system color scheme and font configuration. No font or
appearance preference is bundled or forced.

Keyboard shortcuts include Control+F for profile search, Control+N for a new
profile, F5 or Control+R for refresh, and Control+1/Control+2 for navigation.

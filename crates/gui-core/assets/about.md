# SSH Tunnel Manager

**Version**: 0.6.0
**Release date**: 2026-09-03

## Description

A modern GTK4/Libadwaita application for managing SSH tunnels through a background daemon. Provides an intuitive interface for creating, monitoring, and controlling SSH port forwarding connections.

Supports both local daemon connections via Unix socket and remote daemon connections over HTTPS, enabling you to manage tunnels on headless servers from your desktop.

## Features

### New in v0.6.0

The second-generation interface is now the production desktop application.

- **Changed**: profiles can be edited while their tunnel is active. Saving keeps the current
  connection intact and offers either **OK** or **Reconnect now**.
- **Changed**: reconnecting after an edit waits for structured daemon status before starting
  the saved profile; daemon messages are never parsed to drive the action.
- **Fixed**: daemon authentication prompt text now follows the dialog's semantic window
  background instead of appearing on a contrasting card.
- **Changed**: the previous GTK interface is retained only as obsolete, frozen source and is
  no longer installed or packaged.

### New in v0.5.0

Authentication prompts now reach you reliably, and the daemon will not start in a way that
nothing can connect to.

- **Fixed**: an authentication prompt could be lost when the event stream reconnected, so
  starting a tunnel failed with "Authentication prompt timed out after 60s" even with the
  application open and waiting. The stream is no longer cut on a timer, reconnect backoff
  recovers, and the daemon re-sends any prompt still outstanding to a client that connects.
- **Fixed**: a repeated prompt could erase itself instead of staying on screen.
- **Changed**: the daemon refuses to run as root. If you were doing that to forward a port
  below 1024, grant the capability instead — `sudo setcap cap_net_bind_service=+ep` on the
  daemon binary — and run it as your normal user.
- **Changed**: the command line and the graphical clients now share one event path, so a fix
  to event delivery reaches both.

### New in v0.4.0

The second-generation GTK interface is available as a source preview. It uses a
toolkit-neutral application core and does not yet replace the packaged GTK GUI.

- **Added**: first-launch client setup with daemon-snippet import and manual
  Unix socket, HTTP, or HTTPS configuration
- **Added**: adaptive profile search, filtering, pinning, sorting, and ordering
- **Added**: typed, request-correlated authentication and explicit WIP states
- **Changed**: GUI-only preferences are stored separately in `ui.toml`
- **Security**: daemon text is display-only; structured codes and fields select actions

### New in v0.3.0

Credential storage. Saving a password or passphrase now works when the daemon runs on
another machine — previously the credential was stored here and looked for there, so you
were asked for it every time regardless.

- **Fixed**: client-held credential storage against a remote daemon
- **Changed**: the daemon reports which credential store it is using
- **Changed**: new GUI profiles record an explicit client or daemon-host storage location;
  the older ambiguous `keychain` value is read but never written

### In v0.2.0

A security and supply-chain release. No new user-facing features; the work went into the
dependency tree and the tests that verify it.

- **Fixed**: a mistyped SSH password now re-prompts instead of failing the tunnel outright
- **Security**: SSH compression and SHA-1 MACs are no longer offered, and a server
  presenting a host *certificate* is refused rather than silently trusted
- **Security**: known vulnerabilities in the dependency tree reduced from 25 to 1

### In v0.1.11

- **Fixed**: cancelling a tunnel during authentication now stops it cleanly instead of
  force-aborting after a timeout
- **Fixed**: several potential crashes in this application, where a daemon request could
  panic if it overlapped with another action

### Earlier

- **Client configuration**: Load daemon connection settings from the shared `cli.toml`
- **Remote Daemon Support**: Connect to daemons over HTTPS on other machines
- **Hybrid Profile Mode**: Profiles sent via API while SSH keys stay secure on daemon host

### Core Features

- **Profile Management**: Save and reuse SSH tunnel configurations
- **Real-time Monitoring**: Live status updates for all tunnels via Server-Sent Events
- **Authentication Support**: SSH keys, passwords, keyboard-interactive (2FA)
- **Daemon Architecture**: Background service for reliable tunnel management
- **Interactive Authentication**: Dynamic prompts for passwords and 2FA codes
- **SSH Host Key Verification**: OpenSSH-compatible known_hosts with SHA256 fingerprints
- **Credential Storage**: Secret Service on desktops, with an explicit client/daemon-host location
- **Modern UI**: Built with GTK4 and Libadwaita for a native GNOME experience
- **Accessible actions**: Keyboard search, navigation, profile reordering, and labelled controls

## Components

- **GUI** (`ssh-tunnel-gui`): This graphical interface
- **Daemon** (`ssh-tunnel-daemon`): Background service managing tunnels
- **CLI** (`ssh-tunnel`): Command-line interface for scripting and automation

## Technology Stack

- **Language**: Rust
- **GUI Framework**: GTK4 + Libadwaita (GNOME)
- **SSH Library**: russh
- **HTTP Client**: reqwest with Unix socket and HTTPS support
- **Architecture**: Client-daemon with SSE for real-time updates

## Connection Modes

### Local Mode (Default)
- Unix domain socket communication
- No network exposure
- Profiles loaded from `~/.config/ssh-tunnel-manager/profiles/`

### Remote Mode
- HTTPS connection to remote daemon
- TLS certificate fingerprint pinning
- Token-based authentication
- Hybrid profile mode: SSH keys on daemon, configuration via API

## License

Apache-2.0

## Credits

Built with Rust and modern GNOME technologies.

Developed with assistance from Generative AI.

## Links

- **Documentation**: See Help menu for user guide
- **Project Repository**: https://github.com/SchirmForge/ssh-tunnel-manager
- **Report Issues**: https://github.com/SchirmForge/ssh-tunnel-manager/issues

---

© 2025-2026 SchirmForge

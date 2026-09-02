# SSH Tunnel Manager

A secure, performant SSH tunnel management application for Linux with CLI interface and event-driven architecture.

## Status

**Version**: v0.5.0
**Status**: Production-ready CLI, daemon, and packaged GTK GUI; second-generation GUI available as a source preview

### Highlights

- ✅ **Packaged GTK GUI** - Stable GUI with first-launch setup, profile management, and live status
- 🧪 **Second-generation GUI preview** - First-launch setup, adaptive profile organization, typed shared actions, safe credential editing, and honest WIP states
- ✅ **Remote daemon support** - Connect to daemons over HTTPS on other machines
- ✅ **Full-featured GUI** - Profile management, real-time status, and markdown documentation
- ✅ **Enhanced CLI** - Status/restart commands and proactive config validation
- ✅ **Local port forwarding** - Works end-to-end with interactive auth, keychain storage, and host key verification
- ✅ **Tested** - A hermetic test tier plus a live SSH tier covering every authentication flow, and CI

### Limitations

- 🚧 Notification on connection lost or reconnect not implemented yet
- 🚧 Auto-reconnect wiring pending

## Features

### ✅ Implemented
- **First-Launch Configuration Wizard**: Both desktop GUIs provide interactive client setup with automatic snippet detection and manual configuration fallback
- **Remote Daemon Support**: Connect to daemons over HTTPS on remote machines while keeping SSH keys secure
- **CLI Interface**: Full-featured command-line tool with interactive prompts and JSON/table output
- **Packaged GTK4/Libadwaita GUI**: Modern GNOME-style application with full profile CRUD, real-time status indicators, and markdown documentation
- **GUI v2 source preview**: First-launch client setup, search, pinning, manual ordering, connected-only filtering, request-correlated authentication, daemon/offline pages, adaptive layout, and keyboard actions. It is not yet packaged or the default GUI.
- **Multiple Authentication Methods**: SSH keys, passwords, keyboard-interactive (2FA)
- **Credential-store integration**: Secret Service on desktops, keyutils fallback on headless Linux
- **Local Port Forwarding**: Forward local ports to remote hosts via SSH with host key verification
- **Real-Time Updates**: Server-Sent Events (SSE) for live status updates in both CLI and GUI
- **Interactive Authentication**: Dynamic prompts for passwords, passphrases, and 2FA codes
- **Detailed Error Messages**: Know exactly why authentication failed and what the server requires
- **Privileged Port Guidance**: Clear messaging for ports ≤1024
- **Security Hardening**: Comprehensive file/directory permissions, authentication by default, HTTPS enforcement for network access

### 🚧 Planned
- Dynamic (SOCKS) port forwarding
- Auto-reconnect/health monitoring wiring
- Desktop notifications
- Additional packages

Remote port forwarding (`ssh -R`) is **not planned**. Use `ssh -R` directly if you need it.

## Installation

### Quick Install
- **Debian/Ubuntu**: `.deb` packages available for daemon, CLI, and GUI
- **From Source**: Build with Rust and cargo. See **[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)**

### Getting Started
1. Install packages or build from source
2. Start daemon as user service: `systemctl --user enable --now ssh-tunnel-daemon`
3. Launch GUI: `ssh-tunnel-gtk` (configuration wizard runs automatically)
4. Create profiles and start tunnels

For detailed instructions, platform-specific requirements, system service configuration, and advanced setup (HTTPS mode, network access, group permissions), see the **[Installation Guide](docs/INSTALLATION.md)**.

The second-generation GUI is currently a Fedora 44/Bazzite-targeted source preview in
`crates/gui-v2`. Its build and validation commands are documented in the
**[Development Guide](docs/DEVELOPMENT.md)**. The packaged/default application remains
`ssh-tunnel-gtk` until manual accessibility and runtime acceptance plus a separately
reviewed cutover are complete.

### CLI (For automation and scripts)

```bash
# Profile management
ssh-tunnel add <name>          # Interactive profile creation
ssh-tunnel list                # List all profiles
ssh-tunnel info <name>         # Show profile details
ssh-tunnel delete <name>       # Delete profile

# Tunnel control
ssh-tunnel start <name>        # Start tunnel
ssh-tunnel stop <name>         # Stop tunnel
ssh-tunnel restart <name>      # Restart tunnel
ssh-tunnel status [name]       # Check status (--all for table)
ssh-tunnel stop --all          # Stop all tunnels
```

- **More information**: See **[CLI Usage Guide](docs/INSTALLATION.md#option-b-using-the-cli)** for command-line operations, automation, and scripting
- **Help**: Run `ssh-tunnel --help` for built-in documentation

## Configuration

### Profile Storage

Profiles are stored in `~/.config/ssh-tunnel-manager/profiles/` as TOML files.

**Note**: To edit existing profiles, you can either:
- Manually edit the TOML files in `~/.config/ssh-tunnel-manager/profiles/`
- Use the GUI application for a graphical profile editor
- Delete and recreate the profile using the CLI

Example profile structure:

```toml
[metadata]
id = "550e8400-e29b-41d4-a716-446655440000"
name = "Production DB"
created_at = "2024-01-15T10:30:00Z"
modified_at = "2024-01-15T10:30:00Z"

[connection]
host = "ssh.example.com"
port = 22
user = "myuser"
auth_type = "Key"
key_path = "/home/user/.ssh/id_ed25519"
password_stored = true

[forwarding]
forwarding_type = "Local"
bind_address = "127.0.0.1"
local_port = 5432
remote_host = "db.internal.example.com"
remote_port = 5432

[options]
compression = false
keepalive_interval = 60
auto_reconnect = true
reconnect_attempts = 3
reconnect_delay = 5
tcp_keepalive = false
max_packet_size = 65536  # 64 KiB
window_size = 2097152    # 2 MiB
```

### Credential-store integration

When you choose to store credentials:

- **Service**: `ssh-tunnel-manager`
- **Username**: `{profile-uuid}`
- **Desktop store**: Secret Service API (GNOME Keyring, KWallet, etc.)
- **Headless fallback**: Linux kernel keyutils keyring

### Server and Headless Environments

SSH Tunnel Manager selects a credential store at runtime. `auto` prefers Secret Service and
falls back to keyutils when no desktop session is available.

#### Automatic Detection

When running on a headless server:

- keyutils needs no D-Bus or graphical session
- keyutils credentials **do not survive reboot**
- the daemon reports which store it opened
- if no store or credential is available, profile creation still succeeds and the daemon
  prompts an attached client when starting the tunnel

Set `credential_store = "secret-service"`, `"keyutils"`, or `"none"` in `daemon.toml` to
override `auto`.

#### Manual Override

To disable credential storage explicitly (useful for automation):

```bash
export SSH_TUNNEL_SKIP_KEYRING=1
ssh-tunnel add myprofile --host server.com --user myuser --key ~/.ssh/id_ed25519
```

Useful for:
- Docker containers and CI/CD
- Ansible/configuration management
- Systemd system services
- Environments where keyring causes issues

See [SYSTEMD.md](docs/architecture/SYSTEMD.md) for system service configuration.

### Authentication Methods

#### SSH Key Authentication

```bash
# Add profile with SSH key
ssh-tunnel add myprofile
# When prompted, enter the path to your SSH key
# If the key has a passphrase, you can store it in the keychain
```

#### Multi-Factor Authentication

The daemon automatically handles complex authentication flows:

```bash
# Server requires publickey + keyboard-interactive (2FA)
ssh-tunnel start myprofile
# You'll be prompted for your SSH key passphrase (if needed)
# Then prompted for your 2FA code
```

## Troubleshooting

For troubleshooting common issues, see the comprehensive **[Troubleshooting](docs/INSTALLATION.md#troubleshooting)** section in the Installation Guide, which covers:

- CLI configuration missing (401 errors)
- Authentication failures
- Privileged port binding
- Keychain issues
- Daemon startup problems
- GUI connection issues
- Remote daemon SSH key setup

Quick help:
```bash
# Check daemon status
ssh-tunnel info

# View daemon logs
journalctl --user -u ssh-tunnel-daemon -f

# Reset configuration
rm -rf ~/.config/ssh-tunnel-manager/cli.toml
# Then restart GUI or run any CLI command to regenerate
```

## Documentation

| Document | What it covers |
|---|---|
| **[Installation Guide](docs/INSTALLATION.md)** | Install, first run, service setup, troubleshooting |
| **[Development Guide](docs/DEVELOPMENT.md)** | Build, test (both tiers), dev sandbox |
| **[User Stories](docs/user-stories/)** | What the product does today, by epic, with status |
| **[Architecture](docs/architecture/)** | Functional and technical specification, security, requirements |
| **[Roadmap](docs/ROADMAP.md)** | What is planned, and what is explicitly not |
| **[Project Status](docs/PROJECT_STATUS.md)** | Current implementation snapshot |
| **[Known Issues](docs/KNOWN_ISSUES.md)** | Bugs and limitations |
| **[Changelog](docs/CHANGELOG.md)** | Version history |

## Architecture

- **[Functional Architecture](docs/architecture/FUNCTIONAL_ARCHITECTURE.html)** - components,
  responsibilities, data flow, configuration and API behaviour
- **[Technical Architecture](docs/architecture/TECHNICAL_ARCHITECTURE.html)** - design
  decisions and rationale, crate layout, concurrency model, testing strategy
- **[Technical Reference](docs/architecture/TECHNICAL_REFERENCE.md)** - module, struct and
  API-level reference

> The two architecture documents are HTML with embedded diagrams. GitHub shows HTML as
> source, so open them from a local checkout in a browser.

## API

The daemon exposes a REST API over Unix socket (default), HTTP (localhost testing), or HTTPS (network access).

For complete API documentation including endpoints, authentication, and examples, see **[docs/architecture/TECHNICAL_REFERENCE.md](docs/architecture/TECHNICAL_REFERENCE.md#api-description-daemon-httpsse)**.

## Security

SSH Tunnel Manager follows security best practices:

- **Authentication by default**: Token-based authentication enabled for all daemon modes
- **HTTPS for network access**: TLS required for non-localhost connections with certificate fingerprint pinning
- **Restrictive permissions**: Files (0600), directories (0700), and Unix sockets (0600) protected from other users
- **Credential protection**: Keychain storage for passwords, SSH keys referenced by path only
- **SSH host key verification**: Managed `known_hosts` file with SHA256 fingerprints
- **Minimal privileges**: Runs as regular user, uses `CAP_NET_BIND_SERVICE` for privileged ports

For comprehensive security documentation including threat model, remote daemon best practices, and vulnerability reporting, see **[docs/architecture/SECURITY.md](docs/architecture/SECURITY.md)**.

## Known Limitations

1. **Forwarding Types**: Only local port forwarding is implemented. Dynamic/SOCKS is a
   future item; remote forwarding (`ssh -R`) is not planned.
2. **Auto-Reconnect/Health**: Options exist but reconnection/health monitoring isn't wired yet
3. **Platform**: Primary development on Linux; macOS/Windows untested
4. **SSH Agent**: File-based keys only (no ssh-agent integration yet)
5. **Privileged Ports**: Requires `sudo` or `CAP_NET_BIND_SERVICE` for ports ≤1024

Each of these is documented with its status in the
**[user stories](docs/user-stories/)**.

## Roadmap

See **[docs/ROADMAP.md](docs/ROADMAP.md)** for what is planned, what is deliberately not,
and the design decisions already taken for work that has not started.

**Now**: stability and regression testing; packaging (DEB done, RPM in progress).
**Next**: GUI v2 runtime validation and cutover review, unattended credentials for headless
daemons, desktop notifications and auto-reconnect, configurable daemon config path, and
enhanced logging.
**Not planned**: remote port forwarding (`ssh -R`) - use OpenSSH directly.

Release history is in [docs/CHANGELOG.md](docs/CHANGELOG.md) and
[docs/releases/](docs/releases/).

## Contributing

This is a personal project, but contributions are welcome!

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test`
5. Submit a pull request

## License

Apache-2.0

## Acknowledgments

- Built with [russh](https://github.com/Eugeny/russh) for SSH protocol implementation
- Uses [keyring](https://crates.io/crates/keyring) for cross-platform credential storage
- CLI built with [clap](https://crates.io/crates/clap) and [dialoguer](https://crates.io/crates/dialoguer)
- Developed with assistance from generative AI.

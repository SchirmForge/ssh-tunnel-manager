# SSH Tunnel Manager

A secure, performant SSH tunnel management application for Linux with CLI interface and event-driven architecture.

## Status

**Version**: v0.1.9
**Status**: Production-ready CLI/Daemon/GUI(GTK) with remote daemon support

### Highlights

- ✅ **First-launch configuration wizard** - Interactive GUI setup with automatic snippet detection
- ✅ **Remote daemon support** - Connect to daemons over HTTPS on other machines
- ✅ **Full-featured GUI** - Profile management, real-time status, and markdown documentation
- ✅ **Enhanced CLI** - Status/restart commands and proactive config validation
- ✅ **Local port forwarding** - Works end-to-end with interactive auth, keychain storage, and host key verification

### Limitations

- 🚧 Remote/dynamic forwarding not implemented yet
- 🚧 Auto-reconnect wiring pending
- ⚠️ Some `crates/common` tests are stale

## Features

### ✅ Implemented
- **First-Launch Configuration Wizard**: Interactive GUI setup with automatic snippet detection and manual configuration fallback
- **Remote Daemon Support**: Connect to daemons over HTTPS on remote machines while keeping SSH keys secure
- **CLI Interface**: Full-featured command-line tool with interactive prompts and JSON/table output
- **GTK4/Libadwaita GUI**: Modern GNOME-style application with full profile CRUD, real-time status indicators, and markdown documentation
- **Multiple Authentication Methods**: SSH keys, passwords, keyboard-interactive (2FA)
- **Keychain Integration**: Store passwords/passphrases in system keychain
- **Local Port Forwarding**: Forward local ports to remote hosts via SSH with host key verification
- **Real-Time Updates**: Server-Sent Events (SSE) for live status updates in both CLI and GUI
- **Interactive Authentication**: Dynamic prompts for passwords, passphrases, and 2FA codes
- **Detailed Error Messages**: Know exactly why authentication failed and what the server requires
- **Privileged Port Guidance**: Clear messaging for ports ≤1024
- **Security Hardening**: Comprehensive file/directory permissions, authentication by default, HTTPS enforcement for network access

### 🚧 Planned
- Remote port forwarding
- Dynamic (SOCKS) port forwarding
- Auto-reconnect/health monitoring wiring
- Desktop notifications
- Packaging (Flatpak, AUR, deb)

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

### Keychain Integration

When you choose to store credentials in the keychain:

- **Service**: `ssh-tunnel-manager`
- **Username**: `{profile-uuid}`
- **Platform Support**:
  - Linux: Secret Service API (GNOME Keyring, KWallet, etc.)

### Server and Headless Environments

SSH Tunnel Manager works seamlessly on servers and in containers, with automatic keyring fallback.

#### Automatic Detection

When running on headless servers or in environments without keyring access:
- The CLI **automatically detects** keyring unavailability
- Profile creation **still succeeds** - passwords just aren't stored
- You'll see: `⚠️  System keychain not available`
- The daemon will **prompt interactively** when starting tunnels

#### Manual Override

To explicitly disable keyring (useful for automation):

```bash
export SSH_TUNNEL_SKIP_KEYRING=1
ssh-tunnel add myprofile --host server.com --user myuser --key ~/.ssh/id_ed25519
```

Useful for:
- Docker containers and CI/CD
- Ansible/configuration management
- Systemd system services
- Environments where keyring causes issues

See [SYSTEMD.md](docs/SYSTEMD.md) for system service configuration.

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

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for detailed architecture documentation including:
- System architecture and component breakdown
- Key design decisions and rationale
- Communication flow diagrams
- Data flow and API design
- Security considerations

## API

The daemon exposes a REST API over Unix socket (default), HTTP (localhost testing), or HTTPS (network access).

For complete API documentation including endpoints, authentication, and examples, see **[docs/TECHNICAL_REFERENCE.md](docs/TECHNICAL_REFERENCE.md#api-description-daemon-httpsse)**.

## Security

SSH Tunnel Manager follows security best practices:

- **Authentication by default**: Token-based authentication enabled for all daemon modes
- **HTTPS for network access**: TLS required for non-localhost connections with certificate fingerprint pinning
- **Restrictive permissions**: Files (0600), directories (0700), and Unix sockets (0600) protected from other users
- **Credential protection**: Keychain storage for passwords, SSH keys referenced by path only
- **SSH host key verification**: Managed `known_hosts` file with SHA256 fingerprints
- **Minimal privileges**: Runs as regular user, uses `CAP_NET_BIND_SERVICE` for privileged ports

For comprehensive security documentation including threat model, remote daemon best practices, and vulnerability reporting, see **[docs/SECURITY.md](docs/SECURITY.md)**.

## Known Limitations

1. **Forwarding Types**: Only local port forwarding implemented (remote/dynamic pending)
2. **Auto-Reconnect/Health**: Options exist but reconnection/health monitoring isn't wired yet
3. **Platform**: Primary development on Linux; macOS/Windows untested
4. **SSH Agent**: File-based keys only (no ssh-agent integration yet)
5. **Privileged Ports**: Requires `sudo` or `CAP_NET_BIND_SERVICE` for ports ≤1024
6. **Tests**: Some `crates/common` tests are stale and need updates

## Roadmap

See [docs/PROJECT_STATUS.md](docs/PROJECT_STATUS.md) for detailed implementation status and roadmap.

### Completed Features ✅

#### v0.1.9 (Latest)
- ✅ **First-launch configuration wizard** - Interactive GUI setup with automatic snippet detection, manual config dialog, and IP address prompts
- ✅ **Remote daemon profile support** - Profiles work with HTTP/HTTPS remote daemons in hybrid mode (profile via API, SSH keys on daemon filesystem)
- ✅ **SSH Key Setup Warning opt-out** - "Don't show this again" checkbox for SSH key setup dialog
- ✅ **Daemon settings improvements** - Hides restart daemon button for HTTPS mode (remote daemons)
- ✅ **Enhanced SSH key error messages** - Shows daemon's actual SSH directory paths instead of generic `~/.ssh`

#### v0.1.8
- ✅ **Enhanced 401 authentication error handling** - Proactive config validation with interactive snippet copy
- ✅ **CLI status command** - Display tunnel status with `--all` flag for formatted table view
- ✅ **CLI restart command** - Graceful tunnel restart with two-step stop→start process
- ✅ **CLI stop --all command** - Stop all active tunnels with status checking
- ✅ **IPv6 host management** - Proper URL formatting with `[addr]:port` notation for IPv6 literals
- ✅ **Tunnel description formatting** - Unified display across CLI/GUI with proper local/remote labeling

### Planned Features 🚧

#### High Priority
- 🚧 **Remote port forwarding** (`ssh -R`) - Forward remote server ports to local destinations
- 🚧 **Dynamic/SOCKS proxy** (`ssh -D`) - SOCKS5 proxy for dynamic port forwarding
- 🚧 **Configurable daemon config path** - Pass daemon config file as parameter (default: `~/.config/ssh-tunnel-manager`)
- 🚧 **Enhanced logging** - Daemon logging with `--debug` option and configurable log levels
  - Options: journalctl integration or dedicated log files

#### Medium Priority
- 🚧 **Daemon management GUI** - Graphical interface for daemon configuration and monitoring

#### Future Enhancements
- Desktop notifications for tunnel status changes
- Auto-reconnect/health monitoring wiring
- Packaging (Flatpak, AUR, deb)

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

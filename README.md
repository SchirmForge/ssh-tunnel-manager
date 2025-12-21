# SSH Tunnel Manager

A secure, performant SSH tunnel management application for Linux with CLI interface and event-driven architecture.

## Status

🟢 **v0.1.6** — Production-ready CLI/Daemon/GUI with comprehensive security hardening
✅ **Full-featured GUI** with profile management, real-time status, and markdown documentation
🟢 **Local port forwarding** works end-to-end with interactive auth, keychain storage, and host key verification
🚧 Remote/dynamic forwarding and auto-reconnect wiring are not implemented yet; some `crates/common` tests are stale.

## Features

### ✅ Implemented
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

## Quick Start

### Prerequisites

- Rust 1.75+ (install via [rustup](https://rustup.rs/))
- Linux (primary development platform)

### Build

```bash
# Clone the repository
git clone https://github.com/schirmForge/ssh-tunnel.git
cd ssh-tunnel

# Build CLI and daemon
cargo build --release --package ssh-tunnel-cli --package ssh-tunnel-daemon
```

### Basic Usage

```bash
# Start the daemon (in one terminal)
RUST_LOG=info ./target/release/ssh-tunnel-daemon

# Create a profile (in another terminal)
./target/release/ssh-tunnel add myprofile

# Start the tunnel
./target/release/ssh-tunnel start myprofile

# List all profiles
./target/release/ssh-tunnel list

# Stop the tunnel
./target/release/ssh-tunnel stop myprofile
```

## Installation

### Quick Install with Systemd

Use the provided installation script for automated setup:

```bash
# User service (recommended for most users)
./scripts/install.sh --user-unit --enable

# System service (for privileged ports or system-wide daemon)
sudo ./scripts/install.sh --system-unit --instance tunneld --enable
```

The script builds binaries, installs them to `/usr/local/bin`, and configures systemd.

### Manual Installation

See [docs/SYSTEMD.md](docs/SYSTEMD.md) for detailed installation steps:
- Per-user service (default): [docs/systemd/ssh-tunnel-daemon.user.service](docs/systemd/ssh-tunnel-daemon.user.service)
- System service as `tunneld` with CAP_NET_BIND_SERVICE for ports <1024: [docs/systemd/ssh-tunnel-daemon@.service](docs/systemd/ssh-tunnel-daemon@.service)
- Logs are in journald (`journalctl --user-unit ssh-tunnel-daemon -f` or `journalctl -u ssh-tunnel-daemon@tunneld -f`)

## Project Structure

```
ssh-tunnel/
├── crates/
│   ├── common/          # Shared types and utilities (✅ Complete)
│   ├── daemon/          # Core daemon service (✅ Production-ready)
│   ├── cli/             # Command-line interface (✅ Production-ready)
│   └── gui/             # GTK4/Libadwaita desktop application (✅ Functional)
├── docs/                # Documentation and systemd units
├── scripts/             # Installation scripts
├── Cargo.toml           # Workspace configuration
└── README.md
```

## Architecture

### Communication Flow

```
┌─────────────┐                    ┌─────────────┐
│     CLI     │◄─── SSE Events ────│   Daemon    │
│             │                     │             │
│  Interactive│──── HTTP/REST ────►│ Tunnel Mgmt │
│   Prompts   │                     │   russh     │
└─────────────┘                    └─────────────┘
                                           │
                                           ▼
                                    ┌─────────────┐
                                    │  SSH Server │
                                    └─────────────┘
```

### Key Design Decisions

1. **Event-Driven Authentication**: Daemon emits events when it needs credentials; CLI prompts interactively
2. **HTTP + SSE**: Simple REST API for commands, Server-Sent Events for real-time updates
3. **Keychain Integration**: Optional password/passphrase storage in system keychain
4. **Detailed Error Messages**: Parse russh `AuthResult` to provide clear authentication failure reasons

## Usage Examples

### Create a Profile (Interactive)

```bash
./target/release/ssh-tunnel add production-db
```

You'll be prompted for:
- Remote host and port
- SSH username
- Authentication method (SSH key or password)
- Whether to store credentials in keychain
- Port forwarding configuration
- Advanced options (compression, keepalive, etc.)

### Create a Profile (Non-Interactive)

```bash
./target/release/ssh-tunnel add production-db \
  --host ssh.example.com \
  --port 22 \
  --user myuser \
  --key ~/.ssh/id_ed25519 \
  --bind-address 127.0.0.1 \
  --local-port 5432 \
  --remote-host db.internal.example.com \
  --remote-port 5432 \
  --non-interactive
```

### Advanced Options via CLI Flags

```bash
./target/release/ssh-tunnel add myprofile \
  --host ssh.example.com \
  --user myuser \
  --key ~/.ssh/id_ed25519 \
  --local-port 8080 \
  --remote-host internal-api \
  --remote-port 80 \
  --compression true \
  --keepalive-interval 30 \
  --reconnect-attempts 5 \
  --reconnect-delay 10 \
  --max-packet-size 32768 \
  --window-size 1048576
```

### Manage Tunnels

```bash
# List all profiles
./target/release/ssh-tunnel list

# Show specific profile
./target/release/ssh-tunnel show production-db

# Start tunnel (with real-time status)
./target/release/ssh-tunnel start production-db

# Check tunnel status
./target/release/ssh-tunnel status production-db

# Stop tunnel
./target/release/ssh-tunnel stop production-db

# Delete profile
./target/release/ssh-tunnel delete production-db
```

## Configuration

### Profile Storage

Profiles are stored in `~/.config/ssh-tunnel-manager/profiles/` as TOML files:

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
  - macOS: Keychain
  - Windows: Credential Manager

## Authentication Methods

### SSH Key Authentication

```bash
# Add profile with SSH key
./target/release/ssh-tunnel add myprofile
# When prompted, enter the path to your SSH key
# If the key has a passphrase, you can store it in the keychain
```

### Password Authentication

```bash
# Add profile with password
./target/release/ssh-tunnel add myprofile
# When prompted for SSH key, press Enter to use password auth
# Choose whether to store the password in keychain
```

### Multi-Factor Authentication

The daemon automatically handles complex authentication flows:

```bash
# Server requires publickey + keyboard-interactive (2FA)
./target/release/ssh-tunnel start myprofile
# You'll be prompted for your SSH key passphrase (if needed)
# Then prompted for your 2FA code
```

### Error Messages

Authentication failures now provide detailed information:

```
❌ Password authentication rejected. Server requires: publickey

# Clear message telling you the server doesn't accept passwords
```

```
❌ Public key authentication rejected. Server requires: password, keyboard-interactive

# Shows all methods the server accepts
```

## Development

### Run with Debug Logging

```bash
# Daemon
RUST_LOG=debug cargo run --package ssh-tunnel-daemon

# CLI
RUST_LOG=debug cargo run --package ssh-tunnel-cli -- start myprofile
```

### Build Options

```bash
# Debug build (faster compilation, slower runtime)
cargo build --package ssh-tunnel-cli --package ssh-tunnel-daemon

# Release build (optimized)
cargo build --release --package ssh-tunnel-cli --package ssh-tunnel-daemon

# Run tests
cargo test

# Lint
cargo clippy --all-targets

# Format
cargo fmt --all
```

## API

The daemon exposes a REST API. Default transport is a Unix domain socket; for local testing you can enable the TCP listener on `http://127.0.0.1:3000` (use HTTPS + token if binding beyond localhost):

### Endpoints

- `GET /api/health` - Health check
- `GET /api/tunnels` - List active tunnels
- `POST /api/tunnels/start` - Start a tunnel
- `POST /api/tunnels/stop` - Stop a tunnel
- `GET /api/events` - SSE event stream for real-time updates
- `POST /api/auth/respond` - Submit authentication credentials

### Example API Call

```bash
# Health check
curl http://127.0.0.1:3000/api/health

# Start tunnel
curl -X POST http://127.0.0.1:3000/api/tunnels/start \
  -H "Content-Type: application/json" \
  -d '{"profile_id": "550e8400-e29b-41d4-a716-446655440000"}'

# Subscribe to events
curl -N http://127.0.0.1:3000/api/events
```

## Security

### Authentication & Network Security
- **Authentication Enabled by Default**: `require_auth` defaults to `true` for all daemon modes
- **HTTPS Enforcement**: Non-loopback TCP connections require HTTPS mode (HTTP restricted to localhost only)
- **Configuration Validation**: Daemon validates config at startup to prevent insecure configurations
- **SSH Host Key Verification**: Managed `known_hosts` file with SHA256 fingerprints

### File & Directory Permissions
- **Restrictive Umask**: Set to 0077 at daemon startup to prevent permission leaks
- **File Permissions**: All sensitive files (config, tokens, TLS certs/keys) created with 0600 (owner read/write only)
- **Directory Permissions**:
  - User mode (default): 0700 (owner only)
  - Group mode: 0770 (owner + group) for multi-user system daemons
- **Unix Socket Permissions**:
  - User mode (default): 0600 (owner only)
  - Group mode: 0660 (owner + group)

### Credential Management
- **Keychain Storage**: Passwords encrypted at rest via system keychain
- **SSH Keys**: Referenced by path, never copied or embedded
- **No Credential Leaks**: No credentials in process args, environment, or logs
- **Token Security**: Auth tokens stored with 0600 permissions

### Process Security
- **User Process**: Runs as regular user (no root except for ports ≤1024)
- **Capability-Based**: Uses `CAP_NET_BIND_SERVICE` for privileged ports instead of full root
- **Group Access Control**: Optional group-based access for system daemons with proper user membership validation

## Known Limitations

1. **Forwarding Types**: Only local port forwarding implemented (remote/dynamic pending)
2. **Auto-Reconnect/Health**: Options exist but reconnection/health monitoring isn't wired yet
3. **Platform**: Primary development on Linux; macOS/Windows untested
4. **SSH Agent**: File-based keys only (no ssh-agent integration yet)
5. **Privileged Ports**: Requires `sudo` or `CAP_NET_BIND_SERVICE` for ports ≤1024
6. **Tests**: Some `crates/common` tests are stale and need updates

## Troubleshooting

### Authentication Fails with "Server requires: publickey"

Your profile is configured for password authentication, but the server only accepts SSH keys.

**Solution**: Recreate the profile with an SSH key:
```bash
./target/release/ssh-tunnel delete myprofile
./target/release/ssh-tunnel add myprofile
# Enter your SSH key path when prompted
```

### Can't Bind to Privileged Port (≤1024)

**Error**: `Permission denied binding to 0.0.0.0:443. Port 443 is privileged`

**Solution**: Either run daemon with sudo, or grant capability:
```bash
# Option 1: Run with sudo
sudo RUST_LOG=info ./target/release/ssh-tunnel-daemon

# Option 2: Grant capability (one-time)
sudo setcap cap_net_bind_service=+ep ./target/release/ssh-tunnel-daemon
```

### Keychain Not Working

**Error**: `Failed to store password in keychain`

**Solution**: Ensure a keychain service is running:
```bash
# GNOME
gnome-keyring-daemon --start

# KDE
kwalletd5
```

## Roadmap

See [docs/PROJECT_STATUS.md](docs/PROJECT_STATUS.md) for detailed implementation status and roadmap.

### Next Priorities

1. ✅ ~~Enhanced authentication error messages~~ (Done!)
2. Implement remote port forwarding
3. Implement dynamic (SOCKS) port forwarding
4. Add `edit` command for profiles
5. GUI implementation

## Contributing

This is a personal learning project, but contributions are welcome!

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

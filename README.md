# SSH Tunnel Manager

A secure, performant SSH tunnel management application for Linux with CLI interface and event-driven architecture.

## Status

🟢 **v0.1.8** — Production-ready CLI/Daemon/GUI(GTK) with enhanced error handling
✅ **Full-featured GUI** with profile management, real-time status, and markdown documentation
✅ **Enhanced CLI** with status/restart commands and proactive config validation
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

## Server and Headless Environments

SSH Tunnel Manager works seamlessly on servers and in containers, with automatic keyring fallback.

### Automatic Detection

When running on headless servers or in environments without keyring access:
- The CLI **automatically detects** keyring unavailability
- Profile creation **still succeeds** - passwords just aren't stored
- You'll see: `⚠️  System keychain not available`
- The daemon will **prompt interactively** when starting tunnels

### Manual Override

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

## Quick Start

### Prerequisites

#### Rust Toolchain
- Rust 1.75+ (install via [rustup](https://rustup.rs/))
- Linux (primary development platform)

#### System Dependencies

**For CLI and Daemon only:**
- No additional system dependencies required

**For GTK GUI:**
- GTK4 (≥4.12)
- libadwaita (≥1.5)
- GLib development files

**Installation on Debian/Ubuntu:**
```bash
sudo apt install libgtk-4-dev libadwaita-1-dev build-essential pkg-config
```

**Installation on Fedora:**
```bash
sudo dnf install gtk4-devel libadwaita-devel gcc pkg-config
```

**Installation on Arch:**
```bash
sudo pacman -S gtk4 libadwaita base-devel
```

### Build

```bash
# Clone the repository
git clone https://github.com/SchirmForge/ssh-tunnel-manager.git
cd ssh-tunnel-manager

# Build CLI and daemon only
cargo build --release --package ssh-tunnel-cli --package ssh-tunnel-daemon

# Build everything including GTK GUI
cargo build --release
```

### Basic Usage

#### Using the CLI

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

#### Using the GUI

```bash
# Start the daemon (if not already running)
RUST_LOG=info ./target/release/ssh-tunnel-daemon

# Launch the GTK GUI (in another terminal)
./target/release/ssh-tunnel-gtk
```

The GUI provides:
- Visual profile management with create/edit/delete
- Real-time tunnel status indicators
- Interactive authentication dialogs
- Client and daemon configuration
- Markdown-rendered help documentation

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
ssh-tunnel-manager/
├── crates/
│   ├── common/          # Shared types and utilities (✅ Complete)
│   ├── daemon/          # Core daemon service (✅ Production-ready)
│   ├── cli/             # Command-line interface (✅ Production-ready)
│   ├── gui-core/        # Framework-agnostic GUI business logic (✅ Complete)
│   └── gui-gtk/         # GTK4/Libadwaita desktop application (✅ Functional)
├── docs/                # Documentation and systemd units
├── scripts/             # Installation scripts
├── Cargo.toml           # Workspace configuration
└── README.md
```

**GUI Architecture:**
- `gui-core`: Shared business logic, view models, and validation (~60-70% code reuse)
- `gui-gtk`: GTK4/Libadwaita-specific UI implementation
- Future: `gui-qt` for Qt6 implementation (planned)

## Architecture

### Communication Flow

```
┌─────────────┐                    ┌─────────────┐
│   CLI/GUI   │◄─── SSE Events ────│   Daemon    │
│             │                    │             │
│  Interactive│──── HTTP/REST ────►│ Tunnel Mgmt │
│   Prompts   │                    │   russh     │
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
./target/release/ssh-tunnel info production-db

# Start tunnel (with real-time status)
./target/release/ssh-tunnel start production-db

# Check tunnel status
./target/release/ssh-tunnel status production-db

# Check all tunnel statuses (formatted table)
./target/release/ssh-tunnel status --all

# Restart tunnel (graceful stop → start)
./target/release/ssh-tunnel restart production-db

# Stop tunnel
./target/release/ssh-tunnel stop production-db

# Stop all running tunnels
./target/release/ssh-tunnel stop --all

# Delete profile
./target/release/ssh-tunnel delete production-db
```

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
  - macOS: Keychain (not tested)
  - Windows: Credential Manager (not tested) 

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

### CLI Configuration Missing (401 Unauthorized)

**Error**: `Failed to establish SSE connection: Daemon returned non-success status for events: 401 Unauthorized`

This means your CLI configuration file is missing. The CLI will automatically detect this and offer to copy the daemon-generated config snippet.

**Automatic Solution**: Just run any daemon command and follow the prompts:
```bash
./target/release/ssh-tunnel start myprofile
# You'll be prompted to copy the config snippet automatically
```

**Manual Solution**: Copy the daemon-generated snippet:
```bash
cp ~/.config/ssh-tunnel-manager/cli-config.snippet ~/.config/ssh-tunnel-manager/cli.toml
```

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

### Completed Features ✅

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
- 🚧 **GUI dark mode** - Auto-selection based on system theme preferences
- 🚧 **Daemon management GUI** - Graphical interface for daemon configuration and monitoring

#### Future Enhancements
- Desktop notifications for tunnel status changes
- Auto-reconnect/health monitoring wiring
- Packaging (Flatpak, AUR, deb)

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

# SSH Tunnel Manager - Architecture

## Overview

A secure, performant SSH tunnel management application for Linux with GUI and CLI interfaces, focusing on security, 2FA support, and ease of use.

## Core Principles

1. **Security First**: Memory-safe code, secure credential storage, minimal privilege
2. **Unix Philosophy**: Daemon does one thing well, communicate via well-defined API
3. **User Choice**: Both GUI and CLI interfaces for different workflows
4. **Portability**: Flatpak distribution with all dependencies bundled

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     User Interfaces                          │
├──────────────────────────┬──────────────────────────────────┤
│   GUI (✅ Working)        │      CLI Client (✅ Working)      │
│   ┌──────────────────┐   │      - Interactive prompts        │
│   │ GTK4/Libadwaita  │   │      - Real-time status (SSE)     │
│   │ (gui-gtk)        │   │      - Profile management         │
│   └────────┬─────────┘   │                                   │
│            │             │                                   │
│   ┌────────▼─────────┐   │                                   │
│   │ Framework-       │   │                                   │
│   │ agnostic Core    │   │                                   │
│   │ (gui-core)       │   │                                   │
│   │ 60-70% reuse     │   │                                   │
│   └──────────────────┘   │                                   │
└───────────┬──────────────┴──────────────┬───────────────────┘
            │                              │
            │    HTTP REST API (Commands)  │
            │    ◄─────────────────────────┤
            │                              │
            │    Server-Sent Events (SSE)  │
            │    ──────────────────────────►
            │      (Real-time updates)     │
┌───────────▼──────────────────────────────▼───────────────────┐
│              Daemon Process (✅ Working)                      │
│  ┌──────────────────────────────────────────────────────┐   │
│  │       API Server (Axum on Unix socket by default)     │   │
│  │  - REST endpoints for tunnel control                  │   │
│  │  - SSE event stream (/api/events)                     │   │
│  │  - Event broadcasting system                          │   │
│  └────────────────────┬──────────────────────────────────┘   │
│                       │                                       │
│  ┌────────────────────▼─────────────────────────────────┐   │
│  │         Tunnel Manager (Core Logic)                   │   │
│  │  - Tunnel lifecycle management                        │   │
│  │  - Connection state machine                           │   │
│  │  - Health monitoring & auto-reconnect                │   │
│  │  - Interactive authentication events                  │   │
│  └────────┬────────────────────────────┬─────────────────┘   │
│           │                            │                     │
│  ┌────────▼──────────┐    ┌───────────▼──────────────┐     │
│  │  SSH Connection    │    │  Configuration Manager   │     │
│  │  - russh library   │    │  - TOML profile storage  │     │
│  │  - Multi-factor    │    │  - Validation            │     │
│  │  - Detailed errors │    │  - ~/.config/ssh-...     │     │
│  └────────┬───────────┘    └───────────┬──────────────┘     │
│           │                            │                     │
│  ┌────────▼────────────────────────────▼──────────────┐     │
│  │            Security Layer                           │     │
│  │  - Keyring integration (cross-platform)             │     │
│  │  - SSH key loading with passphrase                  │     │
│  │  - Privileged port handling                         │     │
│  └─────────────────────────────────────────────────────┘     │
└───────────────────────────────────────────────────────────────┘
```

## Component Breakdown

### 1. Daemon (`ssh-tunnel-daemon`)

**Purpose**: Core service that manages SSH tunnels

**Responsibilities**:
- Establish and maintain SSH connections
- Handle authentication (keys, passwords, 2FA)
- Monitor tunnel health and auto-reconnect
- Expose REST API for control
- Emit events for UI updates

**Key Modules**:
- `api`: REST API server (Axum framework)
- `tunnel`: SSH tunnel management
- `config`: Configuration persistence
- `security`: Credential/key management
- `known_hosts`: SSH host key verification ✅ v0.1.3
- `monitor`: Health checks and metrics

**Process Model**:
- Single process, multi-threaded (Tokio async runtime)
- Runs as user service (can be systemd --user in future)
- Listener selection:
  - Default: Unix domain socket in the user runtime dir (owner-only permissions)
  - Local dev: TCP HTTP for testing only
  - Remote/any non-localhost host: TCP HTTPS only (self-signed cert with pinning support)
- Server-Sent Events (SSE) for real-time updates to CLI/GUI (`/api/events`)
- Graceful shutdown with tunnel cleanup

**Implementation Status**: ✅ Fully functional

### 2. GUI (`crates/gui-core` + `crates/gui-gtk`)

**Architecture**: Multi-framework design with shared business logic

**Implementation Status**: ✅ GTK implementation working, Qt planned

#### 2a. GUI Core (`crates/gui-core`)

**Purpose**: Framework-agnostic business logic for all GUI implementations

**Implementation Status**: ✅ Complete

**Features**:
- Profile management: load, save, delete, validate, name checking
- View models: `ProfileViewModel` with formatted display data and status colors
- Application state: `AppCore` with tunnel statuses, daemon connection, auth tracking
- Event handling trait: `TunnelEventHandler` for framework implementations
- Daemon configuration helpers

**Technology**:
- Pure Rust, no UI framework dependencies
- Shared types from `ssh-tunnel-common`
- ~60-70% code reuse across GUI implementations

#### 2b. GTK GUI (`crates/gui-gtk`)

**Purpose**: GNOME/GTK desktop application

**Implementation Status**: ✅ Fully functional

**Features**:
- Connection profile management (create, edit, delete) with validation
- Tunnel start/stop controls with real-time status
- Interactive 2FA/auth prompt handling via dialogs
- Daemon connection monitoring with auto-reconnect
- Client and daemon configuration UI
- Help and About dialogs with markdown rendering
- Real-time status indicators (colored dots)
- Split view navigation between Client/Daemon pages

**Technology**:
- `gtk4-rs`: GTK4 bindings (Rust)
- `libadwaita`: Modern GNOME styling (≥1.5)
- `reqwest`: HTTP client for daemon API
- SSE for real-time updates
- Uses `gui-core` for business logic

#### 2c. Qt GUI (`crates/gui-qt`)

**Purpose**: KDE/Qt6 desktop application with QML

**Implementation Status**: 🚧 Under Development (qmetaobject-rs + Qt6/QML)

**Current State**:
- Basic QML UI with Rust backend using qmetaobject-rs
- AppBackend QObject bridging QML and gui-core
- Placeholder window showing technology stack
- Full functionality being implemented

**Planned Features**:
- All features from gui-core
- Qt6/QML declarative UI
- Native KDE Plasma integration
- Same functionality as GTK version

**Technology**:
- **qmetaobject-rs**: Rust bindings for Qt6
- **QML**: Declarative UI (Qt Quick)
- **gui-core**: Shared business logic (~60-70% code reuse)
- See [crates/gui-qt/README.md](../crates/gui-qt/README.md) for implementation details

#### 2d. GNOME Shell Extension (JavaScript)

**Purpose**: Native GNOME top bar integration

**Implementation Status**: 🔜 Phase 2 (after Config GUI)

**Features**:
- Top bar indicator showing tunnel status
- Dropdown menu with tunnel list and quick actions
- Start/Stop/Restart buttons per tunnel
- Real-time status updates via SSE
- GNOME notifications for events
- Authentication prompts via dialogs
- Reconnection prompts on tunnel drop
- "Open Config GUI" action

**Technology**:
- JavaScript (GNOME Shell extension API requirement)
- SSE client for real-time updates
- GNOME notification system

#### 2e. KDE Plasma Applet (QML + JavaScript)

**Purpose**: Native KDE panel/system tray integration

**Implementation Status**: 🔜 Phase 3 (after GNOME extension)

**Features**:
- System tray icon showing tunnel status
- Popup widget with tunnel list and quick actions
- Start/Stop/Restart buttons per tunnel
- Real-time status updates via SSE
- KDE notifications (KNotification)
- Authentication prompts (KPasswordDialog)
- Reconnection prompts on tunnel drop
- "Open Config GUI" action

**Technology**:
- QML for UI (Plasma applet requirement)
- JavaScript for logic
- KNotification for notifications
- KPasswordDialog for auth prompts

### 3. CLI Client (`ssh-tunnel-cli`)

**Implementation Status**: ✅ Fully functional

**Purpose**: Command-line interface for scripting and power users

**Commands**:
```bash
# Profile management
ssh-tunnel add <name> [options]
ssh-tunnel list
ssh-tunnel edit <name>
ssh-tunnel delete <name>

# Tunnel control
ssh-tunnel start <name>
ssh-tunnel stop <name>
ssh-tunnel restart <name>
ssh-tunnel status [name]

# Daemon control
ssh-tunnel daemon start
ssh-tunnel daemon stop
ssh-tunnel daemon status

# Utilities
ssh-tunnel logs [name]
ssh-tunnel export <name> [file]
ssh-tunnel import <file>
```

**Features**:
- Interactive prompts for 2FA when needed
- JSON output mode for scripting
- Watch mode for status monitoring
- Colored output with progress indicators

## Data Flow

### Connection Establishment

```
User (GUI/CLI)
    │
    ├─→ POST /api/tunnels/{id}/start
    │
Daemon
    │
    ├─→ Load profile from config
    ├─→ Retrieve credentials from keyring
    ├─→ Establish SSH connection
    │   ├─→ Key authentication
    │   └─→ If 2FA required: emit event
    │       │
    │       └─→ GUI/CLI prompts user
    │           │
    │           └─→ POST /api/tunnels/{id}/2fa
    │
    ├─→ Create port forwards
    ├─→ Start health monitoring
    └─→ WebSocket: emit "connected" event
```

### Health Monitoring

```
Daemon (background task)
    │
    └─→ Every 30s for each tunnel:
        │
        ├─→ Check SSH connection alive
        ├─→ Test port forward (optional)
        │
        └─→ If failed:
            ├─→ Emit "disconnected" event
            ├─→ Attempt reconnect (configurable)
            └─→ Emit "reconnected" or "failed" event
```

## Configuration Storage

### Profiles Location
`~/.config/ssh-tunnel-manager/profiles/`

### Profile Format (TOML)
```toml
[profile]
name = "Production Database"
id = "prod-db-001"

[connection]
host = "jump.example.com"
port = 22
user = "myuser"
auth_type = "key"  # or "password"
key_path = "~/.ssh/id_ed25519"  # if auth_type = "key"

[forwarding]
type = "local"  # or "remote", "dynamic"
local_port = 5432
remote_host = "db.internal"
remote_port = 5432

[options]
compression = true
keepalive_interval = 60
auto_reconnect = true
reconnect_attempts = 3
reconnect_delay = 5

[metadata]
created_at = "2025-11-30T10:00:00Z"
modified_at = "2025-11-30T10:00:00Z"
tags = ["database", "production"]
```

### Secrets Storage
- Passwords: Linux Secret Service API (via `secret-service` crate)
- SSH keys: Reference paths, never copy
- Collection: `ssh-tunnel-manager`
- Schema: `org.sshtunnelmanager.Password`

### SSH Host Keys (v0.1.3)
- Location: `~/.config/ssh-tunnel-manager/known_hosts` (default)
- Format: OpenSSH known_hosts format
- Configurable path via `daemon.toml`
- SHA256 fingerprint verification
- Interactive prompts for unknown hosts
- Hard rejection on mismatch

### Daemon State
`~/.local/state/ssh-tunnel-manager/daemon.state` (JSON)
- Active tunnels
- Connection timestamps
- Reconnection attempts
- PID file

## API Design

### REST API (daemon)

Base: `http+unix:///run/user/{uid}/ssh-tunnel-manager/ssh-tunnel-manager.sock` (default). TCP modes are for explicit dev/test (HTTP) or remote access (HTTPS only).

**Client connection policy**
- If target host is `localhost`/loopback → force Unix socket.
- If target host is remote → require HTTPS (with optional fingerprint pinning).
- Plain HTTP is only allowed for local development and should not be exposed publicly.

**Endpoints**:

```
GET    /api/health                    # Daemon health
GET    /api/tunnels                   # List active tunnels
POST   /api/tunnels/{id}/start        # Start tunnel
POST   /api/tunnels/{id}/stop         # Stop tunnel
GET    /api/tunnels/{id}/status       # Tunnel status
GET    /api/tunnels/{id}/auth         # Get pending auth request (if any)
POST   /api/tunnels/{id}/auth         # Submit auth/2FA response

SSE    /api/events                    # Server-Sent Events stream for status/auth updates
```

Planned (not yet implemented): profile CRUD endpoints.

**Event Types** (SSE payloads):
```json
{"type": "starting", "id": "..."}
{"type": "connected", "id": "..."}
{"type": "disconnected", "id": "...", "reason": "..."}
{"type": "error", "id": "...", "error": "..."}
{"type": "auth_required", "id": "...", "request": { ... }}
{"type": "heartbeat", "timestamp": "..."}
```

## Security Considerations

### Credential Handling
1. **Never log credentials**: Sanitize all logging
2. **Memory safety**: Rust guarantees + zero on drop for sensitive data
3. **Keyring integration**: Use system keyring, never custom encryption
4. **SSH key permissions**: Verify 0600/0400 before use
5. **API authentication**: Unix socket permissions (0600)

### Process Security
1. **Minimal privileges**: Run as user, not root
2. **Sandbox**: Flatpak sandbox with minimal permissions
3. **IPC security**: Unix socket with owner-only permissions
4. **No password in process args**: Always via API or stdin

### Attack Surface Reduction
1. **No network exposure**: Prefer Unix socket; TCP only with HTTPS and auth when explicitly enabled
2. **Input validation**: All API inputs validated
3. **Resource limits**: Max connections, rate limiting
4. **Dependency audit**: Regular `cargo audit`

## Dependency Strategy

### Core Dependencies
- `tokio`: Async runtime
- `axum`: REST API framework
- `russh` or `thrussh`: SSH protocol
- `serde`: Serialization
- `toml`: Config format
- `secret-service`: Keyring integration
- `anyhow`/`thiserror`: Error handling

### GUI Dependencies
- `gtk4-rs`: GTK4 bindings
- `libadwaita`: GNOME styling
- `ksni`: System tray
- `reqwest`: HTTP client
- `tokio-tungstenite`: WebSocket

### CLI Dependencies
- `clap`: CLI parsing
- `dialoguer`: Interactive prompts
- `indicatif`: Progress bars
- `colored`: Terminal colors

## Development Phases

### Phase 1: Core Daemon (MVP)
- [x] Project setup
- [ ] Configuration management
- [ ] SSH connection (key auth only)
- [ ] Basic tunnel management
- [ ] REST API (start/stop/status)

### Phase 2: Security & Robustness
- [ ] Keyring integration
- [ ] 2FA support
- [ ] Health monitoring
- [ ] Auto-reconnect
- [ ] Error handling

### Phase 3: CLI Client
- [ ] Basic commands (add, list, start, stop)
- [ ] Interactive 2FA prompts
- [ ] Status display
- [ ] Import/export

### Phase 4: GUI Client
- [ ] Main window with connection list
- [ ] Profile editor
- [ ] Connection controls
- [ ] Real-time status updates

### Phase 5: Polish & Distribution
- [ ] System tray integration
- [ ] Notifications
- [ ] Systemd service
- [ ] Flatpak packaging
- [ ] Documentation

## Testing Strategy

### Unit Tests
- Configuration parsing/validation
- State machine transitions
- API request/response handling
- Error scenarios

### Integration Tests
- Full tunnel lifecycle
- 2FA workflow
- Auto-reconnect
- Multiple simultaneous tunnels

### Manual Testing
- Different SSH server configurations
- Network interruption scenarios
- GUI responsiveness
- System tray behavior

### Security Testing
- Credential storage audit
- Process inspection (no secrets in memory dumps)
- Filesystem permissions
- API authorization

## Flatpak Packaging

### Manifest Structure
```yaml
app-id: com.github.SchirmForge.SSHTunnelManager
runtime: org.gnome.Platform
runtime-version: '47'
sdk: org.gnome.Sdk
sdk-extensions:
  - org.freedesktop.Sdk.Extension.rust-stable

command: ssh-tunnel-gui

finish-args:
  - --share=network          # SSH connections
  - --share=ipc              # GUI
  - --socket=wayland         # GUI
  - --socket=fallback-x11    # GUI fallback
  - --filesystem=~/.ssh:ro   # SSH keys (read-only)
  - --talk-name=org.freedesktop.secrets  # Keyring
  - --own-name=com.github.SchirmForge.SSHTunnelManager  # D-Bus

modules:
  - name: ssh-tunnel-manager
    buildsystem: simple
    build-commands:
      - cargo build --release
      - install -Dm755 target/release/ssh-tunnel-daemon /app/bin/
      - install -Dm755 target/release/ssh-tunnel-gui /app/bin/
      - install -Dm755 target/release/ssh-tunnel-cli /app/bin/
```

## Future Enhancements (Post-MVP)

- Jump host/bastion support (multi-hop)
- SOCKS proxy mode
- SSH agent forwarding
- Connection profiles sync (encrypted)
- Statistics and usage tracking
- Tunnel templates
- Concurrent tunnel limit configuration
- VPN integration hints

---

## Next Steps

1. Set up Rust workspace with three crates
2. Implement configuration types and validation
3. Build SSH connection manager
4. Create minimal REST API
5. Develop CLI client for testing
6. Implement GUI client
7. Package as Flatpak

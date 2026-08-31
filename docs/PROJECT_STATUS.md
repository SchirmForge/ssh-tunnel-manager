# Project Status

## Current State

**Version**: v0.1.11
**Status**: ✅ Production-ready CLI, daemon and GTK GUI with a full REST + SSE architecture
**Release date**: 2026-08-30

A snapshot of what exists today. For what is planned see [ROADMAP.md](ROADMAP.md); for what
shipped when see [CHANGELOG.md](CHANGELOG.md); for behaviour described from the user's point
of view see the [user stories](user-stories/).

| Area | State |
|---|---|
| Daemon | ✅ Local port forwarding, interactive authentication over SSE, host key verification, three listener modes |
| CLI | ✅ Profile CRUD, tunnel control, status, watch |
| GTK GUI | ✅ Full profile CRUD, live status, first-launch wizard, remote daemon support |
| Testing | ✅ Two tiers (hermetic + live SSH), sandboxed, plus CI |
| Auto-reconnect | ❌ Config options exist but nothing acts on them |
| Notifications | ❌ Not implemented |

## What’s Implemented

### ✅ Common (`crates/common`)
- Typed configs (`Profile`, `ConnectionConfig`, `ForwardingConfig`, `TunnelOptions`) with validation and TOML persistence.
- `PasswordStorage` enum (keychain/file/none) with backward-compatible deserialization from boolean values.
- Keychain module with availability detection (`is_keychain_available()`) and storage operations.
- Shared types for auth flows (`AuthType`, `AuthRequest`, `TunnelStatus`, events).
- Daemon client helpers (reqwest setup, auth header, TLS pinning helpers, socket path auto-detection).
- **Config validation helpers** - `validate_daemon_config()`, `get_cli_config_snippet_path()`, `cli_config_snippet_exists()` for proactive validation.
- **SSE-first tunnel control flow** (`start_tunnel_with_events`, `stop_tunnel`) with event handler trait for shared CLI/GUI logic.

### ✅ Daemon (`crates/daemon`)
- SSH tunnel lifecycle using russh; interactive auth via SSE-driven prompts (password, key passphrase, keyboard-interactive/2FA).
- Local forwarding fully working; privileged-port error messaging.
- Host key verification with OpenSSH-format `known_hosts`, SHA256 fingerprints, and 0600 perms.
- API server (Axum): health, tunnel start/stop/status, pending-auth get/post, SSE events.
- TLS self-signed cert generation and fingerprint display for HTTPS mode.
- PID file guard to avoid duplicate instances.

### ✅ CLI (`crates/cli`)
- Profile CRUD (add/list/show/delete/info) with interactive prompts and non-interactive flags.
- Keychain integration for passwords/passphrases with graceful fallback for headless environments.
- Automatic keyring availability detection - profile creation succeeds even when keyring unavailable.
- Tunnel control: start/stop/restart/status with `--all` flag support.
- **Proactive daemon config validation** - checks config before connection attempts with interactive snippet copy.
- Table/JSON output, colorized UX, validation of key permissions and privileged ports.
- Start/stop/status using **shared SSE-first flow** from common module; interactive auth handling.

### ✅ GUI Core (`crates/gui-core`)
- Framework-agnostic business logic for GTK and future presentation adapters
- Profile management: `load_profiles`, `save_profile`, `delete_profile`, `validate_profile`, `profile_name_exists`
- View models: `ProfileViewModel` with formatted display data, status colors, and action states
- Shared `AppController`, typed commands/effects, immutable snapshots, and code-derived action availability
- FIFO authentication queue and typed answers driven only by structured daemon request/status codes
- Versioned GUI preferences for profile order, pins, filters, and sorting in `ui.toml`
- Compatibility state: `AppCore` remains available to the current GTK implementation
- Event handling trait: `TunnelEventHandler` for framework-agnostic event notifications
- Daemon helpers: `load_daemon_config`, configuration path utilities

### ✅ GUI GTK (`crates/gui-gtk`)
- Libadwaita/GTK4 application with functional start/stop using **shared SSE-first flow** from common
- GTK event handler utilities implementing centralized event processing with AppCore integration
- Uses `start_tunnel_with_events` and `stop_tunnel` helpers from common module
- Integrates gui-core for profile management, validation, and view models
- **Profile management UI** - Full CRUD with shared common crate functions
- Complete feature list below
  - Create, edit, delete profiles via unified dialog interface
  - "New Profile" button on profiles list page
  - Edit/Delete buttons on profile details page
  - Duplicate name validation and proper overwrite handling
  - Auto-refresh after all CRUD operations
  - Auto-navigation back to list after edit/delete
- **Profile editor dialog** - GNOME Settings-style interface
  - Organized sections: Basic Info, Authentication, Port Forwarding, Advanced Tuning
  - Improved passphrase UX: "Store in Keychain" switch before password entry, password field only visible when storing
  - SSH key validation with permission checks and passphrase verification
  - Advanced options in collapsible accordion (compression, keepalive, packet sizes, window size, auto-reconnect settings)
  - Sensible defaults: ed25519 keys, 8080→80 port forwarding
  - ESC key to close, proper window titles ("New Profile"/"Edit Profile")
  - File chooser for SSH keys with filters
  - All switches properly styled with vertical alignment and activatable rows
- **Real-time status indicators**: Colored dots on profile list (green/orange/red/gray) showing connection status.
- **Daemon connection monitoring**: Network icon with tooltip showing daemon availability, automatic reconnection with exponential backoff, heartbeat-based timeout detection (30s).
- **SSE event integration**: All tunnel events update profile status dots in real-time, with initial status query on connection.
- **Navigation UI**: Split view with sidebar navigation between Profiles and Daemon pages, burger menu with Help/About.
- **Help and About dialogs**: Markdown-rendered documentation accessible from burger menu using `pulldown-cmark`.


## Current Capabilities

✅ Create profiles and store credentials in system keychain  
✅ Connect with key, password, or keyboard-interactive (2FA)  
✅ Verify SSH host keys and prompt on first connect  
✅ Local port forwarding with real-time status via SSE  
✅ Interactive auth prompts (password, key passphrase, 2FA)  

✅ GUI with SSE-first tunnel control and auth dialogs
✅ Real-time status indicators (colored dots) on profile list
✅ Daemon connection monitoring: the GUI reconnects its own SSE stream with backoff and
   detects a missing heartbeat (this is the client reattaching to the daemon — it is not
   SSH tunnel auto-reconnect, which is not implemented)
✅ Initial status query on connection/reconnection
✅ Help and About dialogs with markdown rendering
✅ Full profile CRUD UI (create/edit/delete) with validation
✅ Profile dialog with advanced options accordion
✅ GNOME Settings-style UI with proper switch styling
✅ DEB packaging

✅ Sandboxed test suite (hermetic tier plus a live SSH tier), CI, clippy clean

❌ Remote forwarding — [not planned](ROADMAP.md#not-planned)
❌ Dynamic/SOCKS forwarding — [future, unscheduled](ROADMAP.md#later)
❌ SSH tunnel auto-reconnect / health monitoring — options exist but nothing acts on them;
   [design decided](ROADMAP.md#auto-reconnect-and-health-monitoring), not implemented
❌ System tray — crate removed in v0.1.11; to be rewritten if wanted
❌ Desktop notifications — [planned for v0.2.0](ROADMAP.md#next)
❌ Packaging: Flatpak, AUR ([RPM in progress](ROADMAP.md#packaging))

## Security Notes

- Prefer **Unix socket**; if TCP is enabled, use **HTTPS + token auth** and keep bind address restricted.
- **Authentication is enabled by default**: `require_auth` defaults to `true` for all modes.
- **Non-loopback connections require HTTPS**: HTTP mode (`tcp-http`) is restricted to loopback addresses (127.x.x.x or localhost) only. Network addresses (0.0.0.0, 192.168.x.x, etc.) require `tcp-https` mode.
- **File permissions hardening**: All sensitive files (config, token, TLS certs/keys) are created with 0600 permissions.
- **Directory and socket permissions**:
  - Default (single-user): runtime directory 0700, Unix socket 0600 (owner only)
  - Group access mode: runtime directory 0770, Unix socket 0660 (owner + group)
- **Restrictive umask**: Daemon sets umask to 0077 at startup to prevent permission leaks from parent process.
- Configuration validation prevents insecure daemon configurations at startup.
- Token is generated to disk; avoid logging or exposing it in CLI output.
- Host keys are verified and stored in `~/.config/ssh-tunnel-manager/known_hosts`.
- Credentials remain in OS keyring; SSH keys are referenced by path only.

## Quick Commands

```bash
# Build CLI + daemon
cargo build --package ssh-tunnel-cli --package ssh-tunnel-daemon

# Release build (whole workspace)
cargo build --release

# Run with logs
RUST_LOG=debug cargo run --package ssh-tunnel-daemon

# Start a tunnel (prompts via SSE)
ssh-tunnel start <profile>

# Tests - hermetic; no network, no credentials
make test

# Live SSH tests (needs .local/testing/ssh-target.env; skips without it)
make test-live

# Disposable sandbox with a daemon and seeded profiles
make sandbox ARGS="--profiles key,password,2fa"

# Everything CI runs
make check
```

See [DEVELOPMENT.md](DEVELOPMENT.md#testing) for the full testing guide.

## Related documents

| Document | Purpose |
|---|---|
| [ROADMAP.md](ROADMAP.md) | What is planned, what is not, and design decisions taken in advance |
| [CHANGELOG.md](CHANGELOG.md) | Version history |
| [KNOWN_ISSUES.md](KNOWN_ISSUES.md) | Current defects and limitations |
| [user-stories/](user-stories/) | What the product does, by epic, with implementation status |
| [architecture/](architecture/) | Functional and technical specification |

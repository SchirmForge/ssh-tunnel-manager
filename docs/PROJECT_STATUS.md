# Project Status

## Current State

**Version**: v0.1.8
**Status**: ✅ Production-ready CLI/Daemon/GUI with enhanced error handling and tunnel management  

- CLI and daemon work end-to-end for **local port forwarding** with interactive auth.  
- **SSE** powers real-time updates (`/api/events`); REST covers start/stop/status/auth.  
- **SSH host key verification** is implemented with a managed `known_hosts` file.  
- HTTP/TCP mode exists for local testing only; **Unix socket by default** and **HTTPS required for remote hosts** (token auth recommended).

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
- Framework-agnostic business logic shared across GTK and future Qt implementations (~60-70% code reuse)
- Profile management: `load_profiles`, `save_profile`, `delete_profile`, `validate_profile`, `profile_name_exists`
- View models: `ProfileViewModel` with formatted display data, status colors, and action states
- Application state: `AppCore` with profiles, tunnel statuses, daemon connection state, auth tracking
- Event handling trait: `TunnelEventHandler` for framework-agnostic event notifications
- Daemon helpers: `load_daemon_config`, configuration path utilities

### ✅ GUI GTK (`crates/gui-gtk`)
- Libadwaita/GTK4 application with functional start/stop using **shared SSE-first flow** from common
- GTK event handler utilities implementing centralized event processing with AppCore integration
- Uses `start_tunnel_with_events` and `stop_tunnel` helpers from common module
- Integrates gui-core for profile management, validation, and view models
- **Profile management UI** - Full CRUD with shared common crate functions
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
✅ Daemon connection monitoring with auto-reconnect and heartbeat timeout
✅ Initial status query on connection/reconnection
✅ Help and About dialogs with markdown rendering
✅ Full profile CRUD UI (create/edit/delete) with validation
✅ Profile dialog with advanced options accordion
✅ GNOME Settings-style UI with proper switch styling

❌ Remote forwarding
❌ Dynamic/SOCKS forwarding
❌ Auto-reconnect/health monitoring (options exist but not wired)
❌ System tray/notifications/systemd integration
❌ Packaging (Flatpak/AUR/deb)
❌ Stale tests in `crates/common` need fixing

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

## Roadmap & TODO Tracking

### Recently Completed ✅

- ✅ **Enhanced 401 authentication error handling** (v0.1.8) - Proactive config validation with interactive snippet copy
  - Config validation before daemon connection attempts prevents confusing 401 errors
  - Interactive prompt to copy daemon-generated config snippet when missing
  - Common validation helpers in `ssh_tunnel_common::daemon_client` for reuse in GUI
  - Comprehensive step-by-step error messages for authentication failures
  - All daemon commands now call `ensure_daemon_config()` before connecting
- ✅ **CLI status command** (v0.1.8) - Display tunnel connection status
  - Single tunnel status with detailed information
  - `--all` flag for formatted table of all active tunnels
  - Color-coded status indicators
- ✅ **CLI restart command** (v0.1.8) - Graceful tunnel restart with two-step stop→start process
- ✅ **Keyring graceful fallback for headless environments** (v0.1.7) - Automatic detection and graceful fallback when system keyring unavailable
  - Test-based availability detection using actual keyring operations (no environment variable guessing)
  - CLI profile creation succeeds even when keyring unavailable with clear warnings
  - `PasswordStorage` enum migration from boolean for future extensibility (keychain/file/none)
  - `SSH_TUNNEL_SKIP_KEYRING` environment variable for explicit override
  - GUI passphrase field reorganized: "Store in Keychain" switch before password entry, password only visible when storing
  - Comprehensive documentation in SYSTEMD.md and README.md for server deployments
  - Backward compatible with existing profile TOML files (boolean → enum conversion)
- ✅ **CLI stop --all command** (v0.1.7) - Stop all active tunnels with status checking from daemon
- ✅ **IPv6 host management** (v0.1.7) - Proper URL formatting with `[addr]:port` notation for IPv6 literals
- ✅ **Tunnel description formatting** (v0.1.7) - Unified display across CLI/GUI with `local:`/`remote:` labels
- ✅ **Enforce auth by default for TCP modes** - Authentication enabled by default, HTTP restricted to loopback
- ✅ **Share CLI's SSE-first start/stop flow with GUI** - Extracted to `daemon_client::start_tunnel_with_events`
- ✅ **Integrate shared SSE-first flow into GUI** - GTK event handler implements `TunnelEventHandler` trait
- ✅ **Enhance GUI status updates** - Real-time colored status dots on profile list
- ✅ **Add Help and About windows** - Markdown-rendered documentation in burger menu

### High Priority 🚧

#### Remote Port Forwarding (`ssh -R`)
- Status: **Planned** - Infrastructure ready, implementation needed
- Files: See `~/.claude/plans/remote-forwarding-implementation.md`
- Components:
  - 🚧 Daemon: Implement `run_remote_forward_task()` using russh reverse forwarding API
  - 🚧 CLI: Add `--forwarding-type` argument for profile creation
  - 🚧 GUI: Add forwarding type dropdown in profile dialog
- Estimated effort: 12-19 hours

#### Dynamic/SOCKS Proxy (`ssh -D`)
- Status: **Planned** - Similar to remote forwarding
- Description: SOCKS5 proxy for dynamic port forwarding
- Components:
  - 🚧 Daemon: Implement `run_dynamic_forward_task()` with SOCKS5 protocol handling
  - 🚧 CLI: Support `--forwarding-type dynamic`
  - 🚧 GUI: Add to forwarding type dropdown

#### Configurable Daemon Config Path
- Status: **Planned**
- Description: Pass daemon config file as command-line parameter
- Default: `~/.config/ssh-tunnel-manager/daemon.toml`
- Files: `crates/daemon/src/main.rs`, `crates/daemon/src/config.rs`
- Benefits: Multi-instance daemons, testing, system-wide configs

#### Enhanced Logging
- Status: **Planned - Design decision needed**
- Description: Daemon logging with `--debug` option and configurable log levels
- Options to consider:
  1. **journalctl integration** (systemd) - Best for system services
  2. **Dedicated log files** - Better for debugging, log rotation needed
  3. **Hybrid approach** - Both journalctl and optional file output
- Questions:
  - Default log level? (Info, Debug, Trace)
  - Rotation policy for file-based logs?
  - Structured logging (JSON) for parsing?

### Medium Priority 🔵

#### GUI Dark Mode
- Status: **Planned**
- Description: Auto-selection based on system theme preferences
- Implementation: Use GTK4 `AdwStyleManager` to detect and follow system theme
- Files: `crates/gui-gtk/src/main.rs`, `crates/gui-gtk/src/ui/window.rs`

#### Daemon Management GUI
- Status: **Partially planned**
- Description: Graphical interface for daemon configuration and monitoring
- Features:
  - Show daemon configuration (from running daemon using API, not file)
  - Restart/stop/start daemon (user-only operations)
  - Configure autostart for user (systemctl command integration)
  - Configure profiles that should autostart (via daemon API)
- Files: `crates/gui-gtk/src/ui/daemon_page.rs` (new)

### Future Enhancements 📅

#### Auto-Reconnect/Health Monitoring
- Status: **Config exists, wiring needed**
- Description: Wire `auto_reconnect`/monitoring to actual reconnection and health checks
- Current: Options exist in profile config but not implemented
- Files: `crates/daemon/src/tunnel.rs`

#### Desktop Notifications
- Status: **Planned**
- Description: System notifications for tunnel status changes
- Library: `notify-rust` (already in dependencies)
- Events: Connected, Disconnected, Failed, Authentication Required

#### System Integration
- Status: **Partial** - systemd units exist, tray/notifications pending
- Components:
  - ✅ Systemd user service templates
  - 🚧 System tray integration
  - 🚧 Desktop notifications
  - 🚧 Autostart on login

#### Packaging
- Status: **Planned**
- Targets:
  - 🚧 Flatpak (manifest needed)
  - 🚧 AUR (PKGBUILD needed)
  - 🚧 Debian package (.deb)
  - 🚧 AppImage

### Known Issues / Technical Debt 🔧

- ❌ Fix outdated tests in `crates/common` (profile manager schema drift)
- ❌ Clarify token handoff so CLI can consume it without logging secrets
- 🚧 Remote forwarding not implemented (returns error)
- 🚧 Dynamic/SOCKS forwarding not implemented (returns error)

## Quick Commands

```bash
# Build CLI + daemon
cargo build --package ssh-tunnel-cli --package ssh-tunnel-daemon

# Release build
cargo build --release --package ssh-tunnel-cli --package ssh-tunnel-daemon

# Run with logs
RUST_LOG=debug cargo run --package ssh-tunnel-daemon

# Start a tunnel (prompts via SSE)
ssh-tunnel start <profile>

# Tests (fix pending failures in common tests)
cargo test
```

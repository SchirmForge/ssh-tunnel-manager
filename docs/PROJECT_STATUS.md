# Project Status

## Current State

**Version**: v0.1.6
**Status**: ✅ Production-ready CLI/Daemon/GUI with comprehensive security hardening  

- CLI and daemon work end-to-end for **local port forwarding** with interactive auth.  
- **SSE** powers real-time updates (`/api/events`); REST covers start/stop/status/auth.  
- **SSH host key verification** is implemented with a managed `known_hosts` file.  
- HTTP/TCP mode exists for local testing only; **Unix socket by default** and **HTTPS required for remote hosts** (token auth recommended).

## What’s Implemented

### ✅ Common (`crates/common`)
- Typed configs (`Profile`, `ConnectionConfig`, `ForwardingConfig`, `TunnelOptions`) with validation and TOML persistence.
- Shared types for auth flows (`AuthType`, `AuthRequest`, `TunnelStatus`, events).
- Daemon client helpers (reqwest setup, auth header, TLS pinning helpers, socket path auto-detection).
- **SSE-first tunnel control flow** (`start_tunnel_with_events`, `stop_tunnel`) with event handler trait for shared CLI/GUI logic.

### ✅ Daemon (`crates/daemon`)
- SSH tunnel lifecycle using russh; interactive auth via SSE-driven prompts (password, key passphrase, keyboard-interactive/2FA).
- Local forwarding fully working; privileged-port error messaging.
- Host key verification with OpenSSH-format `known_hosts`, SHA256 fingerprints, and 0600 perms.
- API server (Axum): health, tunnel start/stop/status, pending-auth get/post, SSE events.
- TLS self-signed cert generation and fingerprint display for HTTPS mode.
- PID file guard to avoid duplicate instances.

### ✅ CLI (`crates/cli`)
- Profile CRUD (add/list/show/delete) with interactive prompts and non-interactive flags.
- Keychain integration for passwords/passphrases.
- Start/stop/status using **shared SSE-first flow** from common module; interactive auth handling.
- Table/JSON output, colorized UX, validation of key permissions and privileged ports.

### ✅ GUI (`crates/gui`)
- Libadwaita/GTK4 application with functional start/stop using **shared SSE-first flow**.
- GTK event handler (`GtkTunnelEventHandler`) implements `TunnelEventHandler` trait for auth dialogs and status updates.
- Uses `start_tunnel_with_events` and `stop_tunnel` helpers from common module.
- **Profile management UI** - Full CRUD with shared common crate functions
  - Create, edit, delete profiles via unified dialog interface
  - "New Profile" button on profiles list page
  - Edit/Delete buttons on profile details page
  - Duplicate name validation and proper overwrite handling
  - Auto-refresh after all CRUD operations
  - Auto-navigation back to list after edit/delete
- **Profile editor dialog** - GNOME Settings-style interface
  - Organized sections: Basic Info, Authentication, Port Forwarding, Advanced Tuning
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

## Known Gaps / TODO

- Implement remote and dynamic/SOCKS forwarding.
- Wire `auto_reconnect`/monitoring to actual reconnection and health checks.
- ✅ ~~Enforce auth by default for TCP modes; treat HTTP as dev-only~~ - **Done!** Authentication enabled by default, HTTP restricted to loopback.
- Fix outdated tests in `crates/common` (profile manager schema drift).
- ✅ ~~Share the CLI's SSE-first start/stop flow with the GUI~~ - **Done!** Extracted to `daemon_client::start_tunnel_with_events`.
- ✅ ~~Integrate shared SSE-first flow into GUI~~ - **Done!** GTK event handler implements `TunnelEventHandler` trait.
- ✅ ~~Enhance GUI status updates (add status labels, progress indicators)~~ - **Done!** Real-time colored status dots on profile list.
- ✅ ~~Add Help and About windows~~ - **Done!** Markdown-rendered documentation in burger menu.
- Daemon page in GUI: show daemon configuration (from running daemon using API, not from file), restart daemon (user only), stop daemon (user only), start daemon (user only), configure autostart for user (running a systemctl command), configure profiles that should autostart (via API).
- System integration: systemd user service, tray, notifications.
- Packaging: Flatpak manifest and build pipeline.
- Clarify token handoff so the CLI can consume it without logging secrets.

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

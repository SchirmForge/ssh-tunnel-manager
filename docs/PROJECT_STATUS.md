# Project Status

## Current State

**Version**: v0.1.10
**Status**: ✅ Production-ready CLI/Daemon/GUI with full REST API architecture

**Release Date**: 2026-01-02

### v0.1.10 Release Highlights

**Bug Fixes (Completed):**
- ✅ **Authentication retry on failed 2FA** - Fixed keyboard-interactive retry flow (daemon and GUI)
  - **Daemon**: Properly detects when server allows retry (checks `remaining_methods`)
  - **Daemon**: Starts new keyboard-interactive session if server permits
  - **Daemon**: Respects server's retry policy (no artificial client-side limits)
  - **GUI**: SSE-driven dialog state eliminates race conditions
  - **GUI**: Dialog stays open showing "Verifying..." until SSE confirms next state
  - User gets re-prompted for 2FA/password until server accepts or permanently rejects

- ✅ **Language-independent error detection** - Daemon works correctly on non-English systems
  - **Hyper error categorization**: Uses type-based error inspection (`is_timeout()`, `is_parse()`, etc.)
  - **Encrypted key detection**: Uses `russh::keys::Error` enum matching instead of string comparison
  - **Keyboard-interactive prompts**: Server's prompt text passed as-is (supports non-English SSH servers)
  - **IO error detection**: Checks `ErrorKind` enum variants instead of error messages
  - Works correctly regardless of system locale

- ✅ **SSE client consolidation** - Moved from gui-core to common crate
  - Single source of truth for both CLI and GUI
  - Framework-agnostic EventListener shared across all frontends
  - Removed duplicate TunnelEvent definitions

- ✅ **Improved daemon error logging** - Better diagnostics for HTTP connection errors
  - Intelligent categorization: ClientDisconnect, SseStreamClose, NetworkError, ProtocolError, ServerError
  - SSE stream disconnects moved from ERROR to DEBUG level (reduces log noise)
  - Structured logging with error source, type, and actionable hints
  - HTTP request tracing middleware added for context

**Documentation (Completed):**
- ✅ **Updated GUI About and Help dialogs**
  - Content moved to `crates/gui-core/assets/` for framework-agnostic sharing
  - About dialog updated with v0.1.9 features
  - Help dialog includes comprehensive remote daemon setup guide
  - Copyright updated to SchirmForge, correct GitHub URLs

**Current cycle (post-v0.1.10): stability, not features**
- ✅ Sandboxed, isolated test suite: a hermetic tier that always runs and a live SSH tier
  covering every authentication flow
- ✅ CI from nothing: fmt, `clippy -D warnings`, tests, release build
- ✅ Removed the orphaned `crates/tray` (outside the workspace build since v0.1.6)
- ✅ Fixed a lock held across an await in `TunnelManager::stop` that made every
  cancellation during auth force-abort the tunnel task
- 🚧 Bug fixing driven by what the live tier turns up

**Future Enhancements (Planned for v0.2.x):**
- 🚧 **NEW FEATURE - GUI Notification System** - Desktop notifications for tunnel connection events
  - Connected notifications
  - Disconnected notifications
  - Error notifications
  - System tray integration (optional)

- 🚧 **Adaptive Authentication Dialogs** - Better UX for authentication prompts
  - Dynamic dialog sizing based on text content
  - Better readability for long prompts

- 🚧 **User Manual** - Improve user documentation
  - Getting started guide
  - Troubleshooting tips
  - Screenshots and examples

---

## Released Features (v0.1.9)  

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

### 🚧 GUI Qt (`crates/gui-qt`)
- **Qt6/QML skeleton** using cxx-qt; launches and lands on About page with a skeleton notice
- Profiles page uses static placeholder data; daemon/event wiring not yet implemented
- Bridges QML declarative UI with gui-core business logic (planned; wiring pending)
- **Technology**: cxx-qt + Qt6 + QML (Qt Quick)
- **Architecture**: QML for UI, Rust for logic, gui-core for ~60-70% code reuse (planned)
- **Status**: **Does not currently compile** - the cxx-qt 0.8.0 bridge macro fails to
  parse (see [crates/gui-qt/README.md](../crates/gui-qt/README.md)). It is therefore
  excluded from `default-members` in the root `Cargo.toml`: `cargo build` skips it, while
  `cargo build -p ssh-tunnel-gui-qt` still works for anyone with Qt6 installed. Use the
  GTK GUI meanwhile.

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
✅ DEB packaging

✅ Sandboxed test suite (hermetic tier plus a live SSH tier), CI, clippy clean

❌ Remote forwarding (not planned)
❌ Dynamic/SOCKS forwarding (future, unscheduled)
❌ Auto-reconnect/health monitoring (options exist but not wired; design decided below)
❌ System tray (crate removed in the stabilisation pass; to be rewritten if wanted)
❌ Desktop notifications
❌ Packaging (Flatpak/AUR)

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

- ✅ **Remote daemon profile support** (v0.1.9) - Profiles work with HTTP/HTTPS remote daemons
  - New `ProfileSourceMode` enum: Local (filesystem), Hybrid (API + daemon filesystem), Remote (future)
  - `StartTunnelRequest` sent via API includes profile data for remote daemon compatibility
  - SSH private keys remain secure on daemon filesystem - never sent over network
  - Enhanced error messages show daemon's actual SSH directory paths instead of generic `~/.ssh`
  - Daemon calculates and reports SSH directory via `DaemonInfo.ssh_key_dir` API field
  - Simplified SSH key setup instructions - removed specific scp/chmod commands
  - Added ssh-agent recommendation for encrypted keys
- ✅ **SSH Key Setup Warning opt-out** (v0.1.9) - User-controllable warning dialog
  - "Don't show this again" checkbox on SSH key setup warning dialog
  - Preference persists in `cli.toml` as `skip_ssh_setup_warning` field
  - Respects user choice across application restarts
- ✅ **Daemon settings improvements** (v0.1.9) - Better UI for remote daemon scenarios
  - Hides "Restart Daemon" button when using HTTPS mode (remote daemon)
  - Prevents confusion about local-only daemon operations
  - Restart row only shown for unix-socket mode
- ✅ **Debug logging migration** (v0.1.9) - Proper structured logging
  - Converted all `eprintln!` debug output to `tracing::debug!` and `tracing::warn!`
  - Consistent logging framework across all GUI code
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

#### Packaging
- Status: **in progress**
- Targets:
  - ✅ DEB = Done
  - 🚧 RPM = in progress
  - 🚧 AUR (PKGBUILD needed)
  - 🚧 Flatpak (to be confirned)

#### Stability and Regression Testing (current focus)
- Status: **In progress**
- Description: Automated test coverage, sandboxed test environments, and bug fixing,
  rather than new features
- Delivered:
  - ✅ Sandbox isolation via `XDG_CONFIG_HOME`/`XDG_RUNTIME_DIR` (`scripts/sandbox.sh`)
  - ✅ Tier-1 integration tests against a real daemon (`crates/daemon/tests/daemon_api.rs`)
  - ✅ Tier-2 live SSH tests covering every auth flow (`crates/daemon/tests/live_ssh.rs`)
  - ✅ `scripts/dev-env.sh` for one-command manual/GUI testing
  - ✅ CI (`.github/workflows/ci.yml`); `cargo clippy -- -D warnings` clean
  - ✅ Removed the orphaned `crates/tray`
- Remaining:
  - 🚧 Fix whatever the live tier turns up once the test accounts exist
  - 🚧 Decide whether `AUTH_RESPONSE_TIMEOUT` (60s) is the right unattended value

#### Remote Port Forwarding (`ssh -R`)
- Status: ❌ **Not planned.** Dropped from the roadmap - no use case has come up. Use
  `ssh -R` directly. `ForwardingType::Remote` remains in the enum because it is
  serialised in existing profile TOML, but no implementation is intended.

#### Dynamic/SOCKS Proxy (`ssh -D`)
- Status: **Future, unscheduled**
- Description: SOCKS5 proxy for dynamic port forwarding
- Components:
  - 🚧 Daemon: Implement `run_dynamic_forward_task()` with SOCKS5 protocol handling
  - 🚧 CLI: Support `--forwarding-type dynamic`
  - 🚧 GUI: Add to forwarding type dropdown
  - Note: `ForwardingType::Local` is currently hardcoded at profile creation in both
    clients (`crates/cli/src/main.rs`, `crates/gui-gtk/src/ui/profile_dialog.rs`), so
    this is not a daemon-only change

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

#### Daemon Management GUI
- Status: **Partially planned**
- Description: Graphical interface for daemon configuration and monitoring
- Features:
  - Show daemon configuration (from running daemon using API, not file)
  - Restart/stop/start daemon (user-only operations)
  - Configure autostart for user (systemctl command integration)
  - Configure profiles that should autostart (via daemon API)
- Files: `crates/gui-gtk/src/ui/daemon_page.rs` (new)

#### Client using multiple daemon connections (GUI/CLI)
- Status: **Partially planned**
- Description: Allow users to have more than one cli.toml 
- Features:
  - By default, ~/.config/ssh-tunnel-manager/cli.toml is used, but user can select another configuration in the Client configuration page
  - Create a *preferences* file in ~/.config/ssh-tunnel-manager/ where to store the config files location
  - Reload the configuration upon cli.toml selection
  - Note: Only one daemon can be monitored at a time, but multiple GUI instances can be launched
- Files: TBC

#### Desktop Notifications
- Status: **Planned**
- Description: System notifications for tunnel status changes
- Library: `notify-rust` (already in dependencies)
- Events: Connected, Disconnected, Failed, Authentication Required

### Future Enhancements 📅

#### Auto-Reconnect/Health Monitoring
- Status: **Config exists, wiring needed. Design decided, not implemented.**
- Current: `TunnelOptions.auto_reconnect`, `reconnect_attempts` and `reconnect_delay` are
  read from profiles and surfaced in the CLI and GUI, but nothing acts on them.
  `run_tunnel()` connects once, and `crates/daemon/src/monitor.rs` is an empty stub.
- Files: `crates/daemon/src/tunnel.rs`, `crates/daemon/src/monitor.rs`

**Design decisions taken (to implement later):**

1. **Global setting with a per-profile override.** Today `auto_reconnect` is per-profile
   only. A daemon-wide default belongs in `daemon.toml`, since the daemon is what would
   perform the reconnection, with the profile field becoming `Option<bool>`: `None`
   inherits the global, `Some(_)` overrides it. Existing profiles carrying
   `auto_reconnect = true` still deserialise unchanged, so this is backward compatible.

2. **Never auto-reconnect where authentication needs a human.** Reconnecting is only
   meaningful when the daemon can authenticate unattended:

   | Auth setup | Can reconnect unattended? |
   |---|---|
   | `Key` + unencrypted key (`PasswordStorage::None`) | ✅ yes |
   | `Key` + passphrase in keychain | ✅ yes |
   | `Password` + password in keychain | ✅ yes - the daemon does retrieve it |
   | `Password`, not stored | ❌ prompts |
   | `PasswordWith2FA` | ❌ **never**, stored password or not - a TOTP code is single-use |

   An unencrypted key cannot be detected without trying to load it, so treat
   "key + storage None" as eligible and let a failed load fall back to the no-reconnect
   path.

3. **The current default is wrong and must change with this work.**
   `default_auto_reconnect()` returns `true`, so every profile - including 2FA ones that
   can never reconnect unattended - is currently flagged for auto-reconnect. It is
   harmless only because nothing acts on the flag.

4. **Surface ineligibility in the UI** rather than silently ignoring the setting: where a
   profile's auth setup makes unattended reconnection impossible, the CLI flag and the
   GUI's Advanced accordion should show the control as unavailable, with the reason.

> The v0.2.0 plan doc's proposed `can_auto_reconnect_without_auth()` is wrong in both
> directions - it rejects unencrypted keys (the most common eligible case) and rejects
> `Password` + keychain (which the daemon does support). Use the table above instead.

#### System Integration
- Status: **Partial** - systemd units exist - other might not be implemented
- Components:
  - ✅ Systemd user service templates
  - 🚧 System tray integration
  - 🚧 Desktop notifications
  - 🚧 Autostart for profiles (autostart option is already present but not wired)
  
### Known Issues / Technical Debt 🔧

- ✅ ~~Fix outdated tests in `crates/common`~~ - the "schema drift" was a misdiagnosis;
  the fixtures matched the structs and every test passed once run. The real problems were
  isolation (two tests read the developer's real `~/.config`, and the pidfile test could
  delete a running daemon's PID file) and missing coverage. Both fixed.
- ❌ Clarify token handoff so CLI can consume it without logging secrets
- ❌ `crates/daemon/src/monitor.rs` is an empty stub, but ARCHITECTURE.md described a
  health-monitoring loop as if it shipped (corrected)
- 🚧 Dynamic/SOCKS forwarding not implemented (returns error)
- 🚧 `crates/gui-qt` does not compile (cxx-qt bridge macro); excluded from
  `default-members` so it cannot break the default build or CI

## Quick Commands

```bash
# Build CLI + daemon
cargo build --package ssh-tunnel-cli --package ssh-tunnel-daemon

# Release build (default-members: excludes gui-qt)
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

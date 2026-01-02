# Changelog

All notable changes to SSH Tunnel Manager will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

---

## [0.1.10] - 2026-01-02

### Fixed
- **Authentication retry on failed 2FA/keyboard-interactive** - Complete fix for daemon and GUI
  - **Daemon**: Server-controlled retry now works correctly
    - Previously: First failed 2FA attempt would permanently fail the tunnel connection
    - Now: Daemon checks if `keyboard-interactive` is still in server's `remaining_methods`
    - If retry allowed, starts new keyboard-interactive session
    - Respects server's retry policy (typically 3 attempts for Google Authenticator)
    - Two-loop structure: outer loop for retry sessions, inner loop for prompts within session
    - File: `crates/daemon/src/tunnel.rs:1048-1179`
  - **GUI**: Architectural redesign - SSE-driven dialog state (eliminates race conditions)
    - Previously: Dialog closed immediately on submit, causing race with SSE events; polling workaround tried to catch "missed" events
    - Root cause: Multiple sources of truth (dialog state, network timing, SSE events)
    - Now: Dialog stays open showing "Verifying..." until SSE event confirms next state
    - SSE events are single source of truth: Connected → close dialog, AuthRequired → replace dialog, Error → close dialog
    - Removed polling workaround - trust SSE stream completely
    - Follows stateless web app pattern: GUI is pure view, daemon controls state
    - Files: `crates/gui-gtk/src/ui/auth_dialog.rs:161-276`, `crates/gui-gtk/src/ui/event_handler.rs:20-54`, `crates/gui-gtk/src/ui/window.rs:44-45,69`
- **Language-dependent string comparisons removed from daemon** - Internationalization-ready error detection
  - **Daemon connection error categorization** - Type-based error inspection instead of string matching
    - Replaced `err.to_string().contains()` checks with proper error type methods
    - Uses hyper::Error inspection: `is_timeout()`, `is_parse()`, `is_body_write_aborted()`, `is_incomplete_message()`, etc.
    - Walks error chain to find underlying `std::io::Error` and checks `ErrorKind` enum variants
    - Works correctly regardless of system locale or language settings
    - File: `crates/daemon/src/main.rs:56-107`
  - **Systemd error detection centralized** - Documented contract with TunnelManager
    - Created `is_tunnel_not_found_error()` helper function
    - Uses exact string matching (`==`) instead of substring contains
    - Centralized with clear documentation of error message contract
    - File: `crates/daemon/src/api.rs:40-51,289-299`
  - **Encrypted SSH key detection** - Type-based error matching
    - Replaced language-dependent string comparison with `russh::keys::Error` enum matching
    - Uses `RusshKeyError::KeyIsEncrypted` variant for language-independent detection
    - File: `crates/daemon/src/tunnel.rs:967-981`
  - **Keyboard-interactive prompt handling** - Server prompt passed as-is
    - Changed from hardcoded `AuthRequestType::TwoFactorCode` to `AuthRequestType::KeyboardInteractive`
    - Server's prompt text (in server's configured language) passed directly to GUI
    - No client-side string manipulation or language assumptions
    - File: `crates/daemon/src/tunnel.rs:1209-1218`
- **Test3 auth dialog placeholder mismatch** - Correct placeholder for remote password prompt
  - GUI now shows generic "Enter password or code" for keyboard-interactive auth
  - Server's prompt text guides user on what to enter
  - Previously hardcoded "Enter 2FA code" for all keyboard-interactive prompts

### Changed
- **Improved daemon HTTP connection error logging** - Better diagnostics and reduced log noise
  - Added intelligent error categorization: ClientDisconnect, SseStreamClose, NetworkError, ProtocolError, ServerError
  - SSE stream disconnects now logged at DEBUG level (were ERROR, cluttered logs)
  - Added structured logging with error source, type, and actionable hints
  - Added HTTP request tracing middleware with path, method, status, and latency
  - Files: `crates/daemon/src/main.rs:33-180` (error analysis), `crates/daemon/src/api.rs:100-108` (tracing middleware)
- **SSE client consolidated to common crate** - Single source of truth for both CLI and GUI
  - Moved `EventListener` and `TunnelEvent` from `gui-core` to `common` crate
  - Both CLI and GUI now import from `ssh_tunnel_common::sse`
  - Removed duplicate `TunnelEvent` enum from `daemon_client.rs`
  - Renamed `TunnelEvent` in `types.rs` to `TunnelDomainEvent` to avoid conflict
  - Framework-agnostic SSE client shared by all frontends
  - Files: `crates/common/src/sse.rs` (new), `crates/gui-core/src/daemon/sse.rs` (deleted), `crates/common/src/lib.rs`, `crates/gui-core/src/daemon/mod.rs`, `crates/cli/src/main.rs`

### Documentation
- **Updated About and Help dialogs** - Now show v0.1.9 features and current information
  - Moved documentation content to `crates/gui-core/assets/` for framework-agnostic sharing
  - Updated About dialog with v0.1.9 highlights (config wizard, remote daemon support, hybrid profile mode)
  - Updated Help dialog with comprehensive user guide including remote daemon setup
  - Updated copyright to SchirmForge, repository links to correct GitHub URL
  - Files: `crates/gui-core/assets/about.md`, `crates/gui-core/assets/help.md`

### Technical Details
- Error detection now uses type-based matching for language independence
- Hyper error inspection methods: `is_timeout()`, `is_parse()`, `is_body_write_aborted()`, `is_incomplete_message()`, `is_closed()`, `is_canceled()`
- IO error kind matching: `ConnectionReset`, `ConnectionAborted`, `BrokenPipe`, `NotConnected`, `TimedOut`
- SSE module now in common crate enables code reuse across all frontends
- Auth retry respects server's `remaining_methods` for protocol compliance

---

## [0.1.9] - 2025-12-31

### Added
- **Remote daemon profile support** - Profiles now work with HTTP/HTTPS remote daemons
  - New `ProfileSourceMode` enum: Local (filesystem), Hybrid (API + daemon filesystem), Remote (future)
  - `StartTunnelRequest` type sent via API includes profile data for remote daemons
  - Daemon automatically detects connection mode and handles profile accordingly
  - **Security**: SSH private keys stay on daemon filesystem at `~/.ssh/`, never sent over network
  - Profile content sent via API for HTTP/HTTPS connections (Hybrid mode)
  - SSH key path automatically converted to filename only (e.g., `/home/user/.ssh/id_ed25519` → `id_ed25519`)
  - Helper functions in common crate: `prepare_profile_for_remote()`, `get_remote_key_setup_message()`
  - **CLI**: Shows SSH key setup warning with scp commands to stderr before starting remote tunnel
  - **GUI**: Interactive warning dialog with Continue/Cancel button shows scp commands for key setup
  - Daemon validates SSH key exists at `~/.ssh/` and returns helpful error with scp commands if missing
  - Backward compatible: Unix socket mode unchanged, continues loading profiles from filesystem (Local mode)
  - Power user oriented: Manual SSH key file copying required for remote daemon scenarios
  - Files: `crates/common/src/types.rs`, `crates/common/src/profile_manager.rs:200-279`, `crates/daemon/src/api.rs:128-245`, `crates/common/src/daemon_client.rs:561-601`, `crates/gui-core/src/daemon/client.rs:88-159`, `crates/gui-gtk/src/ui/details.rs:215-313`
- **IP address validation in common crate** - Shared validation for IPv4, IPv6, and hostnames
  - New `is_valid_host()` function in `crates/common/src/network.rs`
  - Validates IPv4 addresses (e.g., `192.168.1.1`)
  - Validates IPv6 addresses (e.g., `::1`, `2001:db8::1`)
  - Validates hostnames following RFC 1123 rules (e.g., `daemon.local`, `example.com`)
  - Exported from `ssh_tunnel_common` for use in CLI and GUI
  - Comprehensive test coverage for valid and invalid inputs
  - Used in GUI configuration wizard for IP address prompts
- **HTTPS network configuration with empty daemon_host handling** - Improved UX for network access scenarios
  - Daemon now writes empty `daemon_host` field in snippet when binding to `0.0.0.0` or `::`
  - Prevents invalid `0.0.0.0` address from being saved in client configuration
  - Both CLI and GUI automatically detect empty `daemon_host` and prompt for actual IP address
  - GUI shows dialog with entry field and suggested default (192.168.1.100)
  - CLI will prompt interactively during snippet import (planned)
  - New validation functions in common crate: `config_needs_ip_address()`, `validate_client_config()`
  - Comprehensive documentation added to INSTALLATION.md explaining the behavior
  - Updated daemon.toml.example with explanation of snippet generation behavior
  - Affects: daemon snippet generation, GUI configuration wizard, client config validation
- **First-launch configuration wizard improvements** - Enhanced GUI setup experience
  - Fixed crash when declining snippet import (switched from adw::Window to adw::PreferencesWindow)
  - Changed button labels from "Import/Cancel" to "Yes/No" for better UX
  - Added fallback to manual configuration when snippet is declined
  - Manual config dialog now uses PreferencesWindow with proper layout
  - Auth token marked as required for all connection modes (not just HTTPS)
  - TLS fingerprint validation enforced for HTTPS mode
  - Enter key support in IP address dialog for quick submission
  - IP validation with helpful error messages showing valid examples
- **Qt6 GUI skeleton** - Minimal runnable Qt/QML shell (cxx-qt) landing on About page with a skeleton notice and static placeholder profiles; daemon wiring and real data are still in progress.
- **SSH Key Setup Warning opt-out** - Added "Don't show this again" checkbox to SSH key setup dialog
  - New `skip_ssh_setup_warning` field in `DaemonClientConfig` stored in `cli.toml`
  - Checkbox appears in both sidebar details panel and full profile details page
  - Preference persists across application restarts
  - Users can manually edit `cli.toml` to re-enable warnings
  - Files: `crates/common/src/daemon_client.rs`, `crates/gui-core/src/daemon/client.rs`, `crates/gui-core/src/daemon/config.rs`, `crates/gui-gtk/src/ui/details.rs`, `crates/gui-gtk/src/ui/profile_details.rs`
- **Daemon settings page improvements** - Hides "Restart Daemon" button when using HTTPS mode
  - Restart row only shown for unix-socket mode (local daemon)
  - Prevents confusion for remote daemon scenarios where restart would need to happen on remote host
  - File: `crates/gui-gtk/src/ui/daemon_settings.rs`
- **Enhanced SSH key error messages** - Improved clarity and accuracy for remote daemon scenarios
  - Daemon calculates and reports actual SSH directory via `DaemonInfo.ssh_key_dir`
  - Warning messages show daemon's actual paths instead of generic `~/.ssh`
  - Simplified copy instructions - removed specific scp/chmod commands that assume same usernames
  - Added recommendation to use ssh-agent for encrypted SSH keys
  - Daemon expands relative key paths (e.g., `id_reverse`) to full `~/.ssh/id_reverse`
  - Files: `crates/common/src/types.rs`, `crates/common/src/profile_manager.rs`, `crates/daemon/src/api.rs`, `crates/daemon/src/tunnel.rs`

### Fixed
- **IP address validation accepting invalid octets** - Now properly rejects IPs like `10.1.2.256`
  - Added check to distinguish between malformed IPv4 addresses and valid hostnames
  - Strings with 4 numeric parts separated by dots that fail IP parsing are now rejected
  - Prevents accepting invalid octets > 255 that look like hostnames (e.g., `256.1.1.1`, `1.1.1.999`)
  - Added comprehensive test coverage for invalid IP addresses
  - File: `crates/common/src/network.rs:46-52`
- **GUI configuration wizard connection mode persistence** - HTTPS mode now persists correctly
  - Fixed event listener to use wizard's pending config instead of loading from file
  - Modified `start_event_listener()` to check config in priority order: daemon_client → pending_daemon_config → file
  - Resolves issue where wizard configured HTTPS but connection used UnixSocket mode
  - File: `crates/gui-gtk/src/ui/window.rs:272-294`
- **GUI daemon client update after saving configuration** - Client now connects with saved config
  - Both save locations (Client Config page and close dialog) now update `daemon_client` in AppState
  - Fixes timeout waiting for first event from daemon after configuration save
  - Files: `crates/gui-gtk/src/ui/client_config.rs:255-287`, `crates/gui-gtk/src/ui/window.rs:230-260`
- **GUI close dialog infinite loop** - Proper window destruction after save
  - Changed from `window.close()` to `window.destroy()` to avoid re-triggering close-request handler
  - File: `crates/gui-gtk/src/ui/window.rs:260`

### Changed
- **Config validation architecture** - Validation before connection attempts
  - Moved config validation to common crate for reuse across CLI and GUI
  - CLI validates daemon config at start of every daemon command
  - Clear separation: validate → offer snippet copy → connect
  - Prevents confusing 401 errors by catching config issues early

---

## [0.1.8] - 2025-12-27

### Added
- **CLI stop --all command** - Stop all active tunnels with daemon status checking
  - Queries daemon for list of active tunnels
  - Filters to only running states (Connecting, Connected, WaitingForAuth, Reconnecting)
  - Stops each active tunnel individually with error handling
  - Provides detailed feedback with success/failure counts
  - Usage: `ssh-tunnel stop --all`
- **CLI status command** - Display tunnel connection status
  - `ssh-tunnel status <profile>` - Show detailed status for a single tunnel
  - `ssh-tunnel status --all` - Show formatted table of all active tunnels
  - Color-coded status indicators (green/yellow/red/gray)
  - Displays connection status, remote host, forwarding configuration, and uptime
- **CLI restart command** - Gracefully restart tunnels
  - `ssh-tunnel restart <profile>` - Stop and restart a tunnel
  - Two-step process with delay to ensure clean shutdown
  - Graceful handling when tunnel is not currently running
- **Enhanced 401 authentication error handling** - Proactive config validation
  - Comprehensive error message with step-by-step instructions when auth fails
  - Automatic detection if CLI config file is missing
  - Interactive prompt to copy daemon-generated config snippet
  - Common validation helpers in `ssh_tunnel_common::daemon_client` for reuse in GUI
  - New functions: `get_cli_config_snippet_path()`, `cli_config_snippet_exists()`, `validate_daemon_config()`
  - `ConfigValidationResult` enum with three states: Valid, MissingConfigSnippetAvailable, MissingConfigNoSnippet
  - All daemon commands now validate config BEFORE attempting connection
- **Keyring graceful fallback for headless environments** - Server and container support
  - Test-based keyring availability detection using actual DBus/Secret Service operations
  - No environment variable guessing - tests real keyring functionality
  - CLI profile creation succeeds even when keyring unavailable with clear warnings
  - `SSH_TUNNEL_SKIP_KEYRING` environment variable for explicit override
  - Useful for Docker containers, CI/CD, systemd services, automation
  - New `is_keychain_available()` function in common/keychain.rs
  - Comprehensive documentation in SYSTEMD.md and README.md for server deployments

### Fixed
- **IPv6 address formatting in host:port strings** - Proper URL notation for IPv6 literals
  - Added `format_host_port()` helper function in common crate
  - IPv6 addresses now wrapped in brackets: `[::1]:22` instead of `::1:22`
  - Fixes SSH connections, local port binding, daemon URLs with IPv6 addresses
  - Applied to: daemon tunnel connections, daemon bind addresses, daemon client URLs, GUI displays

### Changed
- **Tunnel description formatting** - Unified display across CLI and GUI
  - Added `format_tunnel_description()` helper in common crate
  - New format with explicit labels:
    - Local forwarding: `local: 127.0.0.1:8080 → remote: example.com:80`
    - Remote forwarding: `remote: example.com:80 → local: 127.0.0.1:8080`
    - Dynamic SOCKS: `SOCKS: 127.0.0.1:1080`
  - Consistent across CLI connection messages, GUI profile details, and profile lists
  - Properly handles IPv6 addresses with bracket notation

---

## [0.1.7] - 2025-12-22

### Fixed
- **GUI Authentication Dialog Architecture** - Complete rewrite to event-driven pattern
  - Fixed crashes with `EnterError` (nested executor issue)
  - Fixed unresponsive/frozen dialogs
  - Fixed duplicate dialogs appearing on cancel
  - Fixed 60-second timeout on cancellation
  - **Root cause**: Attempting to run nested `glib::MainLoop` inside `glib::spawn_local` context
  - **Solution**: Event-driven architecture via SSE
    - Daemon sends `AuthRequired` events via Server-Sent Events
    - GUI `handle_auth_request()` shows dialogs asynchronously
    - Dialog state tracking prevents duplicates (`auth_dialog_open`, `pending_auth_requests`, `active_auth_requests`)
    - Cancel button calls `daemon_client.stop_tunnel()` for immediate abort
  - Removed failed approaches: async-trait, async channels, nested MainLoop
  - **Files changed**:
    - `crates/gui/src/ui/auth_dialog.rs` - New event-driven dialog handler with queuing
    - `crates/gui/src/ui/tunnel_handler.rs` - Removed (no longer needed)
    - `crates/gui/src/ui/profile_details.rs` - Simplified to just call `start_tunnel()`
    - `crates/gui/src/ui/details.rs` - Simplified to just call `start_tunnel()`
    - `crates/gui/src/ui/profiles_list.rs` - Check for pending auth on initial status query
    - `crates/gui/src/ui/window.rs` - Added state fields for dialog tracking
  - All authentication flows now work correctly: password entry, cancel, SSH retry, concurrent tunnels
- **Daemon Graceful Cancellation** - Fixed connection hanging during auth
  - Modified `stop()` to send shutdown signal before aborting task
  - Modified `run_tunnel()` to use `tokio::select!` for shutdown during connection/auth phase
  - Fixes 30-second hang when canceling during authentication prompt

### Changed
- Removed `async-channel` dependency from GUI (no longer needed with event-driven approach)
- Removed unused `TunnelEventHandler` implementations in GUI (auth now via SSE)
- Fixed `unused_mut` warnings in `daemon_client.rs` (removed `mut` from timer variables)

### Technical Details
- Architecture shift: Synchronous trait blocking → Event-driven SSE
- No more nested event loops or executor conflicts
- Dialog responsiveness via async GTK callbacks
- State machine for dialog queuing prevents race conditions
- Clean separation: daemon handles SSH, GUI handles user interaction via events

---

## [Unreleased - Future IPv6 Work]

### Changed
- **BREAKING: Configuration Fields Split for Better IPv6 Support**
  - **Daemon Configuration** (`daemon.toml`):
    - `bind_address` (single field) replaced with `bind_host` and `bind_port` (separate fields)
    - Migration: `bind_address = "127.0.0.1:3443"` → `bind_host = "127.0.0.1"` and `bind_port = 3443`
  - **CLI Configuration** (`cli.toml`):
    - `daemon_url` (for HTTP/HTTPS) replaced with `daemon_host` and `daemon_port` (separate fields)
    - Migration: `daemon_url = "127.0.0.1:3443"` → `daemon_host = "127.0.0.1"` and `daemon_port = 3443`
    - `daemon_url` retained for UnixSocket mode (socket path override)
  - **Improvements**:
    - Better IPv6 support and clearer configuration
    - Loopback detection now uses `std::net::IpAddr::is_loopback()` instead of string matching
    - Properly handles IPv4 (`127.0.0.1`), IPv6 (`::1`), and hostname (`localhost`, case-insensitive)
    - Type-safe port as `u16` instead of string parsing

### Added
- IPv6 loopback test coverage (`::1` for tcp-http mode, `::` rejection for tcp-http)
- IPv6 support test for tcp-https mode
- Case-insensitive hostname matching for "localhost" (DNS is case-insensitive)

### Fixed
- Updated all unit tests to use new field structure (`daemon_host`/`daemon_port`, `bind_host`/`bind_port`)
- Fixed `profile_manager` tests to use current `ProfileMetadata` structure (with timestamps)
- Fixed `daemon_client` tests to use `daemon_host` and `daemon_port` instead of `daemon_url`

---

## [0.1.6] - 2025-12-20

### Security
- **Enhanced Daemon Security** - Comprehensive security hardening
  - **Authentication enabled by default**: `require_auth` now defaults to `true` instead of `false`
  - **Non-loopback HTTPS enforcement**: HTTP mode (`tcp-http`) is restricted to loopback addresses only
  - **Configuration validation**: Daemon validates config at startup to prevent insecure configurations
  - **Restrictive umask**: Set umask to 0077 at daemon startup to prevent permission leaks
  - **File permissions hardening**:
    - Config files: `daemon.toml` set to 0600 (owner read/write only)
    - Auth token: `daemon.token` set to 0600
    - TLS files: Certificate and key files set to 0600
    - CLI config snippet: `cli-config.snippet` set to 0600 (contains auth token and TLS fingerprint)
    - Runtime directory: 0700 by default (owner only)
    - Unix socket: 0600 by default (owner only)
  - **Group access mode**: Optional `group_access` config for multi-user system daemons
    - When enabled: directory 0770, socket 0660 (owner + group access)
    - When disabled (default): directory 0700, socket 0600 (owner only)
  - **TLS fingerprint fix**: Certificate generated before fingerprint calculation to prevent "unavailable" in client config
  - Added comprehensive unit tests for security validation logic
  - Updated example configuration with security warnings and recommendations

### Added
- **Permissions Module** - New `permissions.rs` module in daemon crate
  - `set_restrictive_umask()` - Sets umask to 0077 at startup
  - `set_file_permissions_private()` - Sets 0600 on files
  - `set_directory_permissions()` - Sets 0700 or 0770 based on group_access
  - `set_socket_permissions()` - Sets 0600 or 0660 based on group_access
  - `ensure_directory_with_permissions()` - Creates directories with correct permissions

### Changed
- **Daemon Configuration**: Added `group_access: bool` field with default `false`
- **Daemon Startup**: Umask set to 0077 before any file creation
- **Unix Socket Creation**: Permissions set immediately after binding, uses already-loaded config instead of reloading
- **TLS Certificate Generation**: Moved before fingerprint calculation in HTTPS mode
- **Config File Saving**: Sets 0600 permissions after writing `daemon.toml`
- **CLI Config Snippet**: Sets 0600 permissions after writing (contains auth token and TLS fingerprint)

### Technical Details
- New module: `crates/daemon/src/permissions.rs`
- 9 security validation unit tests all passing (8 config validation + 1 CLI snippet permissions)
- Compatible with systemd service `RuntimeDirectoryMode=0750`
- Umask: 0077 (rwx------)
- Default permissions: directories 0700, files/sockets 0600
- Group mode permissions: directories 0770, sockets 0660

---

## [0.1.5] - 2025-12-20

### Added
- **GUI Real-time Status Indicators** - Visual feedback for tunnel connection states
  - Colored status dots on profile list (green/orange/red/gray)
  - Green (●) for Connected tunnels
  - Orange (●) for Connecting, WaitingForAuth, Reconnecting, Disconnecting states
  - Red (●) for Failed tunnels
  - Gray (●) for NotConnected and Disconnected tunnels
  - Pulsing animation on orange dots during transitional states
  - Status dots update in real-time via SSE events
  - New `create_status_dot()` helper function in `profiles_list.rs`
  - CSS classes for status styling in `style.css`
- **Daemon Connection Monitoring** - Enhanced daemon availability tracking
  - Network icon in navigation sidebar showing daemon status
  - Tooltip on hover displaying connection state
  - Changed from `network-transmit-receive-symbolic` to `network-wired-symbolic` (standard GNOME icon)
  - Heartbeat-based timeout detection (30-second timeout)
  - Automatic reconnection with exponential backoff (2s → 30s max)
  - All profile dots reset to gray when daemon disconnects
  - Initial status query via `/api/tunnels` on connection/reconnection
  - CSS classes: `daemon-connected`, `daemon-connecting`, `daemon-offline`
- **Help and About Dialogs** - Integrated documentation
  - Help dialog accessible from burger menu showing usage documentation
  - About dialog showing application information (version, features, license)
  - Markdown rendering support via `pulldown-cmark` library
  - Markdown to Pango markup converter for proper text formatting
  - Support for headings (H1-H6), bold, italic, code blocks, lists, links, block quotes
  - Documents embedded at compile time using `include_str!` macro
  - New modules: `help_dialog.rs`, `about_dialog.rs`, `markdown.rs`
  - Markdown assets: `assets/help.md`, `assets/about.md`
- **Profile Status Tracking** - Per-profile tunnel state management
  - Added `status: RefCell<TunnelStatus>` field to `ProfileModel`
  - Manual `Default` implementation for `ProfileModel`
  - `status()` and `update_status()` methods for state access
  - Status stored on ActionRow widgets for efficient updates
- **Profile Management Dialog** - Complete CRUD interface for SSH tunnel profiles
  - Unified dialog for creating and editing profiles
  - "New Profile" button on profiles list page with `list-add-symbolic` icon
  - ESC key handler to close dialog
  - Profile dialog shows appropriate window title ("New Profile" or "Edit Profile")
  - Organized sections: Basic Information, Authentication, Port Forwarding, Advanced Tuning
  - **Advanced Options Accordion** - Collapsible section for TunnelOptions
    - Compression toggle (SSH compression)
    - Keepalive interval (0-300 seconds)
    - Auto-reconnect toggle with attempts and delay settings
    - TCP keepalive toggle
    - Max packet size (1024-65536 bytes)
    - Window size (32KB-2MB)
  - **Default Values** for quick profile creation
    - Profile name: "My SSH Tunnel"
    - SSH port: 22
    - SSH key path: `~/.ssh/id_ed25519`
    - Local bind: 127.0.0.1:8080
    - Remote forward: localhost:80
  - All switches with proper vertical alignment and activatable rows
  - File chooser for SSH key selection (opens to `~/.ssh` directory)
  - SSH key filters: `id_*`, `*.pem`, `*.key` patterns
  - Duplicate name validation prevents conflicts
  - Profile editing with automatic navigation back to list after save

### Changed
- **GUI Navigation** - Improved sidebar layout
  - Added burger menu (three-dot icon) to navigation header
  - Menu items: Help and About
  - Actions registered with GTK application: `app.help`, `app.about`
- **Profile Dialog UX** - Restructured for better usability
  - Authentication and Port Forwarding sections visible by default (essential config)
  - Advanced tuning options (compression, keepalive, packet sizes) in collapsible accordion
  - Switches properly aligned with GNOME Settings style (centered, activatable rows)
  - Modern SSH key defaults (ed25519 instead of RSA)
  - Practical port forwarding defaults (8080→80 for web development)
- **Profile Management** - Uses shared functions from common crate
  - Profile creation/editing reuses `ssh_tunnel_common::save_profile()`
  - Profile deletion reuses `ssh_tunnel_common::delete_profile_by_id()`
  - Duplicate name checking via `ssh_tunnel_common::profile_exists_by_name()`
  - Proper `overwrite` parameter usage (false for new, true for edit)
  - Profile list auto-refresh after create/edit/delete operations
- **SSE Event Handling** - Complete integration with status indicators
  - All tunnel events (Connected, Starting, Disconnected, Error, etc.) update profile dots
  - `update_profile_status()` function in `profiles_list.rs` for centralized status updates
  - Status dot widgets stored as ActionRow data for in-place updates
  - CSS class swapping instead of widget replacement for better performance
- **Initial Status Query** - Accurate state on startup/reconnection
  - GUI queries `/api/tunnels` endpoint when connecting to daemon
  - All active tunnels show correct status immediately
  - Prevents stale gray dots for already-connected tunnels
  - Query happens asynchronously after first SSE event received
- **Daemon Event Listener** - More robust connection handling
  - Heartbeat monitoring task with proper timestamp tracking
  - Fixed false timeout issue by initializing `last_heartbeat` correctly
  - Removed redundant daemon health check at startup
  - Event listener handles all daemon status updates

### Fixed
- **False Heartbeat Timeouts** - Connection incorrectly marked as offline
  - Increased heartbeat timeout from 15 to 30 seconds
  - Fixed timing issue where monitor task started before first heartbeat received
  - Heartbeat timestamp now initialized when connection verified
  - Eliminated spurious "daemon appears offline" messages
- **Status Dots Not Updating** - Visual indicators stuck at initial state
  - Fixed by storing status dot widget reference on ActionRow
  - Updates now modify CSS classes instead of recreating widgets
  - Corrected widget hierarchy traversal (ActionRow vs ListBoxRow)
  - Removed duplicate status dots appearing on updates
- **Missing Initial Status** - Tunnels already running not shown as connected
  - GUI now queries current tunnel status on connection/reconnection
  - `/api/tunnels` endpoint called after SSE connection established
  - All profile statuses reflect actual daemon state immediately

### Technical Details
- New dependency: `pulldown-cmark = "0.12"` for markdown parsing
- Heartbeat timeout: 30 seconds (daemon sends heartbeats every ~10-12 seconds)
- Status dot implementation: GTK Label widgets with bullet character (●)
- Pango markup support: Headings, emphasis, code, lists, links, block quotes
- Window actions use `gio::SimpleAction` for menu integration
- Auto-reconnect backoff: 2s → 4s → 8s → 16s → 30s (max)

---

## [0.1.4] - 2025-12-09

### Added
- **GUI Authentication Dialog** - Interactive password/2FA prompt handling
  - Modal dialog for SSH key passphrase, remote password, and 2FA code input
  - Real-time response to `AuthRequired` SSE events from daemon
  - Password entry field with peek icon to reveal text
  - Keyboard shortcut (Enter) to submit authentication
  - Async submission to daemon `/api/tunnels/{id}/auth` endpoint
  - New `auth_dialog.rs` module in GUI crate
- **SSH Key Authentication Support in GUI**
  - SSH key switch in profile editor to enable/disable key authentication
  - Key path entry field with file browser button
  - File chooser dialog opening to `~/.ssh` directory by default
  - SSH key file filters (id_*, *.pem, *.key patterns)
  - Password field for encrypted SSH keys
  - Authentication section in profile details view showing auth type and key path
- **Bind Address Configuration in GUI**
  - Local host/bind address field in profile editor
  - Defaults to 127.0.0.1 (localhost)
  - Supports 0.0.0.0 (all interfaces) or specific IP addresses
  - Displayed in port forwarding section of profile details
- **SSE Heartbeat Support** - Connection health monitoring
  - Daemon emits periodic heartbeat events on `/api/events`
  - CLI and GUI detect stalled connections via heartbeat timeout
  - Configurable test interval for development/testing
  - New `Heartbeat` event variant in `TunnelEvent` enum
- **Graceful Daemon Shutdown** - Clean tunnel teardown on exit
  - Signal handling for Ctrl+C (SIGINT) and SIGTERM
  - `TunnelManager::stop_all()` method to stop all active tunnels
  - Proper cleanup across Unix socket, TCP HTTP, and HTTPS listeners
  - Uses `axum-server::Handle` for graceful shutdown coordination

### Changed
- **GUI Event Listener** - Now SSE-first for daemon connection detection
  - GUI daemon status indicator updates from SSE traffic (events + heartbeats)
  - Status icon shows connected as soon as heartbeats/events arrive
  - Handles case where GUI starts before daemon (reconnects automatically)
  - Removed polling-based status checks in favor of SSE stream
- **GUI Profile Details Layout** - Reorganized for better UX
  - Profile name and ID now centered at top
  - Connection status section centered below header
  - Action buttons (Start/Stop) centered below status
  - Info sections (SSH Connection, Authentication, Port Forwarding) below actions
- **CLI Start Flow** - SSE-first with fallback to polling
  - Primary connection monitoring via SSE stream
  - Polling only used on idle/stream loss for resilience
  - Adjusted auth handling to work with SSE events
  - Added idle and overall timeouts for connection attempts
- **Auth Middleware Logging** - Reduced log spam
  - Success messages moved to trace level
  - Only warnings and errors logged at default level
- **TunnelManager** - Now cloneable for shared access
  - Added `Clone` derive to enable multi-threaded access
  - Required for shutdown coordination across listeners

### Fixed
- **Auth Event Handling** - GUI now properly handles `AuthRequired` events
  - Previously ignored auth requests, causing connection hangs
  - Dialog automatically appears when daemon needs user input
  - Removed premature "Tunnel started successfully" dialogs
  - Status updates via SSE provide accurate connection state

### Technical Details
- GUI dependencies: `libadwaita::prelude::MessageDialogExt` for dialog APIs
- Daemon dependencies: Signal handling via `tokio::signal`
- SSE heartbeat interval: 30 seconds (configurable via test override)
- Auth dialog uses `gtk4::PasswordEntry` with show/hide toggle

---

## [0.1.3] - 2025-12-08

### Added
- **SSH Host Key Verification** - Full known_hosts implementation
  - Verify server host keys to prevent MITM attacks
  - Support OpenSSH known_hosts format
  - Store in `~/.config/ssh-tunnel-manager/known_hosts` by default
  - Configurable known_hosts path via `daemon.toml`
  - SHA256 fingerprint calculation and display
  - Interactive prompts for unknown hosts
  - Clear warning messages for host key mismatches
  - Unit tests for known_hosts parsing and verification

### Changed
- `DaemonConfig` now includes `known_hosts_path` field with default function
- `TunnelManager` accepts known_hosts path in constructor
- `ClientHandler` validates host keys during SSH connection establishment
- Authentication request handler task now spawned before SSH connection (fixes race condition)

### Fixed
- **Critical race condition** where authentication requests were sent before listener was ready
- Connection hanging issue during host key verification
- Auth handler now receives host key verification requests properly

---

## [0.1.2] - 2025-12-08

### Added
- **New CLI commands**
  - `delete [profile]` - Delete a profile with confirmation prompt
  - `info [profile]` - Show detailed information about a profile
- **Duplicate name check** when creating profiles to prevent conflicts

### Changed
- **Code consolidation complete** - Extracted shared functionality to common crate
  - **Profile Manager** moved to common crate for code reuse between CLI, GUI, and Daemon
    - New `profile_manager.rs` module with all profile I/O operations
    - Functions: `load_profile`, `load_profile_by_id`, `load_profile_by_name`, `load_all_profiles`, `save_profile`, `delete_profile_by_id`, `delete_profile_by_name`, `profile_exists_by_id`, `profile_exists_by_name`
    - Removed duplicate profile management code from CLI and daemon (110+ lines consolidated)
  - **Daemon Client** moved to common crate for code reuse between CLI and GUI
    - New `daemon_client.rs` module with daemon connection logic
    - Types: `DaemonClientConfig`, `ConnectionMode` (UnixSocket, Http, Https)
    - Functions: `create_daemon_client()`, `add_auth_header()`
    - Simplified CLI config to wrap `DaemonClientConfig` with file I/O
    - Centralized all daemon connection logic for consistency
- **Code cleanup** - Removed unused functions and methods
  - CLI: Removed `daemon_socket_path()`, `save()`, and `socket_path()` methods
  - Daemon: Removed unused `handle_keyboard_interactive_result()` function
  - Zero warnings in CLI and daemon builds
- Updated `ashpd` dependency from 0.9.2 to 0.12.0 to avoid future Rust compatibility issues

---

## [0.1.1] - 2025-12-08

### Added
- **PID file locking mechanism** to prevent multiple daemon instances from running simultaneously
  - New `pidfile.rs` module in daemon crate
  - Automatic detection and cleanup of stale PID files
  - Platform-specific process existence checking on Unix systems
  - PID file location: `/run/user/<uid>/ssh-tunnel-manager/daemon.pid`
  - Comprehensive test script (`test-pidfile.sh`) for verification

### Changed
- **TLS module moved to common crate** for code reuse between CLI and GUI
  - Moved `tls.rs` from CLI to `crates/common/src/tls.rs`
  - Updated CLI to import TLS functions from common crate
  - Added TLS dependencies (rustls, webpki-roots, sha2) to common crate
  - Removed duplicate TLS dependencies from CLI crate

### Fixed
- Issue where multiple daemon instances could run simultaneously causing conflicts
- Test script failures due to multiple daemon instances

---

## [0.1.0] - 2025-12-07

### Added
- **Network-ready daemon architecture** with three operational modes:
  - Unix Socket (local-only, no TLS, no auth required)
  - TCP HTTP (localhost-only, token auth, no TLS)
  - TCP HTTPS (network-ready, token auth, TLS with certificate pinning)
- **TLS certificate generation** for HTTPS mode
  - Self-signed certificate generation with rcgen
  - Certificate fingerprint validation
  - Client-side certificate pinning for security
- **Token-based authentication** for TCP modes
  - Automatic token generation and persistence
  - X-Tunnel-Token header authentication
  - Optional authentication (disabled by default for Unix socket)
- **Daemon configuration system**
  - TOML-based configuration (`daemon.toml`)
  - Runtime mode switching (unix-socket, tcp-http, tcp-https)
  - Configurable bind addresses
  - TLS certificate and key paths
- **CLI configuration system**
  - Separate CLI config (`cli-config.toml`)
  - Daemon connection settings (URL, mode, token)
  - TLS certificate fingerprint pinning
  - Interactive configuration wizard
- **Comprehensive test suite** (`test-network-modes.sh`)
  - Automated testing of all three daemon modes
  - Profile creation and tunnel lifecycle testing
  - Authentication verification
  - TLS certificate pinning validation

### Changed
- Daemon now supports multiple listener modes instead of Unix socket only
- API endpoints now support optional authentication middleware
- Enhanced security with TLS and token authentication for network access

### Technical Details
- Dependencies: axum-server with TLS support, rcgen for certificate generation
- Architecture: Modular design with separate auth, TLS, and config modules
- Security: Certificate pinning prevents MITM attacks in HTTPS mode

---

## Version Numbering Guidelines

This project uses semantic versioning (MAJOR.MINOR.PATCH):

### Patch Version (0.1.X -> 0.1.X+1)
Increment for:
- Bug fixes
- Security patches
- Performance improvements
- Code refactoring without functionality changes
- Documentation updates
- Internal architecture improvements (like code consolidation)
- Test additions/improvements

### Minor Version (0.X.0 -> 0.X+1.0)
Increment for:
- New features or functionality
- New operational modes
- New CLI commands
- GUI implementation milestones
- Significant architecture changes
- Breaking changes in configuration format
- New authentication methods
- Protocol additions (SSH host key verification, etc.)

### Major Version (X.0.0 -> X+1.0.0)
Increment for:
- Complete rewrites
- Fundamental architecture changes
- Breaking API changes for library users
- Production-ready releases (0.x -> 1.0)
- Major feature completions warranting stable release

---

## Relationship with PROJECT_STATUS.md

### CHANGELOG.md (this file)
- **Purpose**: Track version history and changes over time
- **Audience**: Users and developers tracking releases
- **Content**: Release notes, version numbers, dates, categorized changes
- **Update When**:
  - Completing a phase of work
  - Releasing a new version
  - Merging significant features
  - Before creating git tags

### PROJECT_STATUS.md
- **Purpose**: Current state snapshot and implementation roadmap
- **Audience**: New developers and contributors
- **Content**: What's implemented, what's planned, architecture overview
- **Update When**:
  - Major feature completion (entire subsystem implemented)
  - Architecture changes affecting multiple components
  - Significant milestone achievements
  - Adding/removing planned features
  - Status changes (prototype -> beta -> stable)
  - Not for every patch version

### Update Guidelines

**Update CHANGELOG.md:**
- Every version bump (patch, minor, major)
- When completing a todo/task with user-visible changes
- Before running comprehensive test suites

**Update PROJECT_STATUS.md:**
- When completing major features (GUI, host key verification)
- When changing project status (working prototype -> beta -> v1.0)
- When adding new crates or major modules
- When architecture decisions affect the big picture
- Typically with minor version bumps, not patches

---

## [Unreleased] - Template for Next Version

When starting work on next version, copy this template:

```markdown
## [0.1.X] - YYYY-MM-DD

### Added
- New features, files, capabilities

### Changed
- Modifications to existing functionality
- Refactoring and improvements

### Deprecated
- Features being phased out

### Removed
- Deleted features or files

### Fixed
- Bug fixes

### Security
- Security-related changes
```

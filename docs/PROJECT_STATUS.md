# Project Status

## Current State

**Version**: v0.6.0
**Status**: ✅ Production-ready CLI, daemon, and GTK 4/libadwaita GUI
**Release date**: 2026-09-03

A snapshot of what exists today. For what is planned see [ROADMAP.md](ROADMAP.md); for what
shipped when see [CHANGELOG.md](CHANGELOG.md); for behaviour described from the user's point
of view see the [user stories](user-stories/).

| Area | State |
|---|---|
| Daemon | ✅ Local port forwarding, interactive authentication over SSE, host key verification, three listener modes. Refuses to run as root; privileged ports come from `CAP_NET_BIND_SERVICE` |
| Event delivery | ✅ One `EventListener` shared by the CLI and every GUI. Outstanding authentication prompts are re-sent to any client that connects, so a reconnect no longer loses them |
| CLI | ✅ Profile CRUD, tunnel control, status, watch |
| Production GUI (`ssh-tunnel-gui`) | ✅ `crates/gui-v2`, first-launch setup, adaptive profile management, structured authentication, live status, remote daemon support |
| Obsolete GUI (`gui-gtk`) | ⛔ Frozen in-tree reference; not a default target, installed, packaged, or maintained |
| Testing | ✅ Four tiers (static, hermetic, live SSH on a local fixture, live SSH on a real host), sandboxed, all but the last gating CI |
| Supply chain | ✅ `cargo deny check` gates every pull request; git dependencies banned; 1 known advisory, documented as accepted |
| Credential storage | ✅ Store selected at runtime and reported; works with a local or remote daemon; migrates from the pre-v0.2.0 store |
| Automatic tunnel reconnect | ❌ Config options exist but nothing acts on them; the explicit post-edit **Reconnect now** action is manual and separate |
| Notifications | ❌ Not implemented |

## What’s Implemented

### ✅ Common (`crates/common`)
- Typed configs (`Profile`, `ConnectionConfig`, `ForwardingConfig`, `TunnelOptions`) with validation and TOML persistence.
- `PasswordStorage` records `client`, `daemon-host`, `none`, reserved `file`, and legacy
  `keychain`, with backward-compatible deserialization and location-aware resolution.
- Credential-store facade with Secret Service/keyutils selection, availability detection,
  migration, and storage operations.
- Shared types for auth flows (`AuthType`, `AuthRequest`, `TunnelStatus`, events).
- Daemon client helpers (reqwest setup, auth header, TLS pinning helpers, socket path auto-detection).
- **Config validation helpers** - `validate_daemon_config()`, `get_cli_config_snippet_path()`, `cli_config_snippet_exists()` for proactive validation.
- **SSE-first tunnel control flow** (`start_tunnel_with_events`, `stop_tunnel`) with event handler trait for shared CLI/GUI logic.

### ✅ Daemon (`crates/daemon`)
- SSH tunnel lifecycle using russh 0.63 (crates.io, compression and SHA-1 MACs not offered); interactive auth via SSE-driven prompts (password, key passphrase, keyboard-interactive/2FA), each re-prompting rather than failing on a wrong answer.
- Host key verification against `known_hosts`; a server presenting a CA-signed host *certificate* is refused rather than silently trusted.
- Credentials are read from whichever store the daemon opened (Secret Service, or the kernel keyutils keyring on a headless host), or supplied by the client when the profile says they live there.
- Local forwarding fully working; privileged-port error messaging.
- Host key verification with OpenSSH-format `known_hosts`, SHA256 fingerprints, and 0600 perms.
- API server (Axum): health, tunnel start/stop/status, pending-auth get/post, SSE events.
- TLS self-signed cert generation and fingerprint display for HTTPS mode.
- PID file guard to avoid duplicate instances.

### ✅ CLI (`crates/cli`)
- Profile CRUD (add/list/show/delete/info) with interactive prompts and non-interactive flags.
- Credential-store integration for passwords/passphrases with graceful Secret
  Service/keyutils fallback.
- Store availability detection: profile creation succeeds even when no store is available.
- Tunnel control: start/stop/restart/status with `--all` flag support.
- **Proactive daemon config validation** - checks config before connection attempts with interactive snippet copy.
- Table/JSON output, colorized UX, validation of key permissions and privileged ports.
- Start/stop/status using the **shared `EventListener`** from the common module, the same one
  the GUIs use; interactive auth handling. The CLI no longer carries its own SSE parsing.

### ✅ GUI Core (`crates/gui-core`)
- Framework-agnostic application state and operations for GTK and future presentation adapters.
- Shared `AppController`, immutable `AppSnapshot`, typed `AppCommand`/`ControllerEffect`,
  code-derived `ActionAvailability`, and explicit feature capabilities.
- Shared active-profile reconnect action and presentation contract. The controller decides
  from structured `TunnelStatus` values whether a successful edit needs a reconnect notice;
  the runtime sequences stop, status, and start without parsing daemon text.
- Toolkit-neutral runtime/effect executor for profile I/O, daemon health/inventory/SSE,
  preferences, authentication, editor persistence, and client-held credentials.
- FIFO authentication queue and typed answers correlated by request/tunnel IDs. Structured
  daemon request/status codes and booleans are the only control plane.
- Redacted profile-editor contracts with structured field validation and transactional
  Keep/Store/Remove credential updates.
- Toolkit-neutral client setup discovery, snippet loading, mode-specific validation, token
  redaction, and atomic `0600` persistence before runtime construction.
- Versioned GUI preferences for profile order, pins, filters, and sorting in `ui.toml`, with
  atomic writes and safe recovery from missing, malformed, stale, or unknown IDs.
- Presentation-ready profile list/detail, daemon, authentication, WIP, empty, and offline
  state. `AppCore` remains available only for the obsolete `gui-gtk` compatibility source.

### ✅ Production GUI (`crates/gui-v2`, binary `ssh-tunnel-gui`)

- Production GTK 4/libadwaita application in the root workspace, targeting the Bazzite/Fedora
  44 GTK 4.22 and libadwaita 1.9 runtime.
- First-launch client setup gates runtime creation, imports daemon-generated snippets,
  completes empty network hosts, repairs invalid configuration, or collects Unix
  socket/HTTP/HTTPS settings manually.
- Adaptive profile list and details with search, connected-only filtering, name/manual sort,
  pinning, drag ordering, and keyboard ordering within pin sections.
- Shared connect/cancel/disconnect/retry, edit, duplicate, delete, pin, order, and
  auto-reconnect commands routed exclusively through `gui-core`.
- Profiles remain editable while active. A successful save explains that the running tunnel
  still uses its previous settings and offers **OK** or **Reconnect now**.
- Profile editor with local/remote key-path semantics, redacted credential changes, dynamic
  auth/2FA/account/auto-reconnect fields, and explicit WIP capability presentation.
- One request-ID-keyed authentication dialog at a time, FIFO advancement, structured hidden
  input, typed host-key decisions, and SSE/inventory-confirmed completion.
- Real daemon health/info/refresh, empty-profile and offline states. Start, restart, shutdown,
  SSH import, unsupported forwarding/runtime options, and missing telemetry are honest WIP
  surfaces.
- System fonts, semantic theme colors, daemon prompt copy on the window background, labelled
  controls, alert/status semantics, keyboard shortcuts, wrapping layouts, and long-content
  scrollers.
- Automated validation, Bazzite runtime launch, live daemon/SSE reconciliation, and user
  regression acceptance pass. Final keyboard-only and screen-comparison checks remain listed
  in [KNOWN_ISSUES.md](KNOWN_ISSUES.md) and do not block the production target.

### ⛔ Obsolete GUI GTK (`crates/gui-gtk`)

This source is frozen for reference and compatibility. It remains a workspace member on the
same GTK 4.22/libadwaita 1.9 binding generation, but is excluded from the default members,
is not installed or packaged, and receives no further feature updates. Removal is not
currently planned.

Historical capabilities at the point it was frozen:
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

✅ Create profiles and store credentials through Secret Service or keyutils
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

✅ Production GUI profile search, pinning, ordering, filtering, and safe CRUD
✅ Production GUI structured FIFO authentication and client-held credential resolution
✅ Production GUI daemon/empty/offline views and adaptive/accessibility groundwork
✅ `ssh-tunnel-gui` is the default executable and installed desktop entry
✅ Active profiles can be edited, retained on their current settings, or reconnected with the
saved settings through a structured status-driven action

✅ Sandboxed test suite across four tiers, blocking supply-chain audit, CI, clippy clean

❌ Remote forwarding — [not planned](ROADMAP.md#not-planned)
❌ Dynamic/SOCKS forwarding — [future, unscheduled](ROADMAP.md#later)
❌ SSH tunnel auto-reconnect / health monitoring — options exist but nothing acts on them;
   [design decided](ROADMAP.md#auto-reconnect-and-health-monitoring), not implemented
❌ System tray — crate removed in v0.1.11; to be rewritten if wanted
❌ Desktop notifications — [planned](ROADMAP.md#next)
❌ Packaging: Flatpak, AUR ([RPM in progress](ROADMAP.md#packaging))

The complete list of features deferred or excluded from v0.4.0 is maintained in
[ROADMAP.md](ROADMAP.md#deferred-or-excluded-from-v040).

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
- Credentials remain behind the Secret Service/keyutils facade; SSH keys are referenced by
  path only.

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

# Live SSH tests against a local unprivileged sshd - no setup, no credentials
make test-live-fixture

# Live SSH tests against a real host (see scripts/provision-test-target.sh)
make test-live

# Supply-chain audit: advisories, licences, sources, unused dependencies
make audit

# Credential storage against a real Secret Service, in a throwaway session
make test-keychain-live

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

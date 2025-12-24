# SSH Tunnel Manager Technical Reference

Suggested name: `TECHNICAL_REFERENCE.md`. Purpose: internal architecture map of the CLI, daemon, GUI, shared library, and their dependencies.

## Module List
- Common library (`crates/common`): shared config/types, daemon client helpers, TLS pinning, profile I/O.
- Daemon (`crates/daemon`): long-lived service exposing REST/SSE, manages SSH tunnels and auth, TLS, known_hosts, PID guard.
- CLI (`crates/cli`): terminal client for profile CRUD and tunnel lifecycle, talks to daemon.
- GUI (`crates/gui`): GTK/libadwaita desktop app consuming the same daemon API with SSE updates.
- Config/docs/scripts: `docs/*.md`, `config-files/*.toml`, `quick-setup.sh`, `Makefile`, `daemon.toml.example`, `cli.toml.example`.

## Source Files & Related Assets
- Common
  - `crates/common/src/lib.rs`: module exports/re-exports.
  - `config.rs`: `Profile`/`ConnectionConfig`/`ForwardingConfig`/`TunnelOptions` + validation.
  - `types.rs`: auth/forwarding enums, tunnel status/events, auth request/response, `StartTunnelResult`.
  - `daemon_client.rs`: `DaemonClientConfig`, connection mode, reqwest client builder, auth header helper.
  - `tls.rs`: rustls client config, fingerprint pinning.
  - `profile_manager.rs`: profile load/save/delete utilities (used by CLI/daemon/GUI).
  - `error.rs`: common error enum (not widely used in newer code paths).
- Daemon
  - `src/main.rs`: startup, logging, PID guard, config load, token handling, router wiring, listeners for Unix/TCP/HTTPS.
  - `api.rs`: axum routes `/api/health`, `/api/tunnels`, start/stop/status/auth, SSE `/api/events`.
  - `tunnel.rs`: `TunnelManager`, SSH connection/auth/forwarding, known_hosts checks, event broadcast, auth prompts.
  - `config.rs`: daemon config file handling, listener modes, CLI snippet writer.
  - `auth.rs`: token generation/persistence, axum middleware.
  - `tls.rs`: self-signed cert generation, rustls server config, fingerprinting.
  - `known_hosts.rs`: parse/verify/write known_hosts.
  - `pidfile.rs`: singleton guard.
  - `monitor.rs`: placeholder.
  - `security.rs`: keyring password retrieval.
  - Related examples: `daemon.toml.example`.
- CLI
  - `src/main.rs`: clap command tree (add/list/edit/delete/info/start/stop/restart/status/daemon/watch), profile creation flow, SSE watcher, auth prompts, daemon client wrapper.
  - `config.rs`: wraps `DaemonClientConfig` for cli.toml I/O.
  - Related examples: `cli.toml.example`.
- GUI
  - `src/main.rs`: GTK/libadwaita bootstrap with Tokio runtime.
  - `ui/window.rs`: main window, header, connection indicator, event listener hook.
  - `ui/sidebar.rs`: profile list, refresh/new-profile dialog wiring.
  - `ui/details.rs`: profile detail pane, start/stop/edit/delete actions, status query.
  - `ui/profile_dialog.rs`: create/edit profile dialog and persistence.
  - `daemon/client.rs`: REST client mirroring CLI.
  - `daemon/sse.rs`: SSE listener/parsing.
  - `models/profile_model.rs`: GObject wrapper around `Profile`.
  - `utils/profiles.rs`: profile directory utilities.

## Dependencies by Module / File
- Common
  - Core: `serde`, `toml`, `serde_json`, `chrono`, `uuid`, `anyhow`, `thiserror`, `dirs`, `tracing`.
  - TLS: `rustls`, `webpki-roots`, `sha2`.
  - HTTP client for daemon access: `reqwest`.
- Daemon
  - Async/HTTP: `tokio`, `axum`, `tower`, `tower-http`, `hyper`, `hyper-util`, `axum-server`.
  - SSH: `russh`, `russh-util` (Eugeny fork), `tokio-util` (codec helpers).
  - TLS: `rcgen`, `rustls`, `tokio-rustls`, `rustls-pemfile`, `sha2`, `time`.
  - Config/serde/logging: `serde`, `toml`, `tracing`, `tracing-subscriber`, `anyhow`, `thiserror`, `dirs`, `chrono`, `uuid`.
  - Security: `secret-service`, `keyring`, `zeroize`, `libc` (PID), `base64` (known_hosts).
  - File-specific highlights:
    - `tunnel.rs`: russh client/auth, `tokio::sync` (broadcast/mpsc/oneshot/RwLock), `tokio::net::TcpListener`, `tokio::io::copy_bidirectional`, auth timeouts, known_hosts integration.
    - `api.rs`: axum extractors, SSE via `axum::response::sse` + `BroadcastStream`.
    - `tls.rs`: cert generation/loading with `rcgen`/`rustls`.
    - `auth.rs`: `zeroize` for token, middleware over axum.
    - `pidfile.rs`: `libc::kill` on Unix for process liveness.
- CLI
  - UI/CLI: `clap`, `dialoguer`, `indicatif`, `colored`, `comfy-table`, `shellexpand`.
  - HTTP: `reqwest` (shares config with daemon_client), SSE parsing via `futures` stream.
  - Storage: `keyring` for password/passphrase.
  - Data: `serde`/`toml`, `chrono`, `dirs`.
- GUI
  - UI: `gtk4`, `libadwaita`, `glib`, `gio`.
  - Async/HTTP: `tokio` runtime, `reqwest`, `futures-util` for SSE streams.
  - Data: `serde`, `toml`, `uuid`, `chrono`, `dirs`, `tracing`.

## Classes (Structs/Enums) by Module
- Common (mostly public)
  - `config.rs`: `Profile`, `ProfileMetadata`, `ConnectionConfig`, `ForwardingConfig`, `TunnelOptions` (all public); defaults/validation methods.
  - `types.rs`: `AuthType`, `ForwardingType`, `TunnelStatus`, `TunnelEvent`, `AuthRequestType`, `AuthRequest`, `AuthResponse`, `StartTunnelResult`, `TunnelStatusResponse`; helper methods on `TunnelStatus`.
  - `daemon_client.rs`: `ConnectionMode`, `DaemonClientConfig`; helper fns `create_daemon_client`, `add_auth_header`.
  - `tls.rs`: internal `FingerprintVerifier` (private); public `create_pinned_tls_config`, `create_insecure_tls_config`.
  - `profile_manager.rs`: public helpers `profiles_dir`, `load_*`, `save_profile`, `delete_profile_*`, `profile_exists_*`.
  - `error.rs`: `Error` enum, `Result` alias.
- Daemon
  - `main.rs`: orchestrates `AppState` (from `api.rs`), `TunnelManager` usage; no new public structs.
  - `api.rs`: `AppState`, `ErrorResponse`, `SuccessResponse`, `TunnelStatusResponse`, `TunnelsListResponse`, `OutgoingEvent`.
  - `tunnel.rs`: `TunnelManager`, `ActiveTunnel`, `PendingAuth`, `AuthContext` (private), `ClientHandler` (private), `TunnelEvent` (daemon-side broadcast enum), `AuthResponseSender`.
  - `config.rs`: `ListenerMode`, `DaemonConfig`.
  - `auth.rs`: `AuthState`.
  - `tls.rs`: helpers; no exported structs beyond functions.
  - `known_hosts.rs`: `KnownHosts`, `VerifyResult`.
  - `pidfile.rs`: `PidFileGuard`.
  - `security.rs`: functions only.
  - Visibility: most are crate-private; external clients interact via HTTP API rather than Rust API.
- CLI
  - `main.rs`: `Cli`, `Commands`, `DaemonCommands`, `TunnelStatusResponse` (local), internal `IncomingEvent` enums.
  - `config.rs`: `CliConfig`.
- GUI
  - `main.rs`: constants only.
  - `ui/window.rs`: `AppState`.
  - `ui/sidebar.rs`: functions only.
  - `ui/details.rs`: functions only.
  - `ui/profile_dialog.rs`: functions only.
  - `daemon/client.rs`: `DaemonClient`, `OperationResponse`, `ErrorResponse`, `TunnelStatusResponse`, `TunnelsListResponse`.
  - `daemon/sse.rs`: `TunnelEvent`, `EventListener`.
  - `models/profile_model.rs`: GObject `ProfileModel` (public methods for profile fields).
  - `utils/profiles.rs`: helpers only.

## Data Model
- Profiles (`Profile`):
  - Metadata: `id: Uuid`, `name`, optional `description`, `created_at`, `modified_at`, `tags`.
  - Connection: `host`, `port`, `user`, `auth_type` (`Key`, `Password`, `PasswordWith2FA`), `key_path`, `password_stored`.
  - Forwarding: `forwarding_type` (`Local`|`Remote`|`Dynamic`), `local_port`, `remote_host`, `remote_port`, `bind_address`.
  - Options: `compression`, `keepalive_interval`, `auto_reconnect`, `reconnect_attempts`, `reconnect_delay`, `tcp_keepalive`, `max_packet_size`, `window_size`.
- Runtime tunnel state:
  - `TunnelStatus`: `NotConnected`, `Connecting`, `WaitingForAuth`, `Connected`, `Disconnecting`, `Disconnected`, `Reconnecting`, `Failed(String)`.
  - Daemon events: `TunnelEvent` (daemon) with variants `Starting`, `Connected`, `Disconnected{reason}`, `Error{error}`, `AuthRequired{request}`.
  - Auth exchange: `AuthRequest` (type, prompt, hidden, tunnel_id) and `AuthResponse`.
  - API status payloads: `TunnelStatusResponse` (status + optional pending auth), `StartTunnelResult` (when starting directly in daemon code).
- Persistence:
  - Profiles stored as TOML under `~/.config/ssh-tunnel-manager/profiles/{uuid}.toml`.
  - Daemon config: `~/.config/ssh-tunnel-manager/daemon.toml`.
  - CLI config: `~/.config/ssh-tunnel-manager/cli.toml`.
  - Auth token: `~/.config/ssh-tunnel-manager/daemon.token`.
  - Known hosts: `~/.config/ssh-tunnel-manager/known_hosts` (custom), can use system one manually.
  - PID file: `$XDG_RUNTIME_DIR/ssh-tunnel-manager/daemon.pid`.
  - Unix socket: `$XDG_RUNTIME_DIR/ssh-tunnel-manager/ssh-tunnel-manager.sock`.

## API Description (daemon HTTP/SSE)
- Authentication: optional X-Tunnel-Token header when `require_auth` is true; enforced via axum middleware.
- Endpoints (from `crates/daemon/src/api.rs`):
  - `GET /api/health` → `"OK"`; 200.
  - `GET /api/tunnels` → `{"tunnels":[{id,status,pending_auth?}]}`; always 200.
  - `POST /api/tunnels/{id}/start` → 202 Accepted on success; 404 if profile missing; 500 on failure.
  - `POST /api/tunnels/{id}/stop` → 200 or 404 if not active; 500 on error.
  - `GET /api/tunnels/{id}/status` → 200 with `TunnelStatusResponse` or 404 if not active.
  - `GET /api/tunnels/{id}/auth` → pending `AuthRequest` or 404 if none.
  - `POST /api/tunnels/{id}/auth` (body `AuthResponse`) → 200 on acceptance; 400 on mismatch/invalid.
  - `GET /api/events` → SSE stream; events serialized as `OutgoingEvent` (`starting`, `connected`, `disconnected`, `error`, `auth_required`).
- Listener modes (daemon config):
  - Unix socket (default, no TLS).
  - TCP HTTP (no TLS; local/dev only; warns on startup).
  - TCP HTTPS (rustls; auto-generates self-signed cert; fingerprint logged + written to CLI snippet).

## Error Handling
- Libraries: `anyhow` for context-rich errors; `thiserror` for `ssh_tunnel_common::Error`.
- Daemon runtime:
  - API handlers translate errors to HTTP codes with JSON `{"error": ...}`.
  - `TunnelManager` sets status to `Failed(reason)` and broadcasts `Error` events on connection/auth/forwarding failures; `fail_tunnel` centralizes status update.
  - Auth timeouts (60s) and connect timeouts (15s) produce failures; privileged port binding returns specific guidance.
  - Known_hosts mismatches are hard failures with detailed logging; unknown keys prompt user via `AuthRequired` host verification prompt.
  - PID guard aborts startup if another instance is running (or removes stale PID).
  - TLS module regenerates cert/key if missing; errors surface during startup.
- Clients:
  - CLI surfaces daemon HTTP status and body; parses SSE errors; prompts user for auth/host-key.
  - GUI shows errors via dialogs and eprintln output; event listener logs warnings on parse errors.

## Testing Overview
- Unit tests present in:
  - `crates/common`: `config.rs` validation, `daemon_client.rs`, `tls.rs`, `profile_manager.rs` (note: some test structs outdated vs current schema), `types.rs` methods.
  - `crates/daemon`: `auth.rs`, `tls.rs`, `known_hosts.rs`, `pidfile.rs`.
  - `crates/cli`: `config.rs`.
- Integration/manual guidance:
  - `TESTING_PLAN.md`: scenario-based plan for UnixSocket/TcpHttp/TcpHttps (auth, TLS pinning).
  - Scripts: `test-cli-snippet.sh`, `test-pidfile.sh`, `test-network-modes.sh` (see root).
  - `QUICK_TEST_GUIDE.md` describes quick checks.
- Gaps:
  - No automated tests for tunnel lifecycle or GUI; `monitor.rs` unimplemented; remote/dynamic forwarding not yet covered.

## Additional Notes / Gaps
- `monitor.rs` is a stub; tunnel health monitoring beyond port-forward loop is future work.
- Remote/dynamic forwarding branches in `tunnel.rs` return "not yet implemented".
- `profile_manager` tests use outdated field names (`username`, `password`); real code paths rely on `ssh-tunnel-common` definitions.
- Security: passwords/passphrases can be stored in system keychain; auth token persisted with 0600 perms; known_hosts uses custom path by default.
- Distribution/packaging and systemd integration are not yet represented in code (see README/SETUP for future plans).

# SSH Tunnel Manager Technical Reference

**Version**: v0.5.0
**Scope**: Current modules, public contracts, persistence, daemon API, GUI boundaries, and
build/validation requirements.

## Crates and executables

| Crate | Kind | Responsibility |
|---|---|---|
| `crates/common` | Library | Shared configuration/types, profile I/O, daemon HTTP/SSE client, TLS pinning, credential-store facade, SSH-key inspection and validation |
| `crates/daemon` | `ssh-tunnel-daemon` | REST/SSE service, SSH lifecycle, authentication, local forwarding, host-key verification, listeners and process security |
| `crates/cli` | `ssh-tunnel` | Profile and tunnel commands, terminal authentication, table/JSON output |
| `crates/gui-core` | Library | Toolkit-neutral client setup, controller/runtime, actions/effects, snapshots, preferences, editor/auth contracts, view models |
| `crates/gui-v2` | `ssh-tunnel-gui` | Production GTK4/libadwaita adapter, first-launch setup, adaptive views, actions, and dialogs |
| `crates/gui-gtk` | `ssh-tunnel-gtk` | Obsolete frozen adapter retained as a non-default workspace member; uses compatibility `AppCore` paths |

Both GUI crates use the workspace GTK 4.22/libadwaita 1.9 binding generation. `gui-v2` is a
default member; obsolete `gui-gtk` is compiled explicitly for compatibility.

## Common library

Primary modules:

- `config.rs`: `Profile`, `ProfileMetadata`, `ConnectionConfig`, `ForwardingConfig`,
  `TunnelOptions`, `PasswordStorage`, defaults and validation.
- `types.rs`: `AuthType`, `ForwardingType`, `TunnelStatus`, `AuthRequestType`, `AuthRequest`,
  `AuthResponse`, `TunnelStatusResponse`, `DaemonInfo`, `StartTunnelRequest`, and source mode.
- `sse.rs`: the shared `EventListener` and daemon-wire `TunnelEvent` variants. `listen()`
  returns immediately and reconnects in the background; `listen_ready()` does not return until
  the stream is established, which a caller that is about to *cause* events must use or it
  races the daemon for its own first events. Reconnect backoff resets after a connection that
  lasted, so early blips cannot pin it at the ceiling for the life of the process.
- `daemon_client.rs`: `ConnectionMode`, `DaemonClientConfig`, client construction, token
  headers, TLS/Unix transport and the shared SSE-first start/stop flow, which is built on
  `EventListener` rather than a second subscription of its own.
  - **Two clients, deliberately.** `create_daemon_client` sets a *total* request timeout;
    `create_streaming_client` sets a **read** timeout instead. A total timeout covers the
    response body, so using the request client for a stream cuts it on schedule regardless of
    traffic. The read timeout is sized against the 10-second heartbeat (two missed beats) and
    the dependency is commented at both ends.
  - `client_credential_applies` is the single decision on whether a prompt may be answered
    from client-side storage: the profile keeps its secret client-side once a legacy
    `keychain` value is resolved against where the daemon runs, **and** the prompt asks for
    something stable for the profile. Submission stays per-client.
- `runtime_paths.rs`: the one place that knows where the daemon's socket and PID file live and
  where clients look for them. Falls back to `/run/ssh-tunnel-manager` when `$XDG_RUNTIME_DIR`
  is unset, which is the case for a service account with no login session, and collapses a
  runtime directory that is already named `ssh-tunnel-manager` instead of nesting a second
  level. These were three independent copies that disagreed under the project's own systemd
  unit.
- `profile_manager.rs`: profile paths, load/save/delete, remote preparation and formatted
  endpoint helpers.
- `keychain.rs`: runtime-selected credential facade using Secret Service or kernel keyutils,
  migration, availability and test injection.
- `ssh_key.rs`: encrypted-key detection, passphrase validation and
  `validate_ssh_key_file`. The file validator applies only to a path on the current host;
  remote daemon paths must not be passed to it.
- `tls.rs`, `network.rs`, `error.rs`: certificate pinning, address classification and common
  error types.

`PasswordStorage` meanings:

| Value | Location | Consumer |
|---|---|---|
| `none` | Not stored | Human prompt |
| `client` | Client host credential store | Attached client answers a structured daemon request |
| `daemon-host` | Daemon host credential store | Daemon |
| `file` | Reserved daemon-host file | WIP / not implemented |
| `keychain` | Legacy ambiguous value | Resolved from structured client/daemon location; no longer written by GUI v2 |

## Daemon

Primary modules:

- `main.rs`: configuration, logging, store selection, token/PID setup and listener startup.
  Refuses to run as uid 0 before anything is written to disk — see `root_refusal`.
- `api.rs`: Axum routes, authentication middleware state, REST responses and outgoing SSE.
- `tunnel.rs`: `TunnelManager`, per-tunnel task, SSH negotiation/authentication, forwarding,
  request-correlated prompts and lifecycle events.
- `known_hosts.rs`: OpenSSH-format verification and append, including hard refusal on changed
  keys.
- `config.rs`: listener modes, `daemon.toml`, CLI snippet and path policy.
- `auth.rs`, `tls.rs`, `pidfile.rs`, `security.rs`: API token, TLS server, singleton guard and
  credential access.
- `monitor.rs`: placeholder; tunnel auto-reconnect/health monitoring remains unimplemented.

External Rust clients interact through HTTP/SSE rather than daemon crate types.

## GUI core

New adapters should build on these modules rather than `AppCore`:

### `actions.rs`

- `AppCommand`: complete presentation-to-controller command enum.
- `ProfileAction`: reusable profile actions for GTK and a future tray.
- `ActionAvailability`: enabled operations derived from `TunnelStatus` and outstanding
  structured operations.
- `ControllerEffect`: side effects emitted by the reducer.

### `controller.rs`

- `AppController`: synchronous reducer and command validator.
- `AppSnapshot`: immutable presentation state with profiles, tunnel inventory, daemon state,
  auth queue, preferences, operations, features and errors.
- `ControllerEvent`: typed completion/input events consumed by the reducer.
- `FeatureState` / `FeatureAvailability`: available, UI-only WIP, or unavailable capability.
- `CommandRejected`, `UiError`, `OperationOutcome`: structured failures and operation results.

Action enablement must come from `ActionAvailability`. Views must not derive it again from
labels or daemon text.

### `client_setup.rs`

- `ClientSetupRepository`: path-injectable discovery, snippet loading, and atomic `cli.toml`
  persistence.
- `ClientSetupDiscovery`: ready, snippet available, or setup-required state selected from
  local file/configuration facts.
- `ClientSetupDraft`: redaction-safe editable settings for Unix socket, HTTP, and HTTPS.
- `ClientSetupValidationErrors`: field and error codes used by adapters without matching
  error text.

Setup runs before `AppRuntime` construction. A missing, unreadable, malformed, or incomplete
configuration cannot silently become a runtime with empty defaults. The daemon API token is
kept out of `Debug` output and the final `cli.toml` is atomically installed with mode `0600`.
Profile passwords and passphrases continue to use the common Secret Service/keyutils facade.

### `runtime.rs`

- `AppRuntime`: executes controller effects for profile files, `ui.toml`, daemon REST/SSE,
  key validation and credential storage.
- `RuntimeResult`: typed events and optional presentation requests returned to the adapter.
- `PresentationRequest`: open editor, confirmation or other UI work that cannot be executed
  by the core.
- `map_sse_event`: maps wire variants to controller events using structured variants and IDs.

Authentication submission always includes both tunnel ID and request ID. The earlier
request-ID-less GUI client method is unavailable.

### `auth.rs`

- `AuthQueue`: FIFO queue, de-duplicated and reconciled by request ID.
- `AuthPromptSnapshot`: presentation-ready prompt with typed `AuthPromptKind` and
  `AuthInputMode`.
- `AuthAnswer` / `AuthSubmission`: typed answer and exact correlation contract.

`prompt` and all other daemon strings are display copy. `AuthRequestType` selects the prompt
kind and answer type; the structured `hidden` boolean selects input visibility. Unknown or
contradictory types fail closed.

### `editor.rs`

- `ProfileEditorSession`, `ProfileEditorDraft`, `ProfileEditorMode`.
- `EditorField` and `EditorValidationErrors` for field-specific validation.
- `SecretValue`: zeroizing secret wrapper with redacted `Debug` output.
- `CredentialUpdate::{Keep, Store, Remove}` and `ProfileSaveRequest`.
- `ProfileDeletionRequest`: confirmation plus credential cleanup contract.

Existing credentials are never loaded into draft strings. Save/delete operations coordinate
profile and credential mutations with rollback. Duplicate creates a new UUID and never copies
the source profile's UUID-scoped credential.

### `preferences.rs`

- `UiPreferencesRepository`: load/reconcile/atomic save.
- `UiPreferences`: version, manual order, pinned IDs, connected-only filter and `SortMode`.
- `CURRENT_UI_PREFERENCES_VERSION`.

Missing preferences use defaults. Malformed optional preferences produce a recoverable
warning. Stale/duplicate IDs are removed and new profiles append deterministically.

### Compatibility and presentation

- `view_models.rs`: `ProfileViewModel`, `ProfileDetailsViewModel`, `StatusColor`.
- `profiles.rs`: shared CRUD/validation helpers.
- `daemon/`: `DaemonClient` and configuration helpers.
- `events.rs`: compatibility `TunnelEventHandler`/`GuiEvent`.
- `state.rs`: legacy `AppCore`, retained for `gui-gtk` only.

## GUI v2 presentation adapter

| Module | Responsibility |
|---|---|
| `lib.rs` | Application/resource bootstrap, system styling and accelerators |
| `window.rs` | Window shell, navigation, action installation, snapshot rendering and presentation requests |
| `setup_wizard.rs` | Non-blocking pre-runtime snippet/manual/repair flow and typed field presentation |
| `bridge.rs` | Dedicated Tokio runtime thread and GLib-safe message bridge |
| `action_router.rs` | Maps shell actions to shared commands or explicit WIP/unavailable presentation |
| `profile_list.rs` | Search/filter/sort/pin/order, rows, details, actions, empty/offline states |
| `profile_editor.rs` | GTK editor bound to the redacted core draft and structured validation |
| `auth_dialog.rs` | One modal keyed by active request ID, typed input/host-key controls, submit/cancel state |
| `daemon_view.rs` | Checking, online, offline and daemon information; lifecycle WIP actions |
| `shell_state.rs` | Presentation mapping for daemon badge, counts and capability labels |
| `components.rs` | Shared WIP, status, wrapping and accessibility components |

GTK objects remain on the GLib main context. After client setup succeeds, the toolkit-neutral
runtime runs on a dedicated Tokio thread; only commands, events, snapshots and presentation
requests cross the bridge.

Application shortcuts:

- `Ctrl+F`: focus profile search
- `Ctrl+N`: new profile
- `F5` / `Ctrl+R`: refresh
- `Ctrl+1`: Profiles
- `Ctrl+2`: Daemon

## Persistence

| Data | Location |
|---|---|
| Profiles | `${XDG_CONFIG_HOME:-~/.config}/ssh-tunnel-manager/profiles/<uuid>.toml` |
| Client config | `${XDG_CONFIG_HOME:-~/.config}/ssh-tunnel-manager/cli.toml` |
| GUI preferences | `${XDG_CONFIG_HOME:-~/.config}/ssh-tunnel-manager/ui.toml` |
| Daemon config/token/known hosts | Same configuration directory |
| Socket/PID | `$XDG_RUNTIME_DIR/ssh-tunnel-manager/` |
| Credentials | Secret Service or kernel keyutils, service `ssh-tunnel-manager`, account `<profile-uuid>` |

`ui.toml` is presentation-only. Profile TOML remains authoritative for tunnel configuration.

## Daemon HTTP/SSE API

- `GET /api/health`: liveness.
- `GET /api/daemon/info`: structured daemon information.
- `GET /api/tunnels`: current inventory and pending authentication.
- `POST /api/tunnels/{id}/start`: start using daemon-local or hybrid profile input.
- `POST /api/tunnels/{id}/stop`: stop/cancel.
- `GET /api/tunnels/{id}/status`: status and pending auth.
- `GET /api/tunnels/{id}/auth`: exact outstanding request.
- `POST /api/tunnels/{id}/auth`: request-ID-correlated `AuthResponse`.
- `GET /api/events`: `starting`, `connected`, `disconnected`, `error`, `auth_required`, and
  `heartbeat` SSE events. A **global, unfiltered broadcast** — consumers filter by tunnel id;
  heartbeats concern everyone. On subscribe, the daemon **replays an `auth_required` for every
  tunnel with an outstanding prompt**, so a client that connects mid-flight learns what is
  waiting rather than waiting for an event that has already been sent.
- `POST /api/daemon/shutdown`: existing shutdown endpoint; GUI v2 does not expose it as an
  available default capability. No start/restart daemon API exists.

Listener modes are Unix socket (default), loopback-only TCP HTTP, and TCP HTTPS with token
authentication and optional certificate fingerprint pinning.

## Error and security boundaries

- Daemon/API errors become typed controller outcomes before presentation.
- Error strings can be displayed but never searched, tokenized or compared to choose an
  action.
- Changed known host keys are hard failures. Only the structured unknown-host request selects
  an accept/reject dialog.
- Host-key prompt text remains unparsed until the daemon supplies structured host,
  algorithm, fingerprint and trust-status fields.
- Unsupported forwarding types are preserved and labelled unsupported; they are never
  rewritten as local forwarding.
- WIP actions cannot report success.

## Build and validation

- Pinned toolchain: Rust 1.98.0.
- Production GUI target: Fedora 44/Bazzite with GTK 4.22, libadwaita 1.9 and GLib 2.88.
- CLI/daemon also require `cmake` and a C toolchain for `aws-lc-sys`.

Root workspace:

```bash
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo build --release --locked
cargo deny check
cargo check --package ssh-tunnel-gui-gtk --locked
```

Functional runtime acceptance and production cutover are complete. Keyboard-only and mockup
screen-comparison follow-up remains in `.plan/UI_v2-final-validation.md`.

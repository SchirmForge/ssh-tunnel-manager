# Epic 11 — Second-generation desktop GUI

The adaptive GTK 4/libadwaita production interface and the toolkit-neutral application core
behind it.

The second-generation GUI became the production desktop application after runtime and
functional acceptance. Its package and binary are `ssh-tunnel-gui`, with application ID
`io.github.schirmforge.SshTunnelManager`. Keyboard-only and mockup-comparison follow-up is
tracked separately; the obsolete `gui-gtk` remains frozen in the workspace.

[← Back to index](README.md)

---

## US-11.1 — Find and organize profiles quickly ⚠️

**As a** desktop user with many profiles  
**I want** to search, filter, pin, sort and reorder them  
**So that** the connections I use most remain easy to reach.

**Implemented**

- Search matches normalized profile name, description, host, user and forwarding summary
- Connected-only filtering and name/manual sorting
- Pinned and unpinned sections, drag ordering, and keyboard Move up/Move down actions
- Manual movement cannot cross a pin-section boundary; name sorting disables reordering
- Order, pins, filter and sort persist atomically in versioned `ui.toml`
- Missing/malformed preferences recover safely; stale IDs disappear and new IDs append
  deterministically

**Remaining**: keyboard-only and visual-parity checks in
`.plan/UI_v2-final-validation.md`.

**Implementation**: `crates/gui-core/src/preferences.rs`, `controller.rs`,
`crates/gui-v2/src/profile_list.rs`

---

## US-11.2 — Edit profiles without exposing or misplacing credentials ✅

**As a** desktop user  
**I want** profile and credential edits to be explicit and recoverable  
**So that** saving a connection cannot leak, duplicate, or silently move its secret.

**Implemented**

- Existing credentials never enter editor draft fields or debug output
- Explicit Keep, Store and Remove operations coordinate profile and credential persistence
  with rollback
- Client-held secrets use the common Secret Service/keyutils facade
- Legacy `keychain` resolves from structured local/remote connection mode and is not written
- Duplicate creates a new UUID and never copies the source credential
- Delete requires confirmation and coordinates client-held credential removal
- Local key paths are validated on the client; daemon-host key paths are preserved without a
  false local file check
- Auth, account/password, 2FA and auto-reconnect choices reveal/hide and validate their actual
  local fields; unsupported backend behavior is labelled WIP
- Remote/dynamic profiles are preserved and labelled unsupported rather than rewritten

The user accepted profile and credential-store behavior during production cutover.

**Implementation**: `crates/gui-core/src/editor.rs`, `runtime.rs`,
`crates/common/src/ssh_key.rs`, `crates/gui-v2/src/profile_editor.rs`

---

## US-11.3 — Answer the daemon's exact authentication request safely ✅

**As a** desktop user  
**I want** authentication dialogs tied to the daemon's actual request  
**So that** delayed, misleading or localized text cannot submit the wrong answer.

**Implemented**

- Requests are queued FIFO, de-duplicated and correlated by request and tunnel IDs
- One GTK modal is keyed by the active request ID; double submission is prevented
- Password, key-passphrase, keyboard-interactive, structured two-factor and host-key controls
  are selected from `AuthRequestType`
- The structured `hidden` boolean controls input visibility
- Submitted/cancelled dialogs wait for structured SSE or inventory completion
- A client-held password/passphrase is offered only for its matching request code and only
  once per connection attempt
- Unknown or contradictory structured data fails closed
- Prompt, instruction, name, error and descriptive status strings are display-only and never
  searched or parsed to choose an action

The user accepted authentication behavior during production cutover. Daemon text remains
display-only and never selects actions.

**Implementation**: `crates/gui-core/src/auth.rs`, `controller.rs`, `runtime.rs`,
`crates/gui-v2/src/auth_dialog.rs`

---

## US-11.4 — Understand daemon, empty, offline and unavailable states ✅

**As a** desktop user  
**I want** the interface to distinguish real daemon state from unavailable features  
**So that** I never mistake placeholder content for a successful operation.

**Implemented**

- Structured checking, online and offline states with real health refresh/retry
- A structured pre-runtime setup-required state for missing, invalid, or incomplete
  `cli.toml`, with snippet import, manual repair, retry, cancellation, and secure save
- Daemon information comes from `DaemonInfo`
- Empty-profile create action is real; SSH config import is visibly WIP
- Daemon start/restart/shutdown controls are WIP under the current capability scope
- Unsupported forwarding, compression/runtime options, stored TOTP and unavailable telemetry
  are identified instead of fabricated
- Existing daemon-wide uptime may be shown; no new traffic or last-connected data is invented

Live daemon/SSE behavior was accepted during production cutover. Lifecycle APIs, import, and
new telemetry remain separate deferred features.

**Implementation**: `crates/gui-v2/src/daemon_view.rs`, `profile_list.rs`,
`action_router.rs`, `setup_wizard.rs`, `crates/gui-core/src/controller.rs`,
`client_setup.rs`

**Tests**: `client_setup::tests::{discovery_distinguishes_missing_and_snippet_available,
discovery_reports_malformed_incomplete_and_ready_config,
snippet_with_bind_all_host_requires_structured_host_completion,
persistence_replaces_atomically_and_secures_the_target}` and `setup_wizard::tests`.

---

## US-11.5 — Use an adaptive and accessible system-native interface ⚠️

**As a** desktop user  
**I want** the GUI to follow my desktop fonts, colors and input method  
**So that** it remains usable with my accessibility and display settings.

**Implemented**

- System fonts and semantic GTK/libadwaita colors; no bundled family or fixed color literals
- Labelled icon/form controls and accessible alert/status roles
- Search, navigation, refresh, new-profile and profile-reorder keyboard actions
- Focus/default actions for editor and authentication dialogs
- Wrapping action/metric layouts and bounded scrollers for narrow or long content
- System light/dark styling

**Remaining**: actual keyboard traversal/focus-order, Orca names/roles/live announcements,
high contrast, enlarged system fonts, narrow resizing, long/localized content and visual
comparison in light/dark modes.

**Implementation**: `crates/gui-v2/src/lib.rs`, `components.rs`, `window.rs`, and view modules

---

## US-11.6 — Reuse the same profile actions from a future tray ⚠️

**As a** desktop user  
**I want** tray and main-window actions to behave identically  
**So that** the same connection cannot have two competing implementations.

**Implemented**

- Toolkit-neutral `AppCommand`, `ProfileAction`, `ActionAvailability` and `AppSnapshot`
- Main-window actions dispatch those shared commands instead of calling daemon/profile APIs
  directly
- Feature availability and operation errors are shared presentation state

**Remaining**: tray UX design, platform behavior, adapter implementation and tray-library
selection. No tray dependency was added in v0.4.0.

**Implementation**: `crates/gui-core/src/actions.rs`, `controller.rs`,
`crates/gui-v2/src/action_router.rs`

---

## US-11.7 — Install GUI v2 as the production desktop application ✅

**As a** desktop user  
**I want** the validated second-generation GUI installed and launched normally  
**So that** I do not need a source checkout or preview executable.

**Status**: **Implemented.** `crates/gui-v2` is a root-workspace default member. CI, the
Makefile, development sandbox, installer, desktop entry, and current launch documentation use
`ssh-tunnel-gui` and `io.github.schirmforge.SshTunnelManager`.

The obsolete `gui-gtk` stays in the workspace on the same GTK binding generation, is excluded
from default/install/package targets, and receives no updates. Removal is not planned.

**Evidence**: `crates/gui-v2/PHASE7_VALIDATION.md`

---

## v0.4.0 scope boundary

Tray rendering, stored TOTP, daemon start/restart APIs, SSH config import, new telemetry,
prompt-string parsing, and non-Fedora GUI v2 compatibility are not silently included in this
epic's cutover gate. Their authoritative dispositions are in
[the roadmap](../ROADMAP.md#deferred-or-excluded-from-v040).

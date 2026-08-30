# Epic 6 — Real-time status

The event stream that keeps clients in step with the daemon, and what users see because of
it.

[← Back to index](README.md)

---

## US-6.1 — See status change as it happens ✅

**As a** user
**I want** the interface to update itself
**So that** I am not re-running a status command or clicking refresh.

**Acceptance criteria**
- `GET /api/events` is a Server-Sent Events stream carrying `starting`, `connected`, `disconnected`, `error`, `auth_required` and `heartbeat`
- Both the CLI and the GUI consume it through the same `EventListener` in `crates/common`, so their behaviour cannot drift
- The GUI updates status dots on the profile list the moment an event arrives
- `ssh-tunnel watch [name]` streams events to the terminal, optionally filtered to one profile

**Implementation**: `crates/common/src/sse.rs`, `crates/daemon/src/api.rs` (`events`), `crates/cli/src/main.rs` (`watch_events`)
**Tests**: `daemon_api::event_stream_connects_and_sends_heartbeats`

---

## US-6.2 — Know when the daemon has gone away ✅

**As a** desktop user
**I want** the GUI to tell me it has lost the daemon
**So that** stale status is not mistaken for live status.

**Acceptance criteria**
- A periodic heartbeat is emitted on the event stream
- The GUI treats 30 seconds without a heartbeat as loss of the daemon
- A network icon with a tooltip shows daemon availability
- On reconnection the client re-queries current status, so nothing missed while disconnected is left stale

**Implementation**: `crates/gui-gtk/src/ui/window.rs`, `crates/gui-core/src/state.rs`

---

## US-6.3 — Have the stream recover on its own ✅

**As a** user
**I want** a dropped event stream to reconnect

**Acceptance criteria**
- `EventListener` reconnects automatically with exponential backoff, capped at 30 seconds
- It stops cleanly when the consumer goes away rather than retrying forever
- A dropped stream is logged at debug level, not as an error — a client disconnecting is normal

**Implementation**: `crates/common/src/sse.rs` (`EventListener::listen`), `crates/daemon/src/main.rs` (error categorisation)

---

## US-6.4 — Be prompted for credentials by whichever client is attached ✅

**As a** user
**I want** the authentication prompt to appear where I am working

**Acceptance criteria**
- `auth_required` events carry the request id, type, prompt text and whether input should be hidden
- The GUI opens a dialog; the CLI prompts in the terminal
- The GUI's dialog state is driven by SSE, not by local optimism: it stays open showing "Verifying…" until an event confirms the next state
- A `connected` event closes the dialog, another `auth_required` replaces it, an `error` closes it

**Implementation**: `crates/gui-gtk/src/ui/auth_dialog.rs`, `crates/common/src/daemon_client.rs` (`TunnelEventHandler`)

---

## US-6.5 — Have the event stream protected like everything else ✅

**As an** operator
**I want** the event stream to require authentication

**Acceptance criteria**
- `GET /api/events` requires the token, exactly as the REST endpoints do
- An unauthenticated subscription attempt is refused with 401 rather than opening an unauthenticated stream

**Implementation**: `crates/daemon/src/api.rs`
**Tests**: `daemon_api::event_stream_requires_authentication`

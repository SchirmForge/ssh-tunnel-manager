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

**Implementation**: `crates/gui-gtk/src/ui/window.rs`, `crates/gui-core/src/state.rs`,
`controller.rs`, `runtime.rs`, `crates/gui-v2/src/window.rs`, `daemon_view.rs`

---

## US-6.3 — Have the stream recover on its own ✅

**As a** user
**I want** a dropped event stream to reconnect

**Acceptance criteria**
- `EventListener` reconnects automatically with exponential backoff, capped at 30 seconds
- The stream is built with a **read** timeout, not a total request timeout: it ends when
  nothing arrives, never on a fixed schedule while traffic is flowing
- The read timeout is sized against the heartbeat interval (two missed beats), and neither may
  be changed without the other
- Backoff **resets after a connection that lasted**, so early failures cannot pin it at the
  ceiling for the life of the process
- It stops cleanly when the consumer goes away rather than retrying forever
- A dropped stream is logged at debug level, not as an error — a client disconnecting is normal

**Implementation**: `crates/common/src/sse.rs` (`EventListener::listen`, `Backoff`),
`crates/common/src/daemon_client.rs` (`create_streaming_client`),
`crates/daemon/src/main.rs` (error categorisation)
**Tests**: `sse::tests` (backoff resets and grows),
`daemon_api::event_stream_survives_longer_than_the_client_timeout`

> **This story was marked done in v0.4.0 while it was not.** The stream carried a *total*
> request timeout, which in `reqwest` covers the response body, so every stream was cut at
> exactly 30 seconds regardless of traffic; the backoff then doubled without ever resetting.
> The result was a permanent 30s-connected / 30s-disconnected cycle in which roughly half of
> all events reached nobody. The first three criteria above are the properties that were
> missing, and they are now pinned by tests. See v0.5.0.

---

## US-6.4 — Be prompted for credentials by whichever client is attached ✅

**As a** user
**I want** the authentication prompt to appear where I am working

**Acceptance criteria**
- `auth_required` events carry the request id, type, prompt text and whether input should be hidden
- The GUI opens a dialog; the CLI prompts in the terminal
- The GUI's dialog state is driven by SSE, not by local optimism: it stays open showing "Verifying…" until an event confirms the next state
- A `connected` event closes the dialog, another `auth_required` replaces it, an `error` closes it
- The same `auth_required` may arrive more than once. Clients dedupe on the request id and
  answer once: recording the prompt is idempotent, and a request already answered is ignored

**Implementation**: `crates/gui-gtk/src/ui/auth_dialog.rs`,
`crates/common/src/daemon_client.rs` (`TunnelEventHandler`),
`crates/gui-core/src/auth.rs`, `controller.rs`, `crates/gui-v2/src/auth_dialog.rs`

GUI v2 queues simultaneous requests FIFO, keys its one active modal by request ID and waits
for structured SSE/inventory completion. Changing prompt wording cannot change the selected
dialog or completion behavior.

---

## US-6.5 — Have the event stream protected like everything else ✅

**As an** operator
**I want** the event stream to require authentication

**Acceptance criteria**
- `GET /api/events` requires the token, exactly as the REST endpoints do
- An unauthenticated subscription attempt is refused with 401 rather than opening an unauthenticated stream

**Implementation**: `crates/daemon/src/api.rs`
**Tests**: `daemon_api::event_stream_requires_authentication`

---

## US-6.6 — Learn what is outstanding when I connect mid-flight ✅

**As a** user whose client reconnected, or who opened a second client

**I want** to be shown any authentication prompt that is still waiting

**so that** a prompt raised while nothing was listening is not lost until it times out.

**Acceptance criteria**
- On subscribing to `GET /api/events`, the daemon emits an `auth_required` for every tunnel
  with an outstanding prompt, before streaming live events
- Re-delivery keeps the **same request id**. The id identifies the question, not the delivery,
  so an answer from a client that was already waiting still matches the daemon's pending
  request
- A genuinely new question — the prompt following a rejected password — still gets a new id
- Only prompts that are still outstanding are replayed; an answered one is gone
- A second client may be asked the same question; the first answer wins and the loser's submit
  is dropped quietly rather than surfaced as an error

**Why not a fresh id per delivery**: `submit_auth_by_request_id` matches the pending request's
id, so reissuing it would stale the id a waiting client holds and make its answer fail with
"Request ID mismatch" — correctness would depend on how many clients happened to be attached.

**Implementation**: `crates/daemon/src/api.rs` (`event_stream` replay),
`crates/daemon/src/tunnel.rs` (`TunnelManager::list_pending_auth`),
`crates/common/src/daemon_client.rs` (`AnsweredRequests`),
`crates/gui-core/src/state.rs` (idempotent `add_pending_auth`)
**Tests**: `live_ssh::a_late_subscriber_is_told_about_an_outstanding_prompt` (verified to fail
without the replay), `gui-core::state` idempotency tests

> The production GUI's event delivery was accepted during cutover; remaining manual checks
> are listed in [KNOWN_ISSUES.md](../KNOWN_ISSUES.md).

---

## US-6.7 — Have one event path behind every client ✅

**As a** contributor

**I want** the CLI and the GUIs to consume daemon events through the same code

**so that** a fix to event delivery cannot reach one client and miss another.

**Acceptance criteria**
- `start_tunnel_with_events` consumes the shared `EventListener` rather than opening and
  parsing a second SSE subscription of its own
- A caller that is about to cause events subscribes first: `EventListener::listen_ready()`
  does not return until the stream is established
- `/api/events` is a global broadcast, so consumers filter by tunnel id; heartbeats concern
  every subscriber
- Whether a prompt may be answered from client-side storage is decided in exactly one place
  (`client_credential_applies`); submission stays per-client

**Implementation**: `crates/common/src/daemon_client.rs`, `crates/common/src/sse.rs`
**Tests**: `daemon_client::tests` (event filtering, credential policy), the live tier

> This is why US-6.3's defect survived: the GUI used `EventListener` while the CLI hand-rolled
> its own subscription on the request client, so fixing the timeout on one path left the other
> broken. `gui-core::events` — a `TunnelEventHandler` with no implementors anywhere — was
> removed at the same time.

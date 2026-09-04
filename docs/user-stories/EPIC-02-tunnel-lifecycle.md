# Epic 2 — Tunnel lifecycle

Starting, stopping, restarting and inspecting tunnels, and what the forwarding actually
does.

[← Back to index](README.md)

---

## US-2.1 — Start a tunnel ✅

**As a** user
**I want** to start a tunnel from a profile
**So that** I can reach a remote service through a local port.

**Acceptance criteria**
- `ssh-tunnel start <name>`, or the Start button in the GUI, begins the connection
- The client subscribes to the event stream *before* sending the start request, so no event fired immediately after starting is missed
- Progress is visible: Starting → (authentication prompts) → Connected
- Failure reports why — host unreachable, authentication rejected, port unavailable — rather than a generic error

**Implementation**: `common::daemon_client::start_tunnel_with_events`, `crates/daemon/src/tunnel.rs` (`run_tunnel`, `establish_connection`)
**Tests**: `live_ssh::key_authentication_connects_and_forwards_traffic`, `password_authentication_connects`

---

## US-2.2 — Local port forwarding carries traffic ✅

**As a** user
**I want** connections to my local port to reach the remote service
**So that** the tunnel is actually useful.

**Acceptance criteria**
- The daemon binds `bind_address:local_port` and forwards each accepted connection to `remote_host:remote_port` through the SSH session
- Multiple simultaneous connections through one tunnel are supported
- The `Connected` event is emitted only *after* the local port is successfully bound, so a client never shows "connected" for a tunnel that failed to bind

**Implementation**: `crates/daemon/src/tunnel.rs` (`run_local_forward_task`, `handle_forward_connection`)
**Tests**: `live_ssh::key_authentication_connects_and_forwards_traffic` reads the SSH banner back through the forwarded port

---

## US-2.3 — Stop a tunnel ✅

**As a** user
**I want** to stop a tunnel and have it stop promptly

**Acceptance criteria**
- `ssh-tunnel stop <name>`, or the Stop button, tears the tunnel down
- Stopping works both when connected and while the tunnel is still connecting or awaiting authentication
- Stopping mid-authentication returns promptly rather than waiting out the authentication timeout
- The local port is released

**Implementation**: `crates/daemon/src/tunnel.rs` (`TunnelManager::stop`)
**Tests**: `live_ssh::stopping_a_connected_tunnel_returns_promptly`, `live_ssh::stopping_during_authentication_returns_promptly`

> Until v0.1.11 `stop` held the tunnel-state write lock across the await on the tunnel task,
> which the task needed to finish, so every cancellation during authentication timed out and
> force-aborted the task. See [ROADMAP.md](../ROADMAP.md#technical-debt).

---

## US-2.4 — Stop every tunnel at once ✅

**As a** CLI user shutting down
**I want** one command to stop everything

**Acceptance criteria**
- `ssh-tunnel stop --all` stops every active tunnel
- The active list comes from the daemon, not from local guesswork
- `--all` and a profile name are mutually exclusive
- Reports which tunnels were stopped

**Implementation**: `crates/cli/src/main.rs` (`Commands::Stop`, `all`)

---

## US-2.5 — Restart a tunnel ✅

**As a** user whose tunnel has gone stale
**I want** to restart it in one step

**Acceptance criteria**
- `ssh-tunnel restart <name>` performs a stop followed by a start
- The start waits for the stop to complete
- Authentication prompts appear as normal on the way back up
- After saving an active profile, the production GUI offers **Reconnect now**, waits for a
  structured inactive status, and starts the saved profile

**Implementation**: `crates/cli/src/main.rs` (`Commands::Restart`),
`crates/gui-core/src/actions.rs`, `controller.rs`, `runtime.rs`

---

## US-2.6 — Check tunnel status ✅

**As a** user
**I want** to know whether a tunnel is up

**Acceptance criteria**
- `ssh-tunnel status <name>` reports the state of one tunnel
- `ssh-tunnel status --all` prints a table of every active tunnel
- States are colour-coded, and any pending authentication request is surfaced
- Reported states: NotConnected, Connecting, WaitingForAuth, Connected, Disconnecting, Disconnected, Failed(reason)

**Implementation**: `crates/cli/src/main.rs` (`Commands::Status`), `common::types::TunnelStatus`
**Tests**: `daemon_api::listing_tunnels_is_empty_on_a_fresh_daemon`, `status_of_an_unknown_tunnel_is_not_found`

---

## US-2.7 — Be told clearly when a privileged port is refused ✅

**As a** user forwarding to a port below 1024
**I want** to understand why it failed and what to do

**Acceptance criteria**
- Binding a port ≤ 1024 without the capability produces an explanation, not a bare permission error
- The message names the options: run as a system service, use `CAP_NET_BIND_SERVICE`, or pick a higher port
- The check happens early, with a warning at profile creation time as well as at connect time

**Implementation**: `crates/daemon/src/tunnel.rs`, `crates/cli/src/main.rs`

---

## US-2.8 — Remote and dynamic forwarding ❌

**As a** user
**I want** remote (`ssh -R`) or SOCKS (`ssh -D`) forwarding

**Status**: **Not implemented.** Both return an explicit error from the daemon rather than
failing obscurely.

- **Remote forwarding** is **not planned** — use `ssh -R` directly.
- **Dynamic/SOCKS** is a future, unscheduled item.

Note that `ForwardingType::Local` is currently hardcoded at profile creation in both
clients, so adding dynamic forwarding is not a daemon-only change.

**Implementation**: `crates/daemon/src/tunnel.rs` (`monitor_tunnel`, forwarding match)
**See**: [ROADMAP.md](../ROADMAP.md#not-planned)

---

## US-2.9 — Tunnels reconnect themselves after a drop ❌

**As a** user with an unreliable network
**I want** a dropped tunnel to come back on its own

**Status**: **Not implemented.** `auto_reconnect`, `reconnect_attempts` and
`reconnect_delay` exist in the profile, are editable in the CLI and GUI, and are read by the
daemon — but nothing acts on them. `run_tunnel()` connects exactly once, and
`crates/daemon/src/monitor.rs` is an empty stub.

The design has been decided (a global setting with a per-profile override, attempted only
where authentication needs no human) but not built. See
[ROADMAP.md](../ROADMAP.md#auto-reconnect-and-health-monitoring).

**Workaround**: `ssh-tunnel restart <name>`.

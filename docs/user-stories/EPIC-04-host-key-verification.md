# Epic 4 — Host key verification

Establishing that the server you reached is the server you meant, and noticing when it
changes.

[← Back to index](README.md)

---

## US-4.1 — Be asked before trusting a new server ✅

**As a** security-conscious user
**I want** to see and approve a server's fingerprint on first connection
**So that** I am not silently trusting whatever answers.

**Acceptance criteria**
- Connecting to a host absent from `known_hosts` raises an `AuthRequired` event of type `HostKeyVerification`
- The prompt shows the host, the key type and the SHA-256 fingerprint
- Declining aborts the connection; nothing is written
- Accepting proceeds and records the key

**Implementation**: `crates/daemon/src/known_hosts.rs`, `crates/daemon/src/tunnel.rs` (`ClientHandler::check_server_key`)
**Tests**: `live_ssh::first_connection_prompts_for_host_key_and_remembers_it`

---

## US-4.2 — Not be asked again ✅

**As a** user
**I want** an accepted host remembered
**So that** routine connections are not interrupted.

**Acceptance criteria**
- The accepted key is appended to `~/.config/ssh-tunnel-manager/known_hosts` in OpenSSH format
- The file is created `0600`
- Subsequent connections to the same host and port proceed with no prompt
- Non-standard ports are recorded in `[host]:port` form and matched correctly

**Implementation**: `crates/daemon/src/known_hosts.rs` (`KnownHostEntry`, `format_host_pattern`)
**Tests**: `known_hosts::tests::test_known_hosts_save_and_load`, `test_known_host_entry_matches_with_port`, `live_ssh::first_connection_prompts_for_host_key_and_remembers_it`

---

## US-4.3 — Be protected when a host key changes ✅

**As a** user
**I want** the connection refused if a server presents a different key
**So that** I am not silently man-in-the-middled.

**Acceptance criteria**
- A key that does not match the stored entry for that host aborts the connection
- The refusal is hard: there is no "accept anyway" prompt in this path
- The reason is reported clearly enough to distinguish a genuine server rebuild from an attack

**Implementation**: `crates/daemon/src/known_hosts.rs`
**Tests**: `live_ssh::a_changed_host_key_is_refused`

---

## US-4.4 — Point at a different known_hosts file ✅

**As an** operator
**I want** to choose which `known_hosts` the daemon uses
**So that** I can share the system one or isolate the daemon's.

**Acceptance criteria**
- `known_hosts_path` in `daemon.toml` overrides the default
- The default is `~/.config/ssh-tunnel-manager/known_hosts`, kept separate from `~/.ssh/known_hosts` so the daemon cannot alter the user's own file unexpectedly
- The path is reported in `GET /api/daemon/info`

**Implementation**: `crates/daemon/src/config.rs` (`known_hosts_path`), `crates/daemon/src/api.rs` (`DaemonInfo`)

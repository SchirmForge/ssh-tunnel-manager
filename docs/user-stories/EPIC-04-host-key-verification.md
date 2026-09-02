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

GUI v2 selects the accept/reject surface only from the structured
`HostKeyVerification` request code. Until the daemon adds structured host, algorithm and
fingerprint fields, it displays the daemon prompt without parsing security facts from it.

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
**Tests**: `known_hosts::a_different_key_for_a_known_host_is_reported_as_mismatch`,
`known_hosts::the_same_key_for_a_known_host_is_trusted`,
`live_ssh::a_changed_host_key_is_refused`

The changed-key path never emits the structured unknown-host request, so GUI v2 cannot turn
its explanatory error text into an “accept anyway” action.

> **The live test was asserting nothing until v0.2.0.** It wrote its poisoned `known_hosts`
> entry as `[host]:22`, but a bare hostname is the correct form on the default port —
> matching OpenSSH — so the entry never matched and the daemon reported the host as
> *unknown* rather than *changed*. The daemon was right throughout; the test was not. The
> `Mismatch` branch now also has unit coverage, so this no longer depends on a live server.

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

---

## US-4.5 — Not have a host certificate substituted for a host key ✅

**As a** user
**I want** a server presenting a CA-signed certificate to be refused rather than accepted
**So that** trust cannot be established by a certificate authority I never approved.

**Acceptance criteria**
- A server presenting a host certificate instead of a plain host key aborts the connection
- The error says why, and names host certificates specifically
- The certificate's embedded public key is **not** compared against `known_hosts`

**Implementation**: `crates/daemon/src/tunnel.rs` (`plain_host_key`)
**Tests**: `tunnel::a_host_certificate_is_refused_rather_than_unwrapped`,
`tunnel::a_plain_host_key_is_passed_through_unchanged`

> russh 0.63 can hand `check_server_key` a certificate rather than a key. The obvious
> migration calls `PublicKeyOrCertificate::public_key()`, which returns the key embedded
> *inside* the certificate — and comparing that against `known_hosts` is a different check
> entirely: it would accept a host that was never pinned, on the strength of a CA the client
> does not evaluate. Refusing is the honest answer until `@cert-authority` trust is
> implemented deliberately, which is a feature rather than a migration detail.
>
> No live test would have caught the wrong choice: test servers present plain host keys.
> The guard is a unit test built on a real `ssh-keygen`-generated certificate.

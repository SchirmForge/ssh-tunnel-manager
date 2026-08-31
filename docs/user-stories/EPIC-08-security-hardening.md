# Epic 8 — Security hardening

The protections that apply whether or not the user asks for them.

See [../architecture/SECURITY.md](../architecture/SECURITY.md) for the threat model and the
full rationale.

[← Back to index](README.md)

---

## US-8.1 — Have my files protected from other users ✅

**As a** user on a shared machine
**I want** my configuration and credentials unreadable by others

**Acceptance criteria**
- Every sensitive file is created `0600`: `cli.toml`, `daemon.toml`, `daemon.token`, `known_hosts`, `server.crt`, `server.key`, `cli-config.snippet` and profiles
- The runtime directory is `0700` and the Unix socket `0600`
- The daemon sets `umask 0077` at startup, so a permissive umask inherited from the parent shell cannot loosen anything
- Permissions are set explicitly rather than relying on the umask alone

**Implementation**: `crates/daemon/src/permissions.rs`, `crates/daemon/src/main.rs`
**Tests**: `daemon_api::runtime_directory_and_socket_are_owner_only`, `token_file_is_not_world_readable`, `auth::tests::test_token_file_permissions`, `config::tests::test_cli_config_snippet_permissions`

---

## US-8.2 — Share daemon access with a group deliberately ✅

**As an** operator running a system daemon
**I want** to grant a specific group access
**So that** several users can manage tunnels without the socket being world-accessible.

**Acceptance criteria**
- `group_access = true` relaxes the runtime directory to `0770` and the socket to `0660`
- The default remains owner-only; group access is opt-in
- The current setting is reported in `GET /api/daemon/info`

**Implementation**: `crates/daemon/src/permissions.rs` (`set_socket_permissions`), `crates/daemon/src/config.rs`

---

## US-8.3 — Not be able to expose the daemon by accident ✅

**As an** operator
**I want** insecure configurations refused
**So that** a careless setting cannot silently expose the API.

**Acceptance criteria**
- Plain HTTP bound to any non-loopback address is refused **at startup**, not at first request
- The refusal explains that network access requires `tcp-https`
- IPv6 loopback is recognised as loopback; IPv6 network addresses are rejected
- Authentication defaults to on in every mode

**Implementation**: `crates/daemon/src/config.rs` (`validate`), `crates/common/src/network.rs`
**Tests**: `daemon_api::http_mode_is_refused_on_a_non_loopback_address`, `config::tests::test_validate_tcp_http_*`, `test_default_require_auth_is_true`

---

## US-8.4 — Have credentials kept out of logs and process listings ✅

**As a** user
**I want** my secrets not to leak into places I forget to check

**Acceptance criteria**
- The auth token is masked when logged, showing only a short suffix
- Passwords and passphrases are never logged
- No credential is passed as a command-line argument, so nothing appears in `ps`
- Sensitive buffers use `zeroize` to clear on drop

**Implementation**: `crates/daemon/src/auth.rs` (`obfuscate_token`), `crates/common` (`zeroize`)
**Tests**: `auth::tests::test_obfuscate_token`

> **Known gap**: `cli-config.snippet` contains the token in plaintext. The file is `0600`,
> but it persists. See [../KNOWN_ISSUES.md](../KNOWN_ISSUES.md).

---

## US-8.5 — Run with no more privilege than needed ✅

**As an** operator
**I want** the daemon to run unprivileged

**Acceptance criteria**
- The daemon runs as an ordinary user; root is not required or expected
- Privileged ports are handled by granting `CAP_NET_BIND_SERVICE` or by running a system service, not by running everything as root
- Only one instance can run at a time, guarded by a PID file that recovers from stale and corrupt files

**Implementation**: `crates/daemon/src/pidfile.rs`, `docs/architecture/SYSTEMD.md`
**Tests**: `daemon_api::second_daemon_instance_refuses_to_start`, `pidfile::tests::test_stale_pid_file_is_reclaimed`, `test_unparseable_pid_file_is_replaced`

---

## US-8.6 — Have private keys stay where I put them ✅

**As a** user
**I want** the tool never to copy or move my SSH keys

**Acceptance criteria**
- Keys are referenced by path and read at connection time
- No key is written to a new location, transmitted, or embedded in a profile
- Key permissions are validated before use
- For remote daemons only the filename is sent — see US-7.3 in
  [Epic 7](EPIC-07-remote-daemon.md)

**Implementation**: `crates/daemon/src/tunnel.rs` (`authenticate_with_key`), `crates/common/src/profile_manager.rs`

---

## US-8.7 — Trust that the dependencies are not carrying known holes ✅

**As a** user running a tool that handles my SSH credentials
**I want** its dependency tree audited automatically
**So that** a published vulnerability does not sit unnoticed in a shipped release.

**Acceptance criteria**
- `cargo deny check` runs on every pull request and **fails the build** on a new advisory, a
  disallowed licence, or a dependency from an unapproved source
- **Git dependencies are banned.** One has no semver contract, and advisory scanners match
  crates.io name and version — so a git dependency is invisible to them. A clean report is
  only meaningful if every dependency is one the scanner can read
- An advisory with no available fix is recorded in `deny.toml` with a written reason and the
  condition under which to revisit it, never as a bare suppression
- Dependencies are refreshed weekly by Dependabot, lockfile-only, so patches arrive without
  semver churn
- The CI pipeline itself is pinned: Actions by commit SHA, the toolchain by
  `rust-toolchain.toml`
- No credential can reach the repository — `gitleaks` scans every pull request

**Implementation**: `deny.toml`, `.github/workflows/ci.yml`, `.github/dependabot.yml`,
`rust-toolchain.toml`, `make audit`
**Documentation**: [../architecture/SECURITY.md](../architecture/SECURITY.md)

> Delivered in v0.2.0, which took the tree from 25 known vulnerabilities to 1. The one that
> remains has no fixed version in any release and is documented as an accepted risk.
>
> The gate was deliberately advisory-only while that cleanup was in flight. A gate that is
> permanently red is one people learn to scroll past.

---

## US-8.8 — Not negotiate obsolete SSH cryptography ✅

**As a** user
**I want** the client to stop offering algorithms that are no longer considered sound

**Acceptance criteria**
- SHA-1 MACs are not offered
- Compression is not offered, which removes the compression "zip bomb" class structurally
  rather than by patch
- The insecure `des` and `dsa` backends stay disabled regardless of what upstream adds to its
  default feature set, because features are listed explicitly

**Implementation**: root `Cargo.toml` (`russh` with `default-features = false`),
`crates/daemon/src/tunnel.rs` (config built from `..Default::default()`)

> A server that *requires* compression or SHA-1 MACs will now fail to negotiate. Both are
> long deprecated: OpenSSH defaults to `Compression no` and dropped SHA-1 MACs from its own
> defaults years ago. This is the one behavioural change in v0.2.0 that could affect an
> existing connection, and it is called out in the release notes.

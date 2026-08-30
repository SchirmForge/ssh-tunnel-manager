# Epic 10 — Development and quality

Stories for contributors: being able to change the code without breaking it, or damaging
your own machine.

See [../DEVELOPMENT.md](../DEVELOPMENT.md#testing) for how to run any of this.

[← Back to index](README.md)

---

## US-10.1 — Run the tests without risking my own setup ✅

**As a** contributor
**I want** the test suite to leave my real configuration and running daemon alone

**Acceptance criteria**
- Tests redirect `XDG_CONFIG_HOME` and `XDG_RUNTIME_DIR` into a temporary tree, so profiles, config, token, `known_hosts`, socket and PID file are all sandboxed
- `~/.config/ssh-tunnel-manager` is never read or written by a test
- A daemon already running on the machine is unaffected
- The integration harness sets these on the spawned daemon's environment rather than the test process's, so tests still run in parallel

**Implementation**: `scripts/sandbox.sh`, `crates/daemon/tests/harness/mod.rs`, `PidFileGuard::create_at`

> Before v0.1.11 this was not true. The pidfile test called `PidFileGuard::create()`, which
> resolves the real runtime path: with a daemon running the test failed, and with none
> running it would create and then delete the live daemon's PID file.

---

## US-10.2 — Get a fast, dependency-free signal ✅

**As a** contributor
**I want** a test tier that always runs
**So that** every change is checked without network access or credentials.

**Acceptance criteria**
- `cargo test` needs no network, no SSH server and no secrets
- It spawns a real daemon per integration test and drives the actual REST/SSE API through the real client, rather than mocking
- Coverage: health and info per listener mode, token missing/wrong/correct, the loopback-only HTTP rule, socket and directory permissions, the single-instance guard, SSE connect, heartbeat and authentication
- The suite completes in well under a second

**Implementation**: `crates/daemon/tests/daemon_api.rs`

---

## US-10.3 — Test the authentication flows for real ✅

**As a** contributor
**I want** the auth paths exercised against a real SSH server
**So that** the most fragile part of the codebase is not protected by hope.

**Acceptance criteria**
- A live tier covers host key verification and refusal, key auth, encrypted-key passphrases, password auth, wrong-password re-prompting, 2FA, the keyboard-interactive retry path, real traffic through the forward, and teardown both connected and mid-authentication
- TOTP codes are generated in-test, so 2FA needs no human. The generator is verified against the RFC 6238 and RFC 2202 vectors, so it is trustworthy without the live server
- Tests drive `start_tunnel_with_events` — the same shared flow the CLI and GUI use — so a regression in the client path is caught too

**Implementation**: `crates/daemon/tests/live_ssh.rs`, `crates/daemon/tests/harness/live.rs`

---

## US-10.4 — Keep credentials out of the repository ✅

**As a** contributor
**I want** test credentials to be impossible to commit by accident

**Acceptance criteria**
- The test host name and every credential live only in `.local/testing/ssh-target.env`
- `/.local/` is gitignored in its entirety
- **No committed file** — script, test, document or CI workflow — contains the host name or any credential
- A template with placeholders is committed at `docs/testing/ssh-target.env.template`
- CI writes the file from a secret at runtime and deletes it afterwards

**Implementation**: `.gitignore`, `docs/testing/ssh-target.env.template`, `crates/daemon/tests/harness/live.rs`

---

## US-10.5 — Have a clone that works immediately ✅

**As a** new contributor
**I want** `cargo test` to pass on a fresh clone with no setup

**Acceptance criteria**
- Live tests are `#[ignore]`d, so they never run by accident
- With no target configured they **skip**, they do not fail
- The skip prints why, and points at the template

> **Trap worth knowing**: a skipped live test still reports `ok`. Run the live tier with
> `--nocapture` — as `make test-live` does — or a completely skipped run looks like a
> passing one.

**Implementation**: `crates/daemon/tests/harness/live.rs` (`live_target_or_skip!`), `Makefile`

---

## US-10.6 — Test a change in the real application quickly ✅

**As a** contributor working on the GUI
**I want** a disposable environment with a daemon and profiles ready
**So that** manual testing is a single command rather than a setup ritual.

**Acceptance criteria**
- `make sandbox` (or `scripts/dev-env.sh`) builds debug binaries, opens a sandbox, starts a daemon in a chosen listener mode, wires `cli.toml` to its generated token and seeds a profile per auth type
- `ssh-tunnel` and `ssh-tunnel-gtk` are on `PATH` and point at the sandbox
- Exiting the shell stops the daemon and deletes everything
- Without the target env file it still opens an empty sandbox and says what is missing

**Implementation**: `scripts/dev-env.sh`, `scripts/sandbox.sh`

---

## US-10.7 — Have every push checked ✅

**As a** maintainer
**I want** CI to catch what review misses

**Acceptance criteria**
- Every push and pull request runs `cargo fmt --check`, `cargo clippy -- -D warnings`, the hermetic tests and a release build
- The network-mode script runs too, covering the CLI against all three listener modes
- The live tier is `workflow_dispatch` only, so credentials never reach an untrusted pull request
- `gui-qt` is excluded from the default build, so its known compile failure cannot break CI

**Implementation**: `.github/workflows/ci.yml`, `default-members` in the root `Cargo.toml`

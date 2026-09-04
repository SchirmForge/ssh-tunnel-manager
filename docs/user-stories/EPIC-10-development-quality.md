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
- The test host name and every credential live only in a gitignored private target file
- The repository's private test-data directory is gitignored in its entirety
- **No committed file** — script, test, document or CI workflow — contains the host name or any credential
- A template with placeholders is committed at `docs/testing/ssh-target.env.template`
- The gating CI tier needs **no credentials at all**: it generates a throwaway localhost fixture per run
- The manual tier writes the file from a secret at runtime and deletes it afterwards
- `gitleaks` runs on every pull request, so the rule is enforced rather than merely stated

**Implementation**: `.gitignore`, `docs/testing/ssh-target.env.template`, `crates/daemon/tests/harness/live.rs`, `scripts/ssh-fixture.sh`, `.github/workflows/ci.yml`

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

In CI that trap is closed rather than documented. `SSH_TUNNEL_TEST_STRICT` turns a skip into
a failure, at two levels because the two live tiers can support different amounts:

| Value | Meaning | Used by |
|---|---|---|
| unset | skip freely — a fresh clone runs `cargo test` with no setup | developers |
| `1` | the target file must exist; an account it does not configure still skips | tier 2, local fixture |
| `all` | the target file must exist **and** every account a test asks for must be configured | tier 4, provisioned host |

**Implementation**: `crates/daemon/tests/harness/live.rs` (`live_target_or_skip!`, `Strictness`), `Makefile`

---

## US-10.6 — Test a change in the real application quickly ✅

**As a** contributor working on the GUI
**I want** a disposable environment with a daemon and profiles ready
**So that** manual testing is a single command rather than a setup ritual.

**Acceptance criteria**
- `make sandbox` (or `scripts/dev-env.sh`) builds debug binaries, opens a sandbox, starts a daemon in a chosen listener mode, wires `cli.toml` to its generated token and seeds a profile per auth type
- `ssh-tunnel` and `ssh-tunnel-gui` are on `PATH` and point at the sandbox
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
- **Real SSH connections are exercised on every pull request**, against an unprivileged localhost sshd — no credentials, so this can run on untrusted pull requests safely
- The tier needing real credentials stays `workflow_dispatch` only
- The supply chain is checked: advisories, licences, dependency sources and unused dependencies
- Secrets are scanned for on every pull request
- Fedora 44 CI validates the production GUI and compiles the obsolete GUI on the common GTK
  binding generation; non-GUI checks remain portable to the main CI environment
- Actions are pinned to commit SHAs and the toolchain is pinned by `rust-toolchain.toml`; neither floats

**Implementation**: `.github/workflows/ci.yml`, `deny.toml`, `rust-toolchain.toml`, `.github/dependabot.yml`

The production GUI is a root-workspace default member. Fedora 44 CI runs its locked tests,
Clippy, and release build and explicitly compiles obsolete `gui-gtk`.

---

## US-10.8 — Run the live SSH tests with no server and no credentials ✅

**As a** contributor
**I want** the authentication tests to run on a fresh clone
**So that** the most fragile part of the codebase is checked on every change, not on demand.

**Acceptance criteria**
- `make test-live-fixture` starts an unprivileged `sshd` on localhost, runs the live tier
  against it, and tears it down
- No root, no container, no credentials, nothing to install beyond the `sshd` binary
- Covers the six key-based tests, **including both host key verification tests**
- Every key and secret is generated per run and thrown away
- It runs in CI on every pull request, so real SSH connections are exercised on untrusted
  pull requests safely
- The four PAM-dependent tests (password, 2FA) skip here and are covered by US-10.10

**Implementation**: `scripts/ssh-fixture.sh`, `make test-live-fixture`,
`.github/workflows/ci.yml`

> `sshd` runs perfectly well as an ordinary user on a high port with its own config and host
> key. That single fact is what moves the SSH client path from "tested when someone
> remembers" to "tested on every change". Password and keyboard-interactive authentication
> go through PAM, which must read the shadow database, so those genuinely need privilege.

---

## US-10.9 — Know the dependencies are safe without checking by hand ✅

**As a** maintainer
**I want** the supply chain audited automatically

**Acceptance criteria**
- `make audit` runs the same checks CI does
- A new advisory, a disallowed licence or a dependency from an unapproved source **fails the
  build**
- Git dependencies are rejected, because advisory scanners cannot see them
- Accepted risks live in `deny.toml` with a reason and a revisit condition
- One tool and one policy file: `cargo audit` was dropped from the gate because it does not
  read `deny.toml`, and two ignore lists would drift apart

**Implementation**: `deny.toml`, `make audit`, `.github/workflows/ci.yml`
**See also**: US-8.7, and [../architecture/SECURITY.md](../architecture/SECURITY.md)

---

## US-10.10 — Set up a real test target in one command ✅

**As a** contributor with a spare machine or VM
**I want** the full live tier, including password and 2FA, without hand-configuring sshd

**Acceptance criteria**
- `scripts/provision-test-target.sh --host <host>` creates the three test accounts, generates
  every key and secret, configures `sshd` and PAM, and writes the private target configuration
- It is idempotent, takes the host as an argument and hardcodes nothing
- Every `sshd_config` change is scoped with `Match User` to the test accounts, so the
  administrative account's authentication is never altered
- The TOTP module is applied to the 2FA account only, via `pam_succeed_if`
- The configuration is validated with `sshd -t` **before** anything is reloaded, and the
  original PAM stack is restored if validation fails
- Administrative access is re-verified afterwards

**Implementation**: `scripts/provision-test-target.sh`

> The script edits `sshd` on a machine reachable only over SSH, so every one of those
> safeguards exists to make locking yourself out impossible rather than unlikely.

---

## US-10.11 — Catch a credential store changing underneath us ✅

**As a** maintainer
**I want** credential storage tested against a real store, not only a fake
**So that** a dependency upgrade cannot silently move where secrets are kept.

**Acceptance criteria**
- Unit tests cover the logic through an in-memory fake, with no keyring needed
- `make test-keychain-live` exercises a **real** Secret Service in a throwaway
  `dbus-run-session` with its own `gnome-keyring-daemon`, and runs in CI
- Nothing touches the developer's own keyring: the session, the keyring and every entry are
  created and discarded, with cleanup on panic
- One test writes a credential, then **re-execs the test binary** and asserts it can be read
  back from a separate process
- The migration path is covered: a credential seeded into the kernel keyring is found,
  adopted, and the original removed

**Implementation**: `crates/common/src/keychain.rs` (unit tests),
`crates/common/tests/keychain_live.rs`, `make test-keychain-live`, `.github/workflows/ci.yml`

> The cross-process test is the one that matters, and it is not obvious why. A write followed
> by a read passes even when the backend changes, because both go to the *new* store. What
> actually breaks is persistence across processes — the GUI writes, and the daemon reads later
> from a different process. Only a second process can tell the difference.
>
> This module had no tests at all before v0.3.0, which is how both the v0.2.0 store change and
> the remote-daemon defect in US-7.5 reached users.

---

## US-10.12 — Never be told a suite passed when it ran nothing ✅

**As a** contributor

**I want** a live suite with no target configured to fail rather than skip

**so that** a green run always means the code was actually exercised.

**Acceptance criteria**
- `make test-live` and `make test-live-fixture` both set `SSH_TUNNEL_TEST_STRICT`, so a missing
  or incomplete target configuration fails loudly
- `=1` requires the target file; `=all` additionally requires every account a test asks for
- A bare `cargo test -- --ignored` still skips, and says so on stderr, for the case where that
  is what you want

**Implementation**: `Makefile` (`test-live`, `test-live-fixture`), `crates/daemon/tests/harness`
(`live_target_or_skip!`)

> This was not hypothetical. `make test-live` ran zero tests and reported
> `test result: ok. 11 passed` in 0.00 seconds — indistinguishable from a passing run unless
> you noticed the timing. The tests had all skipped because their configuration had been
> destroyed by US-10.13.

---

## US-10.13 — Not lose a provisioned target by running the local fixture ✅

**As a** contributor with a provisioned test host

**I want** the local sshd fixture to leave my target configuration alone

**so that** running the quick tier does not silently destroy credentials that exist nowhere
else.

**Acceptance criteria**
- `ssh-fixture.sh up` moves an existing non-fixture private target configuration aside rather
  than overwriting it, and `down` restores it
- The fixture's own file is still removed on `down`, identified by its generated-by marker
- A full `up`/`down` cycle leaves a pre-existing target configuration byte-identical

**Implementation**: `scripts/ssh-fixture.sh`

> The fixture and the provisioned tiers share one configuration path. `up` overwrote it and
> `down` then removed it, so running the local fixture destroyed the provisioned host's
> generated passwords and TOTP secret — which exist only in that file. The failure was
> invisible: the next `make test-live` skipped everything and reported success.

---

## US-10.14 — Have a live test that fails when the feature is broken ✅

**As a** contributor

**I want** new live tests checked against the unfixed code

**so that** a test cannot pass for a reason unrelated to what it claims to cover.

**Acceptance criteria**
- A test for a fix is run against the code *without* the fix and observed to fail
- A test handler that blocks does so releasably: `on_auth_required` is synchronous, so parking
  it occupies a runtime worker, and `JoinHandle::abort()` cannot interrupt a blocking call
- Tests using a blocking handler run on a multi-threaded runtime, so the parked thread cannot
  starve the test body

**Implementation**: `crates/daemon/tests/live_ssh.rs` (`ParksUntilReleased`)

> Two concrete failures drove this. A stream-longevity test counted heartbeats within a window
> that fit *inside* the 30-second cut it was meant to detect, so it passed against broken code
> in 30.25 seconds. And a never-answering handler used `thread::sleep(300)`, which the runtime
> drop then waited out: the tier-2 suite took 300 seconds instead of 20, and whether the test
> worked at all depended on whether the prompt arrived before the body's next await point. The
> suite now runs in 5.3 seconds.

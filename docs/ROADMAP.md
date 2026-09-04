# Roadmap

**Last updated**: 2026-09-03 (v0.6.0)

What is planned, what is deliberately not, and the design decisions already taken for work
that has not started yet.

- For **what exists today**, see [PROJECT_STATUS.md](PROJECT_STATUS.md) and the
  [user stories](user-stories/).
- For **what shipped when**, see [CHANGELOG.md](CHANGELOG.md).
- For **known defects**, see [KNOWN_ISSUES.md](KNOWN_ISSUES.md).

---

## Now

### Stability and regression testing
**Status**: In progress — the current focus. Automated coverage, sandboxed test
environments and bug fixing, rather than new features.

Delivered in v0.1.11:
- Sandbox isolation via `XDG_CONFIG_HOME` / `XDG_RUNTIME_DIR` (`scripts/sandbox.sh`)
- Tier-1 integration tests against a real daemon (`crates/daemon/tests/daemon_api.rs`)
- Tier-2 live SSH tests covering every authentication flow (`crates/daemon/tests/live_ssh.rs`)
- `scripts/dev-env.sh` for one-command manual and GUI testing
- CI (`.github/workflows/ci.yml`); `cargo clippy -- -D warnings` clean
- Removed the orphaned `crates/tray`

Delivered in v0.2.0:
- Dependency tree taken from 25 known vulnerabilities to 1 (the remainder has no fix and is
  documented as accepted) — see [releases/v0.2.0.md](releases/v0.2.0.md)
- `cargo deny check` gates every pull request and **fails the build**; git dependencies banned
- A tier-2 live SSH fixture that needs no server, no root and no credentials, so the SSH
  client path is exercised on every pull request
- `scripts/provision-test-target.sh` for the privileged tier
- `SSH_TUNNEL_TEST_STRICT`, which closes the "a skipped live test still reports ok" trap
- Secret scanning, SHA-pinned Actions, a pinned toolchain, Dependabot
- A documented dev container (`containers/Containerfile.dev`)
- Three defects the live tier found: password auth never retrying, a host-key test that was
  asserting nothing, and axum 0.8's route syntax change

Delivered in v0.3.0:
- **Saved credentials work with a remote daemon** — the setting now records where a credential
  is, not merely that one exists. See [releases/v0.3.0.md](releases/v0.3.0.md)
- Credential store selected at runtime (Secret Service, or the kernel keyring on a headless
  host), configurable with `credential_store`, and reported at startup
- Automatic migration of credentials saved before v0.2.0
- Credential storage tested against a real Secret Service in CI — the module previously had
  no tests at all

Delivered in v0.4.0:

- Parallel second-generation GTK 4/libadwaita GUI source preview, covering the in-scope
  profile, authentication, daemon, empty, and offline screens
- Toolkit-neutral controller/runtime, snapshots, typed actions/effects, capability states,
  authentication queue, editor contracts, and future-tray-ready profile commands in
  `gui-core`
- Profile search, connected-only filtering, name/manual sort, pinning and ordering, persisted
  separately in versioned `ui.toml`
- Client-held credential editing and one-off automatic responses through the common
  Secret Service/keyutils facade
- Structured-code-only GUI control invariant, accessibility/adaptive source work, and locked
  Fedora 44 automated validation for both Cargo workspaces
- First-launch GUI v2 client setup with snippet import, manual/repair flows, and secure atomic
  `cli.toml` persistence before the daemon runtime starts

Delivered in v0.6.0:

- GUI v2 promoted to the production `ssh-tunnel-gui` package, binary, desktop entry, and
  default workspace target; `gui-gtk` marked obsolete and frozen
- Active-profile editing with an explicit post-save choice to keep the current tunnel or
  reconnect immediately with the saved settings
- Reusable, structured status-driven reconnect command/effect in `gui-core`
- Authentication prompt copy aligned with the semantic window background

Remaining:
- Unattended credentials for a daemon with no client attached; this requires an explicit
  security model for credential access without a connected client
- The obsolete `gui-gtk` source still records the legacy storage value. It is frozen,
  excluded from default builds and packaging, and is not a supported editing path.
- Fix whatever the live tier turns up once the test accounts exist on the target host
- Close the remaining test and audit backlog — most notably broader unit coverage for the
  tunnel and credential-store paths, plus an sshd version matrix for algorithm negotiation
- Decide whether `AUTH_RESPONSE_TIMEOUT` (60s) is the right value when nothing answers a
  credential prompt

### Packaging
**Status**: In progress

| Target | Status |
|---|---|
| DEB | ✅ Production package/binary name is `ssh-tunnel-gui`; publish artifacts with the next release |
| RPM | 🚧 In progress |
| AUR (PKGBUILD) | 🚧 Not started |
| Flatpak | 🚧 To be confirmed |

### Production GUI cutover

**Status**: Complete.

- `crates/gui-v2` is the root-workspace/default GUI package `ssh-tunnel-gui`.
- The production application ID is `io.github.schirmforge.SshTunnelManager`.
- CI validates it on Fedora 44; the Makefile, installer, development sandbox, desktop entry,
  and current documentation use the production binary.
- The user accepted profile, credential-store, daemon, SSE, and authentication behavior.
- `crates/gui-gtk` remains an obsolete, frozen workspace member on the same GTK binding
  generation. It is excluded from default builds and packaging; removal is not planned.

Keyboard-only review and screen comparison remain follow-up validation in
[KNOWN_ISSUES.md](KNOWN_ISSUES.md); they do not roll back the accepted production target.

## Deferred or excluded from v0.4.0

This table is the authoritative disposition for capabilities intentionally outside the GUI
v2 preview. “Deferred” means potentially future work; “not planned” is a design rule, not a
backlog item.

| Capability | v0.4.0 disposition | Future tracking / reason |
|---|---|---|
| Tray implementation and tray-library selection | **Deferred** | The shared snapshot and `AppCommand` action surface is implemented. A tray adapter and dependency will be selected only through a separate design review. |
| Stored TOTP generation or secret persistence | **Deferred** | Server keyboard-interactive and structured two-factor prompts work. Persisting a seed changes the security and unattended-credential model, so it requires a separate reviewed design. |
| Daemon API for starting/restarting the daemon | **Deferred** | Health/info/refresh are real. GUI lifecycle controls remain WIP until an authenticated, scoped daemon/service-management contract is designed. |
| SSH config import | **Deferred** | The GUI entry point is visibly WIP and performs no fake import. Parser, merge, conflict, and credential semantics need a separate feature plan. |
| New traffic, uptime, or last-connected telemetry | **Deferred** | GUI v2 renders fields the daemon already supplies, including daemon-wide uptime. No new per-tunnel traffic, last-connected, or additional uptime field/endpoint was added. |
| Parsing daemon prompt strings into structured security data | **Not planned** | Free-form daemon text is presentation-only and will never be a control plane. Future host/key/fingerprint facts require additive structured protocol fields. |
| Cross-distribution, Windows, or macOS GUI compatibility | **Out of scope** | The production GUI currently targets Bazzite/Fedora 44. Portability requires a separate compatibility plan. |

---

## Next

### Desktop notifications and auto-reconnect
**Status**: Planned. A detailed implementation plan is kept locally and is not published
with the repository. Note when picking it up that several of its original premises no
longer hold — it was written before the v0.1.11 stabilisation pass, which removed the tray
crate it assumed, and the daemon refactor it proposes has already been done. The
authoritative statement of the auto-reconnect design is
[the section below](#auto-reconnect-and-health-monitoring), not that plan.

Desktop notifications for connect, disconnect and error events, plus the auto-reconnect
design below. No notification dependency is currently selected; adding one requires review.

### Configurable daemon config path
**Status**: Planned
**Files**: `crates/daemon/src/main.rs`, `crates/daemon/src/config.rs`

Accept the daemon config file as a command-line parameter, defaulting to
`~/.config/ssh-tunnel-manager/daemon.toml`. Enables multi-instance daemons and system-wide
configurations. The daemon currently takes no arguments at all.

### Enhanced logging
**Status**: Planned — design decision needed

Daemon logging with a `--debug` option and configurable levels. Open questions:

- journalctl integration, dedicated log files, or both?
- Default level — Info, Debug or Trace?
- Rotation policy for file output?
- Structured (JSON) logging for machine parsing?

---

## Later

### Auto-reconnect and health monitoring
**Status**: Config exists, wiring needed. **Design decided, not implemented.**
**Files**: `crates/daemon/src/tunnel.rs`, `crates/daemon/src/monitor.rs`

`TunnelOptions.auto_reconnect`, `reconnect_attempts` and `reconnect_delay` are read from
profiles and surfaced in both front-ends, but nothing acts on them. `run_tunnel()` connects
once, and `monitor.rs` is an empty stub.

**Design decisions taken:**

1. **Global setting with a per-profile override.** Today `auto_reconnect` is per-profile
   only. A daemon-wide default belongs in `daemon.toml`, since the daemon is what would
   perform the reconnection, with the profile field becoming `Option<bool>`: `None`
   inherits the global, `Some(_)` overrides it. Existing profiles carrying
   `auto_reconnect = true` still deserialise unchanged, so this is backward compatible.

2. **Never auto-reconnect where authentication needs a human.** Reconnecting is only
   meaningful when the daemon can authenticate unattended:

   | Auth setup | Can reconnect unattended? |
   |---|---|
   | `Key` + unencrypted key (`PasswordStorage::None`) | ✅ yes |
   | `Key` + passphrase in keychain | ✅ yes |
   | `Password` + password in keychain | ✅ yes — the daemon does retrieve it |
   | `Password`, not stored | ❌ prompts |
   | `PasswordWith2FA` | ❌ **never**, stored password or not — a TOTP code is single-use |

   An unencrypted key cannot be detected without trying to load it, so treat
   "key + storage None" as eligible and let a failed load fall back to the no-reconnect
   path.

3. **The current default is wrong and must change with this work.**
   `default_auto_reconnect()` returns `true`, so every profile — including 2FA ones that
   can never reconnect unattended — is currently flagged for auto-reconnect. It is
   harmless only because nothing acts on the flag.

4. **Surface ineligibility in the UI** rather than silently ignoring the setting: where a
   profile's authentication makes unattended reconnection impossible, the CLI flag and the
   GUI's Advanced accordion should show the control as unavailable, with the reason.

> The v0.2.0 plan document's proposed `can_auto_reconnect_without_auth()` is wrong in both
> directions — it rejects unencrypted keys (the most common eligible case) and rejects
> `Password` + keychain (which the daemon does support). Use the table above instead.

### Dynamic / SOCKS proxy (`ssh -D`)
**Status**: Future, unscheduled

- Daemon: implement `run_dynamic_forward_task()` with SOCKS5 protocol handling
- CLI: support `--forwarding-type dynamic`
- GUI: add to a forwarding type dropdown

Note this is **not a daemon-only change**: `ForwardingType::Local` is hardcoded at profile
creation in both clients (`crates/cli/src/main.rs`,
`crates/gui-gtk/src/ui/profile_dialog.rs`).

### Daemon management GUI
**Status**: Information and health refresh implemented in GUI v2; lifecycle operations deferred
**Files**: `crates/gui-v2/src/daemon_view.rs`, `crates/gui-core/src/runtime.rs`

- Show daemon information read from the running daemon over the API, not from the file — ✅
- Start, stop and restart the daemon (user-scoped operations only) — 🚧 WIP; see the v0.4.0
  exclusions above
- Configure autostart via `systemctl`
- Configure which profiles start with the daemon

### Multiple daemon connections
**Status**: Partially planned

Allow more than one `cli.toml`. The default stays
`~/.config/ssh-tunnel-manager/cli.toml`, with an alternative selectable from the client
configuration page, and a preferences file recording the known locations. Only one daemon
is monitored at a time, but multiple GUI instances can run.

### System integration
**Status**: Partial

| Component | Status |
|---|---|
| systemd user and system service templates | ✅ Available |
| Desktop notifications | 🚧 Planned |
| System tray | 🚧 The old crate remains removed. GUI v2 supplies reusable snapshots and commands; adapter/library selection and rendering are deferred |
| Profile autostart | 🚧 The option exists in config but is not wired |

---

## Not planned

### Remote port forwarding (`ssh -R`)
**Dropped from the roadmap.** No use case has come up. Use `ssh -R` directly if you need it.

`ForwardingType::Remote` remains in the enum because it is serialised in existing profile
TOML and removing it would break those files, but no implementation is intended. The daemon
returns an explicit error for it.

### FreeBSD / OPNsense port
**Unscheduled.** A porting analysis is kept locally for reference, but nothing is
committed. It would cover the daemon and CLI only; the GTK GUI is out of scope, since it
depends on D-Bus and FreeDesktop portals. The three blockers identified were the
Linux-specific `__errno_location` in `pidfile.rs`, the Secret Service keyring dependency,
and the systemd unit files.

---

## Technical debt

| Item | Status |
|---|---|
| Outdated tests in `crates/common` | ✅ Resolved in v0.1.11 — the "schema drift" was a misdiagnosis. The fixtures matched the structs and every test passed once run; the real problems were isolation and missing coverage |
| `TunnelManager::stop` held a lock across an await | ✅ Fixed in v0.1.11 |
| Token handoff logs secrets | ❌ Open — clarify so the CLI can consume the token without it appearing in output |
| Token stored in plaintext in `cli-config.snippet` | ❌ Open — 0600, but present at rest. Encrypted storage under consideration |
| `crates/daemon/src/monitor.rs` is an empty stub | ❌ Open — see auto-reconnect above |
| Dynamic/SOCKS forwarding returns an error | 🚧 By design until implemented |
| `russh` pinned to a git branch | ✅ Resolved — moved to crates.io 0.63.1 and git dependencies are now banned by `cargo deny`. A git dependency has no semver contract and is invisible to advisory scanners, so the ban is what makes a clean audit report meaningful |
| Known vulnerabilities in the dependency tree | ✅ Resolved — 25 down to 1, and that one (`RUSTSEC-2023-0071`, `rsa`) has no fix in any release and is documented in `deny.toml` and [architecture/SECURITY.md](architecture/SECURITY.md) |
| `users` crate unmaintained since 2020 | ✅ Resolved — replaced with the `uzers` fork |
| Live SSH tests only runnable by hand, with credentials | ✅ Resolved — an unprivileged localhost sshd covers the key-based half on every pull request with no secrets; the full matrix runs against a provisioned host |
| RSA timing side channel (`RUSTSEC-2023-0071`) | ⚠️ Accepted — no fixed version exists in any `rsa` release; dropping RSA support would break users' keys. See [architecture/SECURITY.md](architecture/SECURITY.md) |
| `rustls-pemfile` unmaintained | 🚧 Open — functional, no known vulnerability; migrate to `rustls-pki-types` when convenient |
| `gui-core` / `gui-gtk` declare unused dependencies | ⚠️ `gui-core` resolved in v0.4.0; obsolete `gui-gtk` is frozen and its remaining report is accepted unless it prevents compilation |
| Two client architectures for the same event stream | ✅ Resolved in v0.5.0 — the CLI hand-rolled its own SSE subscription while the GUI used `EventListener`, so a transport fix on one path missed the other. Both now share `EventListener`, and `gui-core::events` (a trait with no implementors) was removed |
| Runtime paths derived in three places | ✅ Resolved in v0.5.0 — the socket, the PID file and the client's probe list disagreed under the project's own systemd unit. One module in `common` now owns them |
| Live suites could report success having run nothing | ✅ Resolved in v0.5.0 — `make test-live` and `make test-live-fixture` run strict, and the local fixture no longer destroys a provisioned target's configuration |
| Manual GUI validation of event delivery | ✅ Accepted — the production GUI stayed attached beyond the former timeout, reconciled the live daemon, and operated alongside the CLI without regressing SSE/authentication behavior |

---

## Release history

Completed work is recorded in [CHANGELOG.md](CHANGELOG.md) and the per-release notes in
[releases/](releases/). This document tracks only what is ahead.

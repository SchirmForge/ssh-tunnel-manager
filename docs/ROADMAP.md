# Roadmap

**Last updated**: 2026-08-30 (v0.1.11)

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

Remaining:
- Fix whatever the live tier turns up once the test accounts exist on the target host
- Decide whether `AUTH_RESPONSE_TIMEOUT` (60s) is the right value when nothing answers a
  credential prompt
- Get `crates/gui-qt` compiling, or retire it — it is currently excluded from
  `default-members` so it cannot break the build

### Packaging
**Status**: In progress

| Target | Status |
|---|---|
| DEB | ✅ Done — daemon, CLI, GTK GUI |
| RPM | 🚧 In progress |
| AUR (PKGBUILD) | 🚧 Not started |
| Flatpak | 🚧 To be confirmed |

---

## Next

### Desktop notifications and auto-reconnect (v0.2.0)
**Status**: Planned. A detailed implementation plan is kept locally and is not published
with the repository. Note when picking it up that several of its original premises no
longer hold — it was written before the v0.1.11 stabilisation pass, which removed the tray
crate it assumed, and the daemon refactor it proposes has already been done. The
authoritative statement of the auto-reconnect design is
[the section below](#auto-reconnect-and-health-monitoring), not that plan.

Desktop notifications for connect, disconnect and error events, plus the auto-reconnect
design below. `notify-rust` is already a declared dependency.

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
**Status**: Partially planned
**Files**: `crates/gui-gtk/src/ui/daemon_page.rs` (new)

- Show daemon configuration read from the running daemon over the API, not from the file
- Start, stop and restart the daemon (user-scoped operations only)
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
| Desktop notifications | 🚧 Planned for v0.2.0 |
| System tray | 🚧 The old `crates/tray` was removed in v0.1.11 — it had been outside the workspace build since v0.1.6. Any tray support will be written fresh against the current architecture |
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
| `crates/gui-qt` does not compile | 🚧 Open — cxx-qt bridge macro; excluded from `default-members` so it cannot break the build or CI |
| Dynamic/SOCKS forwarding returns an error | 🚧 By design until implemented |

---

## Release history

Completed work is recorded in [CHANGELOG.md](CHANGELOG.md) and the per-release notes in
[releases/](releases/). This document tracks only what is ahead.

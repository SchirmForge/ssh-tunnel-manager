# Known Issues & Limitations

This document tracks known bugs, limitations, and missing features in SSH Tunnel Manager v0.4.0.

For missing features described as user stories with their status, see the
[user stories](user-stories/). For what is planned about them, see [ROADMAP.md](ROADMAP.md).

## Known Bugs

### 🐛 Active Issues

- **Token exposure in CLI config snippet**
  - `cli-config.snippet` contains plaintext auth token
  - File has 0600 permissions but still visible in filesystem
  - **Impact**: Low - file is protected, same security as SSH keys
  - **Workaround**: Delete snippet after copying: `rm ~/.config/ssh-tunnel-manager/cli-config.snippet`
  - **Status**: Considering encrypted storage in a future release

#### Edge Cases
- **60-second wait when no client answers an authentication prompt**
  - Occurs when no GUI/CLI client is connected during authentication
  - The daemon waits out `AUTH_RESPONSE_TIMEOUT` for a response that never comes
  - **Impact**: The tunnel stops eventually, but takes 60 seconds
  - **Workaround**: Use the GUI or CLI to cancel tunnels properly
  - **Status**: Partly addressed. `TunnelManager::stop` used to hold the `tunnels` write
    lock across the await on the tunnel task, which the task itself needed to finish, so
    every cancellation during auth force-aborted after a timeout instead of stopping
    cleanly. That is fixed. The remaining 60 seconds is `AUTH_RESPONSE_TIMEOUT` elapsing
    when *nothing* cancels, which is the timeout working as designed; whether 60s is the
    right value is open. Regression test:
    `stopping_during_authentication_returns_promptly` in `crates/daemon/tests/live_ssh.rs`

#### Event delivery
- ~~**Authentication prompts were lost whenever the event stream reconnected**~~ - **Resolved
  in v0.5.0.** Starting a password-authenticated tunnel could fail with `Authentication prompt
  timed out after 60s` with a client attached and waiting. The stream carried a *total* request
  timeout, which in `reqwest` covers the response body, so it was cut at exactly 30 seconds
  regardless of traffic; the reconnect backoff then doubled without ever resetting. The client
  settled into a permanent 30s-connected / 30s-disconnected cycle, and a prompt raised in a
  dead window was never re-sent. The stream now uses a read timeout, the backoff resets after a
  connection that lasted, and the daemon re-sends outstanding prompts to every new subscriber.
  See US-6.3 and US-6.6.
  - **Manual GUI validation outstanding**: the fix is covered by unit tests and by live tests
    against a real SSH server, but three desktop checks still need a human — reproducing the
    original report by hand (wait ~40s at a host key prompt, then answer, and confirm the
    password prompt appears), confirming an idle GUI holds one long-lived `GET /api/events`
    rather than a new one every 30 seconds, and running the CLI and GUI together without
    either double-prompting. This is called out rather than assumed because US-6.3 was marked
    done in v0.4.0 while the stream was in fact broken.

- ~~**A daemon running as root could not be reached by any client**~~ - **Resolved in v0.5.0.**
  The socket was placed in `$XDG_RUNTIME_DIR`, which for root is `/run/user/0` — mode 0700 by
  design — so clients running as a normal user reported that no daemon was running. The daemon
  now refuses to start as root and explains how to get privileged ports instead. See US-5.10.

#### Credentials
- ~~**"Store in keychain" silently did nothing with a remote daemon**~~ - **Resolved in
  v0.3.0.** `password_storage = "keychain"` recorded only that a credential was in *a*
  keychain, never whose. The client saved it locally and the daemon looked on its own host;
  with the daemon on another machine those are different stores, so nothing was found and the
  user was prompted anyway — no error, no warning. The setting now records **where** the
  credential is, and a client-held one is answered by the client. See US-7.5.
  - **Still open in the packaged GUI**: `gui-gtk` records the legacy value when saving. It
    works against a local daemon and is read correctly everywhere. GUI v2 uses explicit
    client/daemon-host storage and the common Secret Service/keyutils facade, but remains a
    source preview until cutover.

- ~~**Credentials saved before v0.2.0 appeared to be lost**~~ - **Resolved in v0.3.0.** The
  `keyring` 3 → 4 upgrade moved the backing store from the kernel keyutils keyring to Secret
  Service, so earlier credentials became invisible. They are now found, migrated and the
  original removed. The daemon also reports which store it is using, so the next such change
  is visible rather than silent.

#### Testing
- ~~**Outdated tests in `crates/common`**~~ - **Resolved.** The claim was inaccurate: the
  fixtures matched the current structs and all tests passed once run. What they actually
  lacked was isolation (two tests read the developer's real `~/.config`, and the pidfile
  test could delete a running daemon's PID file) and coverage. Both are fixed, and
  `cargo test` and `cargo clippy -- -D warnings` are now clean.

## Limitations & Missing Features

### ❌ Tunnel Types

#### Not Yet Implemented

**Remote Port Forwarding** (`ssh -R`)
- **Status**: **Not planned.** Dropped from the roadmap; no use case has come up
- **Current**: Only local port forwarding (`ssh -L`) is available
- **Workaround**: Use OpenSSH command-line directly for remote forwarding

**Dynamic/SOCKS Proxy** (`ssh -D`)
- **Status**: Future item, unscheduled
- **Current**: Only local port forwarding (`ssh -L`) is available
- **Workaround**: Use OpenSSH command-line directly for SOCKS proxy

**Auto-reconnect/Health Monitoring**
- **Status**: Partially implemented
- **Current**: Config options exist but not wired to actual reconnection logic
- **Impact**: Manual restart required when connections drop
- **Workaround**: Monitor tunnel status and restart manually:
  ```bash
  ssh-tunnel status myprofile
  ssh-tunnel restart myprofile
  ```

### ❌ GUI Features

**GUI v2 preview is not packaged or the default**
- **Status**: Source implementation and automated validation complete
- **Current**: The executable is `ssh-tunnel-gui-v2` in an isolated nested Cargo workspace;
  installed packages and desktop entries still launch `ssh-tunnel-gtk`
- **Remaining**: Visual comparison, keyboard/focus, Orca, high contrast/font scaling,
  narrow/localized layouts, live-daemon authentication, Secret Service, Bazzite runtime,
  production naming, CI/workspace integration, and packaging acceptance
- **Workaround**: Continue using the packaged GUI, or build the preview from source using the
  Development Guide

**System Tray Integration**
- **Status**: Adapter and library selection deferred; reusable core commands are implemented
- **Impact**: Cannot minimize GUI to system tray
- **Workaround**: Close GUI - daemon continues running in background

**Desktop Notifications**
- **Status**: Not implemented
- **Impact**: No notifications for tunnel status changes
- **Workaround**: Check status manually or via CLI

**Profile Autostart**
- **Status**: Not implemented
- **Impact**: Cannot configure profiles to start on daemon startup
- **Workaround**: Use systemd `ExecStartPost` to start tunnels:
  ```ini
  [Service]
  ExecStart=/usr/bin/ssh-tunnel-daemon
  ExecStartPost=/usr/bin/ssh-tunnel start myprofile
  ```

**GUI v2 WIP controls**
- Daemon start/restart/shutdown, SSH config import, stored TOTP, unsupported forwarding
  execution, and unavailable telemetry are labelled WIP and cannot report success
- Existing daemon-wide uptime is shown; no new traffic or last-connected telemetry exists
- See the authoritative [v0.4.0 scope table](ROADMAP.md#deferred-or-excluded-from-v040)

### ❌ CLI Features

**Log Rotation**
- **Status**: Not implemented
- **Impact**: Daemon logs may grow indefinitely
- **Workaround**: Use systemd journal (automatic rotation) or manual cleanup:
  ```bash
  journalctl --user -u ssh-tunnel-daemon --vacuum-time=7d
  ```

**Configurable Daemon Config Path**
- **Status**: Hardcoded to `~/.config/ssh-tunnel-manager/daemon.toml`
- **Impact**: Cannot use custom config location
- **Workaround**: Symlink custom location to expected path

**Structured Logging (JSON)**
- **Status**: Not implemented
- **Impact**: Logs are human-readable only, not machine-parseable
- **Workaround**: Parse text logs with tools like `awk` or `grep`

### ❌ Authentication

**Two-factor codes cannot be stored**
- **Status**: Inherent - a TOTP code is single-use by design
- **Current**: SSH key passphrases *and* passwords can both be stored in the keyring; the
  daemon retrieves them automatically (`crates/daemon/src/tunnel.rs`). Only the
  keyboard-interactive second factor must be entered each time
- **Impact**: `PasswordWith2FA` profiles always require a human
- **Workaround**: Use SSH keys where unattended connection matters:
  ```bash
  ssh-keygen -t ed25519
  ssh-copy-id user@remote-host
  ```

### ❌ System Integration

**The daemon refuses to run as root, with no override**
- **Status**: Deliberate, since v0.5.0
- **Impact**: A container image that runs the daemon as root will fail to start. Nothing in
  this repository ships such an image
- **Workaround**: Run as an unprivileged user and grant `CAP_NET_BIND_SERVICE`
  (`sudo setcap cap_net_bind_service=+ep /path/to/ssh-tunnel-daemon`), or use the shipped
  systemd unit, which already sets it
- See [architecture/SECURITY.md](architecture/SECURITY.md) for why

**No Autostart Enabled by Default**
- **Status**: systemd services exist but manual setup required
- **Impact**: Daemon doesn't start automatically
- **Workaround**: Enable systemd service based on your needs:
  ```bash
  # User service (ports 1024+, starts on login)
  systemctl --user enable ssh-tunnel-daemon

  # System service (grants CAP_NET_BIND_SERVICE for ports <1024, starts at boot)
  # Runs as the named user, never as root -- the daemon refuses to start as root.
  sudo systemctl enable ssh-tunnel-daemon@$USER
  # Or for specific user: sudo systemctl enable ssh-tunnel-daemon@username
  ```
- See [Installation Guide](INSTALLATION.md) for details on choosing between user and system service

## Documentation Gaps

### Missing or Incomplete Docs

**Network Modes Troubleshooting**
- **Status**: Limited documentation
- **Impact**: Harder to debug HTTPS mode issues
- **Workaround**: Check daemon logs and ensure firewall allows connections

**Architecture Diagrams**
- **Status**: ✅ Addressed in v0.5.0. `architecture/TECHNICAL_ARCHITECTURE.html` now carries an
  authentication event-flow diagram covering subscribe → start → prompt → reconnect → replay,
  alongside the existing crate dependency graph
- **Remaining**: no diagram for the tunnel data path or the TLS handshake

## Platform-Specific Issues

### Ubuntu 20.04 LTS
- **Issue**: GTK4 version too old (< 4.10)
- **Solution**: Upgrade to Ubuntu 22.04 or newer

### Debian 11 (Bullseye)
- **Issue**: May need backports for GTK4/libadwaita
- **Solution**: Enable backports or upgrade to Debian 12

### Headless Servers
- **Issue**: Secret Service is usually unavailable without a graphical session
- **Current behavior**: `credential_store = "auto"` falls back to the kernel keyutils
  keyring, which needs no D-Bus session but loses its contents on reboot
- **Alternatives**: Force `keyutils` or `none` in `daemon.toml`; use
  `SSH_TUNNEL_SKIP_KEYRING=1` to disable storage
- See [SYSTEMD.md](architecture/SYSTEMD.md) and the "Server and Headless Environments" section of the
  [README](../README.md#server-and-headless-environments) for detailed guidance

### SELinux/AppArmor
- **Issue**: No official policies
- **Impact**: May require permissive mode or custom policies
- **Workaround**:
  ```bash
  # Temporary SELinux permissive mode
  sudo setenforce 0
  ```

## Performance Issues

### None Currently Known

The application performs well within tested limits (50 concurrent tunnels, 100+ profiles).

## Reporting New Issues

Found a bug not listed here?

**Before reporting:**
1. Check [GitHub Issues](https://github.com/SchirmForge/ssh-tunnel-manager/issues)
2. Update to latest version
3. Check daemon logs: `journalctl --user -u ssh-tunnel-daemon -f`

**When reporting, include:**
- SSH Tunnel Manager version: `ssh-tunnel --version`
- Operating system and version
- Steps to reproduce
- Expected vs actual behavior
- Relevant logs

**Report at:** https://github.com/SchirmForge/ssh-tunnel-manager/issues

## Workarounds Summary

Quick reference for common issues:

| Issue | Quick Fix |
|-------|-----------|
| Daemon won't start | `rm /run/user/$(id -u)/ssh-tunnel-manager/daemon.pid` |
| GUI disconnected | `systemctl --user restart ssh-tunnel-daemon` |
| Auth failures | Check token in `~/.config/ssh-tunnel-manager/cli.toml` |
| Keyring unavailable | `export SSH_TUNNEL_SKIP_KEYRING=1` |
| Need remote forwarding | Use `ssh -R` directly (not planned here) |
| Need SOCKS proxy | Use `ssh -D` directly for now |

See the [Troubleshooting section of the Installation Guide](INSTALLATION.md#troubleshooting)
for detailed troubleshooting steps.

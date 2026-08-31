# Known Issues & Limitations

This document tracks known bugs, limitations, and missing features in SSH Tunnel Manager v0.1.11.

For missing features described as user stories with their status, see the
[user stories](user-stories/). For what is planned about them, see [ROADMAP.md](ROADMAP.md).

## Known Bugs

### 🐛 Active Issues

- **Token exposure in CLI config snippet**
  - `cli-config.snippet` contains plaintext auth token
  - File has 0600 permissions but still visible in filesystem
  - **Impact**: Low - file is protected, same security as SSH keys
  - **Workaround**: Delete snippet after copying: `rm ~/.config/ssh-tunnel-manager/cli-config.snippet`
  - **Status**: Considering encrypted storage for v0.2.0

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

**System Tray Integration**
- **Status**: Not implemented
- **Impact**: Cannot minimize GUI to system tray
- **Workaround**: Close GUI - daemon continues running in background

**Desktop Notifications**
- **Status**: Dependency present but not wired
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

**No Autostart Enabled by Default**
- **Status**: systemd services exist but manual setup required
- **Impact**: Daemon doesn't start automatically
- **Workaround**: Enable systemd service based on your needs:
  ```bash
  # User service (ports 1024+, starts on login)
  systemctl --user enable ssh-tunnel-daemon

  # System service (all ports including <1024, starts at boot)
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
- **Status**: No visual diagrams for SSE event flow
- **Impact**: Harder for contributors to understand system architecture
- **Planned**: v0.2.0 documentation update

## Platform-Specific Issues

### Ubuntu 20.04 LTS
- **Issue**: GTK4 version too old (< 4.10)
- **Solution**: Upgrade to Ubuntu 22.04 or newer

### Debian 11 (Bullseye)
- **Issue**: May need backports for GTK4/libadwaita
- **Solution**: Enable backports or upgrade to Debian 12

### Headless Servers
- **Issue**: Keyring unavailable without graphical session
- **Solution**: Set `SSH_TUNNEL_SKIP_KEYRING=1` environment variable
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

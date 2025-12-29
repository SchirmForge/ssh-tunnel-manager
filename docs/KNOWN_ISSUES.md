# Known Issues & Limitations

This document tracks known bugs, limitations, and missing features in SSH Tunnel Manager v0.1.8.

## Known Bugs

### 🐛 Active Issues

- **Token exposure in CLI config snippet**
  - `cli-config.snippet` contains plaintext auth token
  - File has 0600 permissions but still visible in filesystem
  - **Impact**: Low - file is protected, same security as SSH keys
  - **Workaround**: Delete snippet after copying: `rm ~/.config/ssh-tunnel-manager/cli-config.snippet`
  - **Status**: Considering encrypted storage for v0.2.0

#### Edge Cases
- **60-second timeout when canceling tunnel during auth phase**
  - Occurs when no GUI/CLI client is connected during authentication
  - Daemon waits for auth response that never comes
  - **Impact**: Tunnel stops eventually but takes 60 seconds
  - **Workaround**: Use GUI or CLI to cancel tunnels properly
  - **Status**: Investigating async cancellation improvements

#### Testing
- **Outdated tests in `crates/common`**
  - Profile manager tests need updating for current ProfileMetadata structure
  - Daemon client tests need updating for new field structure
  - All other crates have passing tests
  - **Impact**: Development only - does not affect production usage
  - **Workaround**: None needed for users
  - **Status**: Planned for v0.2.0

### 🔧 Minor Issues

None listed yet

## Limitations & Missing Features

### ❌ Tunnel Types

#### Not Yet Implemented

**Remote Port Forwarding** (`ssh -R`)
- **Status**: Planned for v0.2.0
- **Current**: Only local port forwarding (`ssh -L`) is available
- **Workaround**: Use OpenSSH command-line directly for remote forwarding

**Dynamic/SOCKS Proxy** (`ssh -D`)
- **Status**: Planned for v0.2.0
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

**Remote Host Password Storage**
- **Status**: Planned for future release
- **Current**: Only SSH key passphrases can be stored in keyring
- **Impact**: Keyboard-interactive and password auth require manual entry each time
- **Workaround**: Use SSH keys instead of passwords:
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
- See [Headless Setup](headless-setup.md) for detailed guide

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
1. Check [GitHub Issues](https://github.com/yourusername/ssh-tunnel-manager/issues)
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
| Need remote forwarding | Use `ssh -R` directly until v0.2.0 |
| Need SOCKS proxy | Use `ssh -D` directly until v0.2.0 |

See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for detailed troubleshooting steps.

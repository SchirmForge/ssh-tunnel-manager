# SSH Tunnel Manager - Security Documentation

**Version**: v0.1.9
**Last Updated**: 2025-12-31

## Overview

SSH Tunnel Manager is designed with security as a first-class concern. This document details the security measures, best practices, and considerations for operating the application safely.

## Security Principles

1. **Defense in Depth**: Multiple layers of security controls
2. **Least Privilege**: Minimal permissions required for operation
3. **Secure by Default**: Authentication and encryption enabled out of the box
4. **Fail Secure**: Errors result in denial rather than exposure
5. **No Secret Transmission**: SSH private keys never sent over network

## Authentication & Network Security

### Daemon Authentication

**Token-Based Authentication (Default)**
- **Enabled by Default**: `require_auth = true` in all daemon modes
- **Token Generation**: 32-byte cryptographically random token generated on first startup
- **Token Storage**: Stored in `~/.config/ssh-tunnel-manager/auth-token` with 0600 permissions
- **Token Transport**: Sent via `X-Tunnel-Token` HTTP header
- **Token Lifecycle**: Persists across daemon restarts unless manually regenerated

**Authentication Bypass**
- Only possible by explicitly setting `require_auth = false` in daemon config
- Daemon logs warning when authentication is disabled
- Not recommended for any production use

### Network Transport Security

**Unix Socket (Default Mode)**
- **Location**: `$XDG_RUNTIME_DIR/ssh-tunnel-manager/ssh-tunnel-manager.sock`
- **Permissions**: 0600 (owner read/write only) in user mode
- **Security**: Local-only, no network exposure, OS-level access control
- **Use Case**: Single-user desktop systems, default installation

**TCP HTTP Mode (Testing Only)**
- **Bind Address Restriction**: Only accepts loopback addresses (127.0.0.1, ::1)
- **Validation**: Daemon refuses to start if HTTP mode configured with non-loopback address
- **No Encryption**: Traffic not encrypted (acceptable for localhost only)
- **Use Case**: Local testing, debugging with tools like curl
- **Warning**: Daemon logs prominent warning when HTTP mode is active

**TCP HTTPS Mode (Network Access)**
- **TLS Required**: Self-signed certificate generated automatically
- **Certificate Location**: `~/.config/ssh-tunnel-manager/daemon-cert.pem`
- **Private Key Location**: `~/.config/ssh-tunnel-manager/daemon-key.pem`
- **Key Permissions**: Both files created with 0600 permissions
- **Certificate Expiry**: 365 days, automatically regenerated if expired
- **Fingerprint Pinning**: SHA256 fingerprint of certificate written to CLI config snippet
- **Client Validation**: CLI/GUI verify certificate fingerprint on first connection
- **MITM Protection**: Fingerprint pinning prevents man-in-the-middle attacks
- **Use Case**: Remote daemon access, headless servers, network-based management

### SSH Host Key Verification

**Known Hosts Management**
- **File Location**: `~/.config/ssh-tunnel-manager/known_hosts`
- **Format**: OpenSSH-compatible known_hosts format
- **Fingerprint Algorithm**: SHA256 (same as OpenSSH default)
- **First Connection**: User prompted to verify host key fingerprint
- **Subsequent Connections**: Automatic verification against stored fingerprint
- **Key Mismatch**: Connection refused if fingerprint changes (prevents MITM)
- **Manual Override**: User can edit known_hosts file directly if needed

## File & Directory Permissions

### Restrictive Umask

**Daemon Startup**
- Umask set to 0077 before creating any files
- Ensures all new files are owner-only by default (0600)
- Prevents accidental permission leaks

### File Permissions

**Sensitive Files (0600 - Owner Read/Write Only)**
- `~/.config/ssh-tunnel-manager/auth-token` - Daemon authentication token
- `~/.config/ssh-tunnel-manager/daemon-cert.pem` - TLS certificate
- `~/.config/ssh-tunnel-manager/daemon-key.pem` - TLS private key
- `~/.config/ssh-tunnel-manager/daemon.toml` - Daemon configuration
- `~/.config/ssh-tunnel-manager/cli.toml` - CLI configuration (contains auth token)
- `~/.config/ssh-tunnel-manager/known_hosts` - SSH host keys

**Profile Files (0600)**
- `~/.config/ssh-tunnel-manager/profiles/*.toml` - Individual profile configurations
- Contains SSH usernames, hostnames, port numbers
- Does NOT contain passwords or SSH private keys

### Directory Permissions

**User Mode (Default) - 0700**
- `~/.config/ssh-tunnel-manager/` - Main configuration directory
- `~/.config/ssh-tunnel-manager/profiles/` - Profile storage directory
- `$XDG_RUNTIME_DIR/ssh-tunnel-manager/` - Unix socket directory
- Owner can read/write/execute, no access for group or others

**Group Mode (System Daemons) - 0770**
- Enabled with `group_access = true` in daemon config
- Directories: 0770 (owner + group read/write/execute)
- Files: 0660 (owner + group read/write)
- Unix socket: 0660 (owner + group read/write)
- Use case: Multiple users accessing shared system daemon
- Security: Requires proper group membership management

### Permission Enforcement

**Daemon Behavior**
- Validates permissions on startup
- Refuses to start if critical files have insecure permissions (e.g., world-readable token)
- Automatically fixes permissions on files it creates
- Logs warnings for files with unexpected permissions

## Credential Management

### Keychain Integration

**System Keychain Storage**
- **Linux**: Secret Service API (GNOME Keyring, KDE Wallet, etc.)
- **macOS**: Keychain (untested, should work)
- **Windows**: Credential Manager (untested, should work)
- **Service Name**: `ssh-tunnel-manager`
- **Username Format**: `{profile-uuid}` (e.g., `550e8400-e29b-41d4-a716-446655440000`)

**What Gets Stored**
- SSH key passphrases (if user chooses)
- SSH password authentication passwords (if user chooses)
- Keyboard-interactive authentication responses (NOT stored - prompt each time)

**Security Properties**
- Encrypted at rest by OS keychain
- Protected by user's login session
- Never written to disk in plaintext
- Never logged or included in error messages

**Graceful Fallback**
- Daemon detects keyring availability at runtime
- If unavailable (headless server, container), prompts interactively each time
- No hard dependency on keyring for functionality
- User can disable via `SSH_TUNNEL_SKIP_KEYRING=1` environment variable

### SSH Private Keys

**File-Based References**
- SSH private keys referenced by filesystem path only
- Keys stored in standard locations (e.g., `~/.ssh/id_ed25519`)
- Never copied, embedded, or transmitted over network
- Key files managed by user, not by SSH Tunnel Manager

**Remote Daemon Security (v0.1.9)**
- **Hybrid Profile Mode**: Profile data sent via API, SSH keys stay on daemon filesystem
- **No Key Transmission**: Private keys never sent over network, even with HTTPS
- **User Responsibility**: User must copy SSH keys to daemon host manually
- **Trust Model**: User controls where keys are stored and how they're accessed
- **Error Messages**: Show daemon's actual SSH directory path (e.g., `/home/daemon/.ssh/`)

**Recommended Practices**
- Use SSH agent for encrypted keys (prompts for passphrase once)
- Set correct permissions: `chmod 600 ~/.ssh/id_*`
- Test SSH connection before creating profile: `ssh -i ~/.ssh/id_rsa user@host`

### Password and Token Security

**No Credential Leaks**
- Credentials never passed via command-line arguments (visible in `ps`)
- Credentials never set in environment variables
- Credentials never logged (even in debug mode)
- Credentials zeroized in memory when no longer needed (via `zeroize` crate)

**Auth Token Security**
- Generated using `rand::thread_rng()` (cryptographically secure PRNG)
- 32 bytes = 256 bits of entropy
- Stored with 0600 permissions
- CLI config snippet includes token for initial setup
- User should secure CLI config file (`chmod 600 ~/.config/ssh-tunnel-manager/cli.toml`)

## Process Security

### User Process Model

**Non-Privileged Operation**
- Daemon runs as regular user by default
- No root or elevated privileges required (except for privileged ports)
- Systemd user service recommended installation method
- Uses user's home directory for configuration and profiles

### Privileged Port Binding

**Problem**: Ports ≤1024 require root privileges on Linux

**Solution 1: CAP_NET_BIND_SERVICE (Recommended)**
```bash
# Grant capability to daemon binary
sudo setcap 'cap_net_bind_service=+ep' /usr/local/bin/ssh-tunnel-daemon

# Systemd service (system-wide)
[Service]
User=tunneld
AmbientCapabilities=CAP_NET_BIND_SERVICE
```

**Solution 2: Root with Privilege Dropping**
- Start as root, bind port, drop privileges
- Not currently implemented
- Less secure than capability-based approach

**Solution 3: Port Forwarding**
- Use iptables/nftables to forward high port to low port
- Daemon binds to high port (e.g., 8080)
- Firewall forwards external traffic from port 80 → 8080

### Group Access Control

**Multi-User System Daemons**
- Enabled with `group_access = true` in daemon config
- Directories: 0770, Files: 0660, Socket: 0660
- Requires careful group membership management
- Security considerations:
  - All group members can read auth tokens
  - All group members can manage all tunnels
  - Only use with trusted users in the group
  - Alternative: Run separate daemon per user (recommended)

## Remote Daemon Security (v0.1.9)

### Best Practices

**Always Use HTTPS for Network Access**
- HTTP mode restricted to loopback only
- HTTPS enforced for non-localhost connections
- Self-signed certificate acceptable with fingerprint pinning
- No need for CA-signed certificate

**TLS Fingerprint Pinning**
- SHA256 fingerprint of daemon certificate written to CLI config snippet
- Client verifies fingerprint on first connection
- Prevents MITM attacks even with self-signed certificates
- User prompted if fingerprint changes (certificate regenerated)

**SSH Key Management for Remote Daemons**
1. Copy SSH keys to daemon host before creating profiles
2. Set proper permissions: `chmod 600 ~/.ssh/id_*` on daemon host
3. Use ssh-agent for encrypted keys (recommended)
4. Test SSH connection from daemon to target: `ssh -i ~/.ssh/id_rsa user@host`
5. Never send private keys over network, even encrypted

**Auth Token Security**
- Keep CLI config file secure: `chmod 600 ~/.config/ssh-tunnel-manager/cli.toml`
- Don't commit auth tokens to version control
- Regenerate token if compromised (delete auth-token file, restart daemon)
- Use separate tokens for different clients if needed

**Network Exposure**
- Bind daemon to specific interface if multi-homed (e.g., `bind_host = "192.168.1.10"`)
- Use firewall rules to restrict access to trusted IPs
- Consider VPN or SSH tunnel to daemon for additional layer of security
- Monitor daemon logs for unauthorized access attempts

## Threat Model

### In Scope

**Protected Against**
- Unauthorized local access (file permissions, socket permissions)
- Network eavesdropping (HTTPS/TLS encryption)
- MITM attacks (certificate fingerprint pinning, SSH host key verification)
- Credential theft from disk (keychain encryption, restricted file permissions)
- Credential leaks via process table or logs
- Multi-user conflicts (Unix socket permissions, group access control)

### Out of Scope

**Not Protected Against**
- Root user on same system (root can read all files)
- Physical access to unlocked system (keychain decrypted when logged in)
- Memory dumps while credentials in use (zeroize on drop, but window exists)
- Compromised SSH keys (user responsible for key security)
- Malicious code in dependencies (supply chain attacks - mitigated by Rust ecosystem audits)
- Advanced persistent threats (APTs) with kernel-level access

### Attack Scenarios

**Scenario 1: Unauthorized Local User Access**
- **Attack**: Non-root user tries to access another user's tunnels
- **Defense**: Unix socket permissions (0600), file permissions (0600)
- **Result**: Access denied by OS

**Scenario 2: Network MITM Attack (Remote Daemon)**
- **Attack**: Attacker intercepts traffic between CLI/GUI and remote daemon
- **Defense**: HTTPS/TLS encryption + certificate fingerprint pinning
- **Result**: Client refuses connection due to fingerprint mismatch

**Scenario 3: SSH Host Impersonation**
- **Attack**: Attacker redirects SSH connection to malicious host
- **Defense**: SSH host key verification via known_hosts
- **Result**: Connection refused due to key mismatch

**Scenario 4: Auth Token Theft**
- **Attack**: Attacker reads auth-token file or CLI config
- **Defense**: File permissions (0600), daemon validates token on each request
- **Result**: If attacker has file access, they can authenticate to daemon
- **Mitigation**: Regenerate token, investigate how file access was obtained

**Scenario 5: Credential Theft from Keychain**
- **Attack**: Attacker tries to read credentials from system keychain
- **Defense**: OS keychain encryption, requires user's login session
- **Result**: Credentials protected by OS, inaccessible without user login
- **Limitation**: If attacker has user session access, credentials accessible

## Security Auditing

### Logging

**What Gets Logged**
- Tunnel start/stop events
- Authentication failures (token mismatch, missing token)
- SSH connection failures (host unreachable, auth failed)
- Configuration errors
- TLS certificate generation/regeneration

**What Does NOT Get Logged**
- Passwords or passphrases
- Auth tokens (values)
- SSH private key contents
- Tunnel data contents

**Log Locations**
- Systemd journal: `journalctl --user -u ssh-tunnel-daemon`
- Standard output/error (if running manually)

### Monitoring Recommendations

**Daemon Logs**
```bash
# Watch for authentication failures
journalctl --user -u ssh-tunnel-daemon -f | grep -i "auth"

# Watch for new connections
journalctl --user -u ssh-tunnel-daemon -f | grep -i "connected"
```

**File Permission Checks**
```bash
# Check critical file permissions
ls -la ~/.config/ssh-tunnel-manager/auth-token
ls -la ~/.config/ssh-tunnel-manager/cli.toml
ls -la ~/.config/ssh-tunnel-manager/daemon-*.pem
```

**Active Connections**
```bash
# Check active tunnels
ssh-tunnel status --all

# Check daemon status
ssh-tunnel info
```

## Vulnerability Reporting

If you discover a security vulnerability in SSH Tunnel Manager, please report it privately:

1. **Do NOT** open a public GitHub issue
2. Contact the maintainer directly (see repository for contact info)
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)
4. Allow reasonable time for fix before public disclosure

## Security Updates

Security fixes are released as soon as possible after discovery. Users should:

- Subscribe to GitHub releases for notifications
- Regularly update to latest version
- Review CHANGELOG.md for security-related fixes

## References

- [OWASP Secure Coding Practices](https://owasp.org/www-project-secure-coding-practices-quick-reference-guide/)
- [CWE Top 25 Most Dangerous Software Weaknesses](https://cwe.mitre.org/top25/archive/2023/2023_top25_list.html)
- [Rust Security Working Group](https://www.rust-lang.org/governance/wgs/wg-security)
- [OpenSSH Security Advisories](https://www.openssh.com/security.html)

## Acknowledgments

Security considerations guided by industry best practices and informed by the Rust security community.

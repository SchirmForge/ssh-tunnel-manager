# Testing Plan - Network-Ready Architecture

This document outlines the testing strategy for the network-ready SSH Tunnel Manager architecture (Tasks 1-6).

## Overview

We need to test three main connection modes:
1. **UnixSocket** - Local-only (backward compatible)
2. **TcpHttp** - Localhost HTTP (testing only)
3. **TcpHttps** - Network HTTPS with TLS

## Test Scenarios

### 1. UnixSocket Mode (Backward Compatibility)

**Goal**: Verify existing functionality still works

**Setup**:
```bash
# Default config should use UnixSocket mode
rm -f ~/.config/ssh-tunnel-manager/daemon.toml
rm -f ~/.config/ssh-tunnel-manager/cli.toml
```

**Tests**:
- [ ] 1.1: Start daemon without config (should use defaults)
- [ ] 1.2: Verify daemon listens on Unix socket at `$XDG_RUNTIME_DIR/ssh-tunnel-manager/ssh-tunnel-manager.sock`
- [ ] 1.3: CLI can connect without any configuration
- [ ] 1.4: Add a tunnel profile with `ssh-tunnel add`
- [ ] 1.5: Start a tunnel with `ssh-tunnel start <name>`
- [ ] 1.6: Stop a tunnel with `ssh-tunnel stop <name>`
- [ ] 1.7: Watch events with `ssh-tunnel watch`
- [ ] 1.8: Authentication NOT required (backward compat)

**Expected Results**:
- Daemon starts successfully
- CLI connects via Unix socket
- All existing commands work as before
- No breaking changes

---

### 2. TcpHttp Mode (Testing/Development)

**Goal**: Verify HTTP mode works on localhost

**Setup**:
```bash
# Create daemon config for HTTP mode
cat > ~/.config/ssh-tunnel-manager/daemon.toml << EOF
listener_mode = "tcp-http"
bind_address = "127.0.0.1:3443"
require_auth = true
EOF

# Create CLI config for HTTP mode
cat > ~/.config/ssh-tunnel-manager/cli.toml << EOF
connection_mode = "http"
daemon_url = "http://127.0.0.1:3443"
auth_token = ""  # Will fill in after daemon start
EOF
```

**Tests**:
- [ ] 2.1: Start daemon in tcp-http mode
- [ ] 2.2: Verify daemon binds to 127.0.0.1:3443
- [ ] 2.3: Verify authentication token is generated and displayed
- [ ] 2.4: Copy token to CLI config
- [ ] 2.5: CLI can connect via HTTP
- [ ] 2.6: Test all CLI commands (add, start, stop, watch)
- [ ] 2.7: Test authentication - CLI without token should fail (401)
- [ ] 2.8: Test authentication - CLI with wrong token should fail (401)
- [ ] 2.9: Test authentication - CLI with correct token should work
- [ ] 2.10: Warning message displayed about no encryption

**Expected Results**:
- Daemon listens on TCP port
- Token authentication works correctly
- Warning about lack of encryption shown
- All CLI commands work over HTTP

**Commands to Test**:
```bash
# After copying token to cli.toml:
ssh-tunnel add test-profile
ssh-tunnel start test-profile
ssh-tunnel stop test-profile
ssh-tunnel watch
```

---

### 3. TcpHttps Mode (Network Production)

**Goal**: Verify HTTPS with TLS works, including certificate pinning

**Setup**:
```bash
# Create daemon config for HTTPS mode
cat > ~/.config/ssh-tunnel-manager/daemon.toml << EOF
listener_mode = "tcp-https"
bind_address = "0.0.0.0:3443"
require_auth = true
EOF

# CLI config will be created after getting cert fingerprint
```

**Phase A: HTTPS without Certificate Pinning**

**Tests**:
- [ ] 3.1: Start daemon in tcp-https mode
- [ ] 3.2: Verify TLS certificate is auto-generated
- [ ] 3.3: Note the certificate fingerprint displayed on daemon startup
- [ ] 3.4: Verify daemon binds to 0.0.0.0:3443
- [ ] 3.5: Note the authentication token
- [ ] 3.6: Create CLI config without fingerprint:
```bash
cat > ~/.config/ssh-tunnel-manager/cli.toml << EOF
connection_mode = "https"
daemon_url = "https://127.0.0.1:3443"
auth_token = "<paste-token-here>"
tls_cert_fingerprint = ""  # Empty = no pinning
EOF
```
- [ ] 3.7: CLI connects via HTTPS (should work with system cert trust)
- [ ] 3.8: Test all CLI commands over HTTPS

**Phase B: HTTPS with Certificate Pinning**

**Tests**:
- [ ] 3.9: Update CLI config with certificate fingerprint:
```bash
# Edit ~/.config/ssh-tunnel-manager/cli.toml
# Add the fingerprint from daemon startup log
tls_cert_fingerprint = "AA:BB:CC:DD:..."
```
- [ ] 3.10: CLI connects successfully with pinned certificate
- [ ] 3.11: Test all CLI commands work
- [ ] 3.12: Regenerate daemon certificate (delete .crt and .key files)
- [ ] 3.13: Restart daemon (new cert generated)
- [ ] 3.14: CLI connection should FAIL (fingerprint mismatch)
- [ ] 3.15: Update CLI config with new fingerprint
- [ ] 3.16: CLI connection should work again

**Expected Results**:
- Self-signed certificate generated automatically
- Certificate fingerprint displayed
- CLI can connect with or without pinning
- Certificate pinning prevents connection after cert change
- All CLI commands work over HTTPS

---

### 4. Cross-Machine Testing (Network Access)

**Goal**: Verify daemon on PC1, CLI on PC2 scenario

**Setup on PC1 (Daemon)**:
```bash
# On PC1 - Start daemon in HTTPS mode
cat > ~/.config/ssh-tunnel-manager/daemon.toml << EOF
listener_mode = "tcp-https"
bind_address = "0.0.0.0:3443"  # Listen on all interfaces
require_auth = true
EOF

# Start daemon and note:
# - Authentication token
# - Certificate fingerprint
# - PC1's IP address (e.g., 192.168.1.100)
```

**Setup on PC2 (CLI)**:
```bash
# On PC2 - Configure CLI to connect to PC1
cat > ~/.config/ssh-tunnel-manager/cli.toml << EOF
connection_mode = "https"
daemon_url = "https://192.168.1.100:3443"  # PC1's IP
auth_token = "<paste-token-from-pc1>"
tls_cert_fingerprint = "<paste-fingerprint-from-pc1>"
EOF
```

**Tests**:
- [ ] 4.1: PC2 CLI can connect to PC1 daemon
- [ ] 4.2: PC2 can add a profile (stored on PC1)
- [ ] 4.3: PC2 can start a tunnel on PC1
- [ ] 4.4: PC2 can watch events from PC1
- [ ] 4.5: PC2 can stop a tunnel on PC1
- [ ] 4.6: Test from multiple devices simultaneously
- [ ] 4.7: Verify tunnel traffic works (connect through opened tunnel)

**Expected Results**:
- CLI on PC2 fully controls daemon on PC1
- All operations work over LAN
- Secure TLS connection
- Authentication required

---

### 5. Error Handling Tests

**Goal**: Verify proper error messages and handling

**Tests**:
- [ ] 5.1: CLI with no daemon running → Clear error message
- [ ] 5.2: CLI with wrong URL → Connection refused error
- [ ] 5.3: CLI with invalid token → 401 Unauthorized
- [ ] 5.4: CLI with wrong cert fingerprint → TLS verification error
- [ ] 5.5: Daemon port already in use → Bind error
- [ ] 5.6: Daemon without write permissions for cert directory → Permission error
- [ ] 5.7: Kill daemon during active tunnel → Tunnel stops gracefully
- [ ] 5.8: Ctrl+C on daemon → Graceful shutdown

---

### 6. Configuration Tests

**Goal**: Verify configuration loading/saving works correctly

**Tests**:
- [ ] 6.1: Daemon without config creates default config file
- [ ] 6.2: Daemon config saved with correct format
- [ ] 6.3: Invalid TOML in daemon config → Parse error
- [ ] 6.4: Missing optional config fields → Uses defaults
- [ ] 6.5: CLI config loads correctly
- [ ] 6.6: Example config files have correct syntax

**Commands**:
```bash
# Verify example configs are valid TOML
toml-validator crates/daemon/daemon.toml.example
toml-validator crates/cli/cli.toml.example

# Or use Rust to parse them
cargo run --bin ssh-tunnel-daemon -- --help  # Should create default config
```

---

### 7. Security Tests

**Goal**: Verify security features work as intended

**Tests**:
- [ ] 7.1: Token file has 0600 permissions (Unix)
- [ ] 7.2: Private key file has 0600 permissions (Unix)
- [ ] 7.3: Token is random UUID format
- [ ] 7.4: Same token used across daemon restarts
- [ ] 7.5: Certificate fingerprint is SHA256
- [ ] 7.6: TLS certificate is valid (self-signed)
- [ ] 7.7: TLS certificate has correct SANs (localhost, 127.0.0.1, ::1)
- [ ] 7.8: HTTP mode shows warning about lack of encryption

**Commands**:
```bash
# Check file permissions
ls -la ~/.config/ssh-tunnel-manager/daemon.token
ls -la ~/.config/ssh-tunnel-manager/server.key

# Inspect certificate
openssl x509 -in ~/.config/ssh-tunnel-manager/server.crt -text -noout
```

---

## Test Execution Order

**Recommended order**:
1. Start with UnixSocket tests (simplest, backward compat)
2. Move to TcpHttp tests (adds auth, easier than HTTPS)
3. Test TcpHttps without pinning
4. Test TcpHttps with pinning
5. Cross-machine tests (if available)
6. Error handling tests
7. Configuration tests
8. Security tests

## Test Environment Setup

**Prerequisites**:
```bash
# Clean state before each test suite
rm -rf ~/.config/ssh-tunnel-manager/
rm -f $XDG_RUNTIME_DIR/ssh-tunnel-manager/ssh-tunnel-manager.sock

# Build latest code
cargo build --release

# Have a test SSH server available for actual tunnel tests
# (Can use any SSH server you have access to)
```

**Tools Needed**:
- `curl` - Test HTTP/HTTPS endpoints directly
- `openssl` - Inspect certificates
- `netstat` or `ss` - Verify ports are bound
- Two machines on same LAN (for cross-machine tests)

## Quick Smoke Test

**Minimal test to verify basic functionality**:

```bash
#!/bin/bash
set -e

echo "=== Quick Smoke Test ==="

# Clean state
rm -rf ~/.config/ssh-tunnel-manager/
rm -f $XDG_RUNTIME_DIR/ssh-tunnel-manager/ssh-tunnel-manager.sock

# Test 1: UnixSocket mode (default)
echo "Testing UnixSocket mode..."
cargo run --release --bin ssh-tunnel-daemon &
DAEMON_PID=$!
sleep 2

# Verify socket exists
if [ -S "$XDG_RUNTIME_DIR/ssh-tunnel-manager/ssh-tunnel-manager.sock" ]; then
    echo "✓ Unix socket created"
else
    echo "✗ Unix socket not found"
    exit 1
fi

# Test CLI connection
cargo run --release --bin ssh-tunnel -- --help > /dev/null
echo "✓ CLI connected"

# Cleanup
kill $DAEMON_PID
rm -rf ~/.config/ssh-tunnel-manager/

echo "=== Smoke test passed ==="
```

## Manual Test Checklist

Print this and check off as you test:

```
BACKWARD COMPATIBILITY (UnixSocket)
□ Daemon starts with defaults
□ CLI connects without config
□ Can add profile
□ Can start tunnel
□ Can stop tunnel
□ Can watch events

HTTP MODE
□ Daemon binds to localhost:3443
□ Token generated and displayed
□ CLI connects with token
□ CLI rejected without token
□ All commands work

HTTPS MODE
□ Certificate auto-generated
□ Fingerprint displayed
□ CLI connects without pinning
□ CLI connects with pinning
□ CLI rejected on cert change
□ All commands work

NETWORK ACCESS
□ CLI on PC2 connects to PC1
□ Remote tunnel control works
□ Multiple clients work
□ Actual tunnel traffic works

ERROR HANDLING
□ Meaningful error messages
□ Graceful shutdown
□ Connection failures handled

SECURITY
□ File permissions correct
□ Token format correct
□ Certificate valid
□ TLS encryption works
```

## Reporting Issues

When reporting issues, include:
1. Which test scenario
2. Daemon config (`~/.config/ssh-tunnel-manager/daemon.toml`)
3. CLI config (`~/.config/ssh-tunnel-manager/cli.toml`)
4. Daemon logs (if running with RUST_LOG=debug)
5. CLI output
6. Error messages

## Next Steps After Testing

After all tests pass:
- Document any discovered issues
- Fix critical bugs
- Proceed with Task 7 (SSH host key verification)
- Consider adding automated integration tests

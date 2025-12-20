# Quick Test Guide

## Automated Testing

### Run the test script
```bash
./test-network-modes.sh
```

This will automatically test:
- ✓ UnixSocket mode (backward compatibility)
- ✓ TcpHttp mode with authentication
- ✓ TcpHttps mode with TLS and certificate pinning

**Expected duration**: ~30 seconds

## Manual Quick Tests

### Test 1: Default Mode (UnixSocket)
```bash
# Clean state
rm -rf ~/.config/ssh-tunnel-manager/

# Start daemon
cargo run --release --bin ssh-tunnel-daemon

# In another terminal:
cargo run --release --bin ssh-tunnel -- --help
# Should work without any configuration
```

### Test 2: HTTP Mode
```bash
# Create config
mkdir -p ~/.config/ssh-tunnel-manager
cat > ~/.config/ssh-tunnel-manager/daemon.toml << 'EOF'
listener_mode = "tcp-http"
bind_address = "127.0.0.1:3443"
require_auth = true
EOF

# Start daemon and note the token
cargo run --release --bin ssh-tunnel-daemon
# Look for: "Token: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"

# Create CLI config with token
cat > ~/.config/ssh-tunnel-manager/cli.toml << 'EOF'
connection_mode = "http"
daemon_url = "http://127.0.0.1:3443"
auth_token = "paste-token-here"
EOF

# Test CLI
cargo run --release --bin ssh-tunnel -- --help
```

### Test 3: HTTPS Mode
```bash
# Create daemon config
cat > ~/.config/ssh-tunnel-manager/daemon.toml << 'EOF'
listener_mode = "tcp-https"
bind_address = "127.0.0.1:3443"
require_auth = true
EOF

# Start daemon and note:
# - Token
# - Certificate fingerprint
cargo run --release --bin ssh-tunnel-daemon

# Create CLI config
cat > ~/.config/ssh-tunnel-manager/cli.toml << 'EOF'
connection_mode = "https"
daemon_url = "https://127.0.0.1:3443"
auth_token = "paste-token-here"
tls_cert_fingerprint = "paste-fingerprint-here"
EOF

# Test CLI
cargo run --release --bin ssh-tunnel -- --help
```

## Testing Actual Tunnel Functionality

Once the network modes work, test with a real SSH server:

```bash
# 1. Add a tunnel profile
cargo run --release --bin ssh-tunnel -- add my-tunnel \
    -H your-ssh-server.com \
    -u your-username \
    -k ~/.ssh/id_rsa \
    -l 8080 \
    -p 80

# 2. Start the tunnel
cargo run --release --bin ssh-tunnel -- start my-tunnel

# 3. Test the tunnel (in another terminal)
curl http://localhost:8080
# Should show content from your-ssh-server.com:80

# 4. Watch events
cargo run --release --bin ssh-tunnel -- watch my-tunnel

# 5. Stop the tunnel
cargo run --release --bin ssh-tunnel -- stop my-tunnel
```

## Cross-Machine Testing (LAN)

### On PC1 (Server - 192.168.1.100):
```bash
# Create HTTPS daemon config
cat > ~/.config/ssh-tunnel-manager/daemon.toml << 'EOF'
listener_mode = "tcp-https"
bind_address = "0.0.0.0:3443"  # Listen on all interfaces
require_auth = true
EOF

# Start daemon
cargo run --release --bin ssh-tunnel-daemon

# IMPORTANT: Note these values:
# - Authentication token: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
# - Certificate fingerprint: AA:BB:CC:DD:...
```

### On PC2 (Client):
```bash
# Create CLI config
cat > ~/.config/ssh-tunnel-manager/cli.toml << 'EOF'
connection_mode = "https"
daemon_url = "https://192.168.1.100:3443"  # PC1's IP
auth_token = "paste-token-from-pc1"
tls_cert_fingerprint = "paste-fingerprint-from-pc1"
EOF

# Test connection
cargo run --release --bin ssh-tunnel -- --help

# Add a profile on PC1 from PC2
cargo run --release --bin ssh-tunnel -- add remote-tunnel \
    -H some-server.com \
    -u username

# Start tunnel on PC1 from PC2
cargo run --release --bin ssh-tunnel -- start remote-tunnel

# The tunnel is now running on PC1, accessible from PC2
```

## Verification Commands

### Check daemon is running
```bash
# UnixSocket mode:
ls -la $XDG_RUNTIME_DIR/ssh-tunnel-manager.sock

# TCP modes:
netstat -tlnp | grep 3443
# or
ss -tlnp | grep 3443
```

### Check TLS certificate
```bash
openssl x509 -in ~/.config/ssh-tunnel-manager/server.crt -text -noout
```

### Check file permissions
```bash
ls -la ~/.config/ssh-tunnel-manager/
# server.key and daemon.token should be -rw------- (600)
```

### Test API directly with curl
```bash
# UnixSocket (requires curl with Unix socket support):
curl --unix-socket $XDG_RUNTIME_DIR/ssh-tunnel-manager.sock http://daemon/api/health

# HTTP:
curl http://127.0.0.1:3443/api/health

# HTTP with auth:
curl -H "X-Tunnel-Token: your-token-here" http://127.0.0.1:3443/api/health

# HTTPS (accepting self-signed cert):
curl -k https://127.0.0.1:3443/api/health

# HTTPS with auth:
curl -k -H "X-Tunnel-Token: your-token-here" https://127.0.0.1:3443/api/health
```

## Common Issues & Solutions

### "Failed to bind to socket"
- Socket file already exists → Remove `$XDG_RUNTIME_DIR/ssh-tunnel-manager.sock`
- Daemon already running → Kill existing daemon process

### "Failed to bind to 0.0.0.0:3443"
- Port already in use → Use different port or kill process using port
- Permission denied → Use port > 1024 or run with sudo (not recommended)

### "Authentication failed: missing token"
- Token not in CLI config → Check `~/.config/ssh-tunnel-manager/cli.toml`
- Token not sent → Check `auth_token` field is not empty

### "Certificate fingerprint mismatch"
- Certificate was regenerated → Update fingerprint in CLI config
- Wrong fingerprint copied → Re-copy from daemon startup log

### CLI can't connect
- Check daemon is running: `ps aux | grep ssh-tunnel-daemon`
- Check config paths: `~/.config/ssh-tunnel-manager/`
- Check logs: daemon writes to stdout/stderr
- Verify URL in CLI config matches daemon bind address

## Debug Mode

For detailed logging:

```bash
# Start daemon with debug logging
RUST_LOG=debug cargo run --release --bin ssh-tunnel-daemon

# Or for specific modules:
RUST_LOG=ssh_tunnel_daemon=debug,tower_http=debug cargo run --release --bin ssh-tunnel-daemon
```

## Test Checklist

Minimal checklist before declaring tests complete:

- [ ] UnixSocket mode works (backward compat)
- [ ] TcpHttp mode with auth works
- [ ] TcpHttps mode with TLS works
- [ ] Certificate pinning works
- [ ] Authentication rejects invalid tokens
- [ ] Certificate fingerprint mismatch is detected
- [ ] File permissions are correct (600 for secrets)
- [ ] Cross-machine connection works (if testing with 2 PCs)
- [ ] Actual SSH tunnel works (end-to-end)
- [ ] All CLI commands work in each mode

## What to Test Next

After network modes are verified:

1. **Concurrent connections**: Multiple CLI clients at once
2. **Reconnection**: Stop/restart daemon while CLI is watching
3. **Invalid configs**: Malformed TOML, missing fields
4. **Network failures**: Disconnect network during active tunnel
5. **Resource limits**: Many tunnels running simultaneously
6. **Long-running stability**: Daemon running for hours/days

## Ready for Task 7?

Once all tests pass, you're ready to implement Task 7 (SSH host key verification).

The network-ready architecture is solid!

# Systemd Setup

Two ways to run the daemon with journald logs:
- Per-user service (default; non-privileged ports).
- System service as a dedicated `tunneld` user with capability to bind ports <1024.

## Quick Installation

Use the provided `install.sh` script for automated installation:

```bash
# User service (recommended for most users)
./scripts/install.sh --user-unit --enable

# System service (for privileged ports or system-wide daemon)
sudo ./scripts/install.sh --system-unit --instance tunneld --enable
```

The script handles building binaries, installing them to `/usr/local/bin`, and setting up systemd units.

## Manual Installation

If you prefer manual installation or need more control:

### Build and install binaries

```bash
cargo build --release --package ssh-tunnel-cli --package ssh-tunnel-daemon
sudo install -Dm755 target/release/ssh-tunnel-daemon /usr/local/bin/ssh-tunnel-daemon
sudo install -Dm755 target/release/ssh-tunnel /usr/local/bin/ssh-tunnel
```

### Option 1: Per-user service (current user)

Uses the Unix socket in `/run/user/$UID/ssh-tunnel-manager/ssh-tunnel-manager.sock` (default) and your config in `~/.config/ssh-tunnel-manager/`.

```bash
mkdir -p ~/.config/systemd/user
cp docs/systemd/ssh-tunnel-daemon.user.service ~/.config/systemd/user/ssh-tunnel-daemon.service
systemctl --user daemon-reload
systemctl --user enable --now ssh-tunnel-daemon.service
journalctl --user-unit ssh-tunnel-daemon -f
```

CLI defaults to the Unix socket and should work without extra config.

### Option 2: System service as `tunneld` (privileged ports)

Recommended for ports <1024. The provided template sets `AmbientCapabilities=CAP_NET_BIND_SERVICE`.

1) Create the service account (once):
```bash
# Creates system user and group "tunneld"
sudo useradd -r -m -d /var/lib/ssh-tunnel-manager -s /usr/sbin/nologin tunneld
```

2) (Optional) For multi-user access, add users to the tunneld group:
```bash
# Add users who need to manage tunnels via the CLI
sudo usermod -aG tunneld alice
sudo usermod -aG tunneld bob

# Users must log out and log back in for group membership to take effect
# Verify membership with: groups alice
```

**Note**: Multi-user access also requires `group_access = true` in the daemon config (see step 3).

3) Install the systemd unit:
```bash
sudo cp docs/systemd/ssh-tunnel-daemon@.service /etc/systemd/system/ssh-tunnel-daemon@.service
sudo systemctl daemon-reload
```

4) Configure the daemon for that user (TCP mode so the CLI can talk to it):
```bash
sudo -u tunneld mkdir -p /var/lib/ssh-tunnel-manager/.config/ssh-tunnel-manager
sudo -u tunneld tee /var/lib/ssh-tunnel-manager/.config/ssh-tunnel-manager/daemon.toml >/dev/null <<'EOF'
listener_mode = "tcp-http"      # or "tcp-https" with cert/key paths
bind_address = "127.0.0.1:3443" # use <1024 if you need privileged ports
require_auth = true
known_hosts_path = "/var/lib/ssh-tunnel-manager/.config/ssh-tunnel-manager/known_hosts"
# Enable if multiple users need access (requires users to be in tunneld group)
group_access = false            # Set to true for multi-user access
EOF
```

**For multi-user access**: Set `group_access = true` and ensure users are in the tunneld group (step 2).

5) Start and enable:
```bash
sudo systemctl enable --now ssh-tunnel-daemon@tunneld.service
sudo journalctl -u ssh-tunnel-daemon@tunneld -f
```

### CLI config when using the system service

Point the CLI at the TCP endpoint and supply the token created for `tunneld`:

```toml
# ~/.config/ssh-tunnel-manager/cli.toml
connection_mode = "http"          # or "https" if enabled
daemon_url = "127.0.0.1:3443"
auth_token = "<contents of /var/lib/ssh-tunnel-manager/.config/ssh-tunnel-manager/daemon.token>"
```

For HTTPS, also set `tls_cert_fingerprint = "<sha256-fingerprint>"`.

### Notes

- The system unit sets `XDG_RUNTIME_DIR=/run/ssh-tunnel-manager` so the daemon has a runtime dir even without a login session. The unit creates it via `RuntimeDirectory`.
- If you prefer to keep Unix-socket mode for the system unit, also set `XDG_RUNTIME_DIR=/run/ssh-tunnel-manager` when running the CLI so it looks for the same socket path.
- Logs are in journald; follow them with `journalctl -u ssh-tunnel-daemon@tunneld -f` (or `--user-unit` for the user service).

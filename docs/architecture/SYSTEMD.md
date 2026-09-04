# Systemd Setup

Two ways to run the daemon with journald logs:
- Per-user service (default; non-privileged ports).
- System service as a dedicated `tunneld` user with capability to bind ports <1024.

> **The daemon refuses to run as root.** It exits at startup with an explanation.
>
> Root does not work and cannot be made to work: the socket path comes from
> `$XDG_RUNTIME_DIR`, which for root in a login shell is `/run/user/0` — a per-user directory
> that is mode 0700 *by design*. No other user can traverse it, so the CLI and the GUI simply
> report that no daemon is running. Widening the socket does not help, because it remains
> owned by `root:root`.
>
> Root was only ever needed to bind forward ports below 1024, and `CAP_NET_BIND_SERVICE`
> grants exactly that without giving the daemon — and every SSH key it opens — the rest of
> root's authority. The system unit below already sets it; for a daemon started by hand, use
> `sudo setcap cap_net_bind_service=+ep /path/to/ssh-tunnel-daemon` and then run it as your
> normal user.

## Quick Installation

Use the provided `install.sh` script for automated installation:

```bash
# User service (recommended for most users)
./scripts/install.sh --user-unit --enable

# System service (for privileged ports or system-wide daemon)
sudo ./scripts/install.sh --system-unit --instance tunneld --enable
```

The script handles building binaries, installing them to `/usr/local/bin`, and setting up systemd units.

## Credential storage for system services

Secret Service normally needs a desktop D-Bus session, but its absence no longer means that
all credential storage is unavailable. With the default `credential_store = "auto"`, the
daemon tries Secret Service and falls back to the Linux kernel keyutils keyring.

| Store | Desktop session | Survives reboot | Suitable for a system service |
|---|---|---|---|
| Secret Service | Required in normal setups | Yes | Usually no |
| keyutils | Not required | **No** | Yes, as a session-lifetime cache |
| none | Not required | — | Yes, but every secret requires a client prompt |

The daemon reports the selected store at startup and in `DaemonInfo`. Force it in the service
user's `daemon.toml`:

```toml
credential_store = "keyutils" # or "auto", "secret-service", "none"
```

Keyutils solves access without a desktop session, but its contents disappear on reboot. A
daemon with no attached client can therefore still fail after restart when it needs a lost
password/passphrase: it emits its structured prompt and times out after 60 seconds if nobody
answers. Persistent unattended credentials require a separate reviewed security design; see
the deferred work in [ROADMAP.md](../ROADMAP.md).

### Operational choices

**Option 1: Use an unencrypted service key**
```bash
# Generate a key WITHOUT a passphrase for the service
sudo -u tunneld ssh-keygen -t ed25519 -f /var/lib/tunneld/.ssh/id_service -N ""

# Use in profiles (no passphrase needed)
sudo -u tunneld ssh-tunnel add myprofile --host server.com --user myuser \
  --key /var/lib/tunneld/.ssh/id_service
```

This avoids stored authentication secrets but changes the key's at-rest threat model. Protect
the service account and key file permissions.

**Option 2: Use keyutils and repopulate after reboot**

Choose `credential_store = "keyutils"`, attach a CLI/GUI client after restart, and store or
answer the credential again. Do not describe this as persistent unattended operation.

**Option 3: Store nothing**

Set `credential_store = "none"`, or disable all store access for automation:

```systemd
# In /etc/systemd/system/ssh-tunnel-daemon@.service
[Service]
Environment="SSH_TUNNEL_SKIP_KEYRING=1"
```

### Diagnosing credential-store issues

Check `journalctl` for the startup line naming Secret Service, keyutils, or disabled storage.
An authentication timeout means no client answered; it does not by itself identify which
store was selected. Use structured `password_storage` values (`client`, `daemon-host`, or
`none`) rather than the legacy ambiguous `keychain` value when creating new profiles.

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
bind_host = "127.0.0.1"         # IP address to bind to
bind_port = 3443                # Port to listen on (use <1024 if you need privileged ports)
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
daemon_host = "127.0.0.1"
daemon_port = 3443
auth_token = "<contents of /var/lib/ssh-tunnel-manager/.config/ssh-tunnel-manager/daemon.token>"
```

For HTTPS, also set `tls_cert_fingerprint = "<sha256-fingerprint>"`.

### Notes

- The system unit sets `XDG_RUNTIME_DIR=/run/ssh-tunnel-manager` so the daemon has a runtime dir even without a login session. The unit creates it via `RuntimeDirectory`.
- If you prefer to keep Unix-socket mode for the system unit, also set `XDG_RUNTIME_DIR=/run/ssh-tunnel-manager` when running the CLI so it looks for the same socket path.
- Logs are in journald; follow them with `journalctl -u ssh-tunnel-daemon@tunneld -f` (or `--user-unit` for the user service).

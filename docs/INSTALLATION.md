# Installation Guide

**Version**: v0.1.9
**Last Updated**: 2025-12-31

## Quick Start

For most users, the easiest installation method is:

1. **Install** .deb packages (Ubuntu/Debian) or build from source
2. **Start** the daemon as a user service (`systemctl --user enable --now ssh-tunnel-daemon`)
3. **Launch** the GUI (`ssh-tunnel-gtk`) - configuration wizard runs automatically on first launch
4. **Create** profiles and start tunnels

See detailed instructions below for your platform.

## Debian/Ubuntu (Recommended)

Pre-built .deb packages are available for Ubuntu/Debian systems. Three separate packages are available:

```bash
# Download packages from releases (v0.1.9)
wget https://github.com/SchirmForge/ssh-tunnel-manager/releases/download/v0.1.9/ssh-tunnel-daemon_0.1.9-0_amd64.deb
wget https://github.com/SchirmForge/ssh-tunnel-manager/releases/download/v0.1.9/ssh-tunnel-cli_0.1.9-0_amd64.deb
wget https://github.com/SchirmForge/ssh-tunnel-manager/releases/download/v0.1.9/ssh-tunnel-gui-gtk_0.1.9-0_amd64.deb

# Install all components:
sudo dpkg -i ssh-tunnel-daemon_0.1.9-0_amd64.deb \
                ssh-tunnel-cli_0.1.9-0_amd64.deb \
                ssh-tunnel-gui-gtk_0.1.9-0_amd64.deb
sudo apt-get install -f  # Install dependencies if needed
```

**Or install only what you need:**

```bash
# Minimal: Daemon only (for headless servers)
sudo dpkg -i ssh-tunnel-daemon_0.1.9-0_amd64.deb

# CLI: Daemon + CLI tool
sudo dpkg -i ssh-tunnel-daemon_0.1.9-0_amd64.deb \
                ssh-tunnel-cli_0.1.9-0_amd64.deb

# GUI: All components
sudo dpkg -i ssh-tunnel-daemon_0.1.9-0_amd64.deb \
                ssh-tunnel-cli_0.1.9-0_amd64.deb \
                ssh-tunnel-gui-gtk_0.1.9-0_amd64.deb
```

### What Gets Installed

**ssh-tunnel-daemon** package:
- `/usr/bin/ssh-tunnel-daemon` - Background daemon
- `/usr/lib/systemd/user/ssh-tunnel-daemon.service` - systemd user service
- `/usr/lib/systemd/system/ssh-tunnel-daemon@.service` - systemd system service (template)

**ssh-tunnel-cli** package:
- `/usr/bin/ssh-tunnel` - CLI tool

**ssh-tunnel-gui-gtk** package:
- `/usr/bin/ssh-tunnel-gtk` - GTK GUI
- Desktop entry for the GUI application

## Other Distributions

### RPM (Fedora, RHEL, openSUSE)

RPM packages are coming soon.

### From Source

**Prerequisites:**
```bash
# Ubuntu/Debian
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev

# Fedora
sudo dnf install gcc pkg-config gtk4-devel libadwaita-devel

# Arch
sudo pacman -S base-devel gtk4 libadwaita
```

**Build and install:**
```bash
# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/yourusername/ssh-tunnel-manager
cd ssh-tunnel-manager
cargo build --release --workspace --exclude ssh-tunnel-gui-qt

# Install binaries (adjust prefix as needed)
sudo install -Dm755 target/release/ssh-tunnel-daemon /usr/local/bin/
sudo install -Dm755 target/release/ssh-tunnel /usr/local/bin/
sudo install -Dm755 target/release/ssh-tunnel-gtk /usr/local/bin/

# Install systemd service (optional)
mkdir -p ~/.config/systemd/user/
cp scripts/systemd/ssh-tunnel-daemon.service ~/.config/systemd/user/
systemctl --user daemon-reload
```

## Post-Installation Setup

### 1. Choose Daemon Mode

The daemon can run in two modes depending on your port forwarding needs:

**Option A: User Service**
- Runs as your user account
- Starts when you log in
- **Cannot forward privileged ports (ports below 1024)**
- Use this if you only need ports 1024 and above

**Option B: System Service**
- Runs as a specific user account (typically your account or a dedicated service account)
- Starts at boot (before login)
- **Can forward all ports including privileged ports (80, 443, etc.)**
- Use this if you need to forward ports like 80 (HTTP) or 443 (HTTPS)
- Requires specifying which user account to run as

### 2. Start the Daemon

**Option A: User Service (Ports 1024+)**

```bash
# Start now and enable on login
systemctl --user enable --now ssh-tunnel-daemon

# Check status
systemctl --user status ssh-tunnel-daemon
```

**Option B: System Service (All Ports Including <1024)**

The system service uses a template that requires specifying a username:

#### Run as your current user
```bash
# Run and enable at user login
sudo systemctl enable --now ssh-tunnel-daemon@$USER
```
#### Or run as a specific user (the example below assumes *tunneld* runs the daemon)
```bash
# Create tunneld user 
sudo useradd --system --home-dir /var/lib/tunneld --create-home --shell /usr/sbin/nologin --comment "Tunnel daemon" tunneld
# Or if your distro uses adduser:
sudo adduser --system --home /var/lib/tunneld --shell /usr/sbin/nologin --group tunneld

# Run and enable at system start
sudo systemctl enable --now ssh-tunnel-daemon@tunneld

# Check status
sudo systemctl status ssh-tunnel-daemon@tunneld

# Check logs
sudo journalctl -u ssh-tunnel-daemon@tunneld -f
```

**Allowing other users to access the daemon:**

By default, the daemon socket has restrictive permissions (0600/0700) that only allow the daemon user to connect. To allow other users on the same machine to access the daemon, you have two options:

**Option 1: Enable group access (recommended for local access)**

```bash
# 1. Add users to the daemon's group
sudo usermod -aG tunneld otheruser1
sudo usermod -aG tunneld otheruser2

# 2. Enable group access in daemon config
sudo nano /var/lib/tunneld/.config/ssh-tunnel-manager/daemon.toml
# Add or change: group_access = true

# 3. Restart the daemon
sudo systemctl restart ssh-tunnel-daemon@tunneld

# 4. Users need to log out and back in, or run:
newgrp tunneld
```

This changes socket permissions from 0600/0700 to 0660/0770, allowing group members to access the socket.

**Option 2: Use HTTPS mode (for network or cross-user access)**

See the "Enabling Network Access (HTTPS Mode)" section below for detailed instructions.

**Important notes for system service:**
- The specified user must exist and have a home directory
- Configuration will be stored in that user's home: `~/.config/ssh-tunnel-manager/`
- The daemon runs with that user's permissions
- Can forward privileged ports (below 1024) when run as system service

**Manual start (testing only):**
```bash
ssh-tunnel-daemon &
```

### 3. First-Time Setup

#### Option A: Using the GUI (Easiest)

```bash
# Launch the GUI
ssh-tunnel-gtk
```

The **configuration wizard** runs automatically on first launch (v0.1.9):
- Detects daemon-generated configuration snippet and offers to import it
- For network access (daemon on `0.0.0.0`), prompts for actual IP address
- Falls back to manual configuration dialog if snippet not found
- Validates all settings before saving

After setup, you can:
- Create profiles using the "New Profile" button
- Start/stop tunnels with one click
- View real-time status with colored indicators

#### Option B: Using the CLI

```bash
# Verify daemon is running
ssh-tunnel info

# Create your first profile (interactive prompts)
ssh-tunnel add myprofile

# Or non-interactive
ssh-tunnel add myprofile \
  --host ssh.example.com \
  --user myuser \
  --key ~/.ssh/id_ed25519 \
  --local-port 8080 \
  --remote-host localhost \
  --remote-port 80

# Start the tunnel
ssh-tunnel start myprofile

# Check status
ssh-tunnel status myprofile
```

## Advanced Configuration

### Enabling Network Access (HTTPS Mode)

By default, the daemon uses a Unix socket for local-only access. To enable network access (e.g., to connect from a remote machine or different user account), you need to switch to HTTPS mode.

**Step 1: Configure the daemon for HTTPS**

Edit the daemon configuration file:
```bash
# Location depends on which service mode you're using
# For user service:
nano ~/.config/ssh-tunnel-manager/daemon.toml

# For system service running as specific user:
sudo nano /home/username/.config/ssh-tunnel-manager/daemon.toml
```

Change the listener mode and bind settings:
```toml
# Change from unix-socket to tcp-https
listener_mode = "tcp-https"

# Bind to all interfaces (or use specific IP)
bind_host = "0.0.0.0"  # or "192.168.1.100" for specific interface
bind_port = 3443

# Authentication is required for network access (already enabled by default)
require_auth = true
```

**Step 2: Restart the daemon**

```bash
# For user service
systemctl --user restart ssh-tunnel-daemon

# For system service
sudo systemctl restart ssh-tunnel-daemon@$USER
```

**Step 3: Use the auto-generated configuration snippet (Recommended)**

The daemon automatically generates a configuration snippet for clients when configured for network access. This is the easiest and most secure way to configure your client.

**Configuration snippet location:**
```bash
# For user service:
cat ~/.config/ssh-tunnel-manager/cli-config.snippet

# For system service:
sudo cat /home/username/.config/ssh-tunnel-manager/cli-config.snippet
```

**Automatic import:**

Both CLI and GUI will automatically detect and offer to import this snippet on first use:

```bash
# CLI - Run any command and you'll be prompted
ssh-tunnel start <any-profile>
# Will prompt to import the configuration snippet

# GUI - Launch the application
ssh-tunnel-gtk
# Will show configuration wizard on first launch
```

**Understanding empty `daemon_host` (network access scenarios):**

When the daemon binds to `0.0.0.0` (all network interfaces), the configuration snippet will contain an empty `daemon_host` field:

```toml
connection_mode = "https"
daemon_host = ""  # Empty - daemon listens on all interfaces
daemon_port = 3443
auth_token = "..."
tls_cert_fingerprint = "..."
# Note: daemon_host is empty because daemon is configured to listen on
# all interfaces (0.0.0.0). You must specify the actual IP address to
# connect to (e.g., 192.168.1.100)
```

This is intentional because:
- `0.0.0.0` is a bind-all address (server-side) and cannot be used as a connection target (client-side)
- The actual IP address depends on which network interface you want to connect through

**What happens when you import:**

Both CLI and GUI will automatically detect the empty `daemon_host` and prompt you for the actual IP address:

1. **CLI**: Interactive prompt asking for the daemon's IP address
   ```bash
   ssh-tunnel start myprofile
   > Configuration snippet detected. Import it? (y/n): y
   > Daemon IP address required. Enter the IP to connect to: 192.168.1.100
   > Configuration saved successfully
   ```

2. **GUI**: Dialog box requesting the daemon's IP address
   - Shows suggested default (e.g., 192.168.1.100)
   - Validates the input before saving

The configuration is then saved with your specified IP address to `~/.config/ssh-tunnel-manager/cli.toml`.

**Step 4: Manual client configuration (Advanced)**

If you prefer to manually configure the client without using the snippet:

```bash
nano ~/.config/ssh-tunnel-manager/cli.toml
```

Create configuration matching this format:
```toml
# Daemon connection settings
connection_mode = "https"
daemon_host = "192.168.1.100"  # Replace with your daemon's actual IP
daemon_port = 3443
auth_token = "paste-token-here"  # Get from daemon.token file
tls_cert_fingerprint = "paste-fingerprint-here"  # Get from tls-cert.fingerprint
```

**Get the authentication token:**
```bash
# On daemon machine
cat ~/.config/ssh-tunnel-manager/daemon.token
```

**Get the TLS certificate fingerprint:**
```bash
# For user service:
cat ~/.config/ssh-tunnel-manager/tls-cert.fingerprint

# For system service:
sudo cat /home/username/.config/ssh-tunnel-manager/tls-cert.fingerprint
```

The fingerprint looks like:
```
A1:B2:C3:D4:E5:F6:G7:H8:I9:J0:K1:L2:M3:N4:O5:P6:Q7:R8:S9:T0:U1:V2:W3:X4:Y5:Z6:A7:B8:C9:D0:E1:F2
```

**Alternative method** - Calculate from certificate file:
```bash
openssl x509 -in ~/.config/ssh-tunnel-manager/daemon.crt -noout -fingerprint -sha256 | cut -d'=' -f2
```

**Step 6: Configure firewall (if needed)**

```bash
# Allow incoming connections on port 3443
sudo firewall-cmd --permanent --add-port=3443/tcp
sudo firewall-cmd --reload

# Or using ufw:
sudo ufw allow 3443/tcp
```

**Security Notes:**
- HTTPS mode uses self-signed certificates with fingerprint pinning for security
- Authentication tokens are required for all network access
- Never use HTTP mode over the network (it's restricted to localhost only)
- The daemon config file and token have 0600 permissions (readable only by owner)

## Uninstallation

**Stop the daemon first:**

```bash
# If using user service
systemctl --user disable --now ssh-tunnel-daemon

# If using system service
sudo systemctl disable --now ssh-tunnel-daemon@$USER
# Or for specific user: sudo systemctl disable --now ssh-tunnel-daemon@username
```

**Debian/Ubuntu:**
```bash
# Remove all components
sudo apt remove ssh-tunnel-daemon ssh-tunnel-cli ssh-tunnel-gui-gtk

# Or remove individually
sudo apt remove ssh-tunnel-gui-gtk  # GUI only
sudo apt remove ssh-tunnel-cli      # CLI only
sudo apt remove ssh-tunnel-daemon   # Daemon only (will break CLI/GUI)
```

**From source:**
```bash
sudo rm /usr/local/bin/ssh-tunnel{,-daemon,-gtk}
systemctl --user disable --now ssh-tunnel-daemon
rm ~/.config/systemd/user/ssh-tunnel-daemon.service
```

**Remove configuration and data:**
```bash
rm -rf ~/.config/ssh-tunnel-manager
```

## Troubleshooting

### CLI Configuration Missing (401 Unauthorized)

**Error**: `Failed to establish SSE connection: Daemon returned non-success status for events: 401 Unauthorized`

This means your CLI configuration file is missing. The CLI will automatically detect this and offer to copy the daemon-generated config snippet.

**Automatic Solution**: Just run any daemon command and follow the prompts:
```bash
ssh-tunnel start <any-profile>
# You'll be prompted to copy the config snippet automatically
```

**Manual Solution**: Copy the daemon-generated snippet:
```bash
cp ~/.config/ssh-tunnel-manager/cli-config.snippet ~/.config/ssh-tunnel-manager/cli.toml
```

### Authentication Fails with "Server requires: publickey"

Your profile is configured for password authentication, but the server only accepts SSH keys.

**Solution**: Recreate the profile with an SSH key:
```bash
ssh-tunnel delete myprofile
ssh-tunnel add myprofile
# Enter your SSH key path when prompted
```

Or use the GUI to edit the profile and switch to SSH key authentication.

### Can't Bind to Privileged Port (≤1024)

**Error**: `Permission denied binding to 0.0.0.0:443. Port 443 is privileged`

**Solution**: Use system service instead of user service (see "Post-Installation Setup" above), or run daemon with sudo:
```bash
# Option 1: Use system service (recommended)
sudo systemctl enable --now ssh-tunnel-daemon@$USER

# Option 2: Run with sudo (not recommended for production)
sudo RUST_LOG=info ssh-tunnel-daemon

# Option 3: Grant capability (one-time, for manual runs)
sudo setcap cap_net_bind_service=+ep /usr/bin/ssh-tunnel-daemon
```

### Keychain Not Working

**Error**: `Failed to store password in keychain`

**Solution**: Ensure a keychain service is running:
```bash
# GNOME
gnome-keyring-daemon --start

# KDE
kwalletd5

# Or skip keyring (passwords won't be stored)
export SSH_TUNNEL_SKIP_KEYRING=1
```

### Missing Dependencies

If you get errors about missing libraries:

```bash
# Ubuntu/Debian
sudo apt install libgtk-4-1 libadwaita-1-0

# Fedora
sudo dnf install gtk4 libadwaita

# Arch
sudo pacman -S gtk4 libadwaita
```

### Daemon Won't Start

Check systemd logs:
```bash
# For user service
journalctl --user -u ssh-tunnel-daemon -f

# For system service
sudo journalctl -u ssh-tunnel-daemon@$USER -f
```

Common issues:
- Another instance already running: Check `ps aux | grep ssh-tunnel-daemon`
- Permission issues: Check file permissions in `~/.config/ssh-tunnel-manager/`
- Missing config: Daemon creates default config on first run

### GUI Won't Connect to Daemon

**Symptoms**: GUI shows "Daemon Disconnected" or can't start tunnels

**Solution**:
1. Ensure daemon is running:
   ```bash
   systemctl --user status ssh-tunnel-daemon
   # Or: ssh-tunnel info
   ```

2. If daemon is on a different host (HTTPS mode), ensure configuration is correct:
   ```bash
   # GUI will prompt with configuration wizard on first launch
   # Or manually check:
   cat ~/.config/ssh-tunnel-manager/cli.toml
   ```

3. For remote daemons, check firewall allows port 3443

### Remote Daemon: SSH Keys Not Found

**Error**: `SSH key not found: /home/daemon/.ssh/id_rsa`

This occurs when connecting to a remote daemon - SSH keys must be on the daemon host, not your local machine.

**Solution**:
```bash
# On daemon host, ensure SSH key exists
ssh daemon-host "ls -la ~/.ssh/"

# Copy your key to daemon host if needed
scp ~/.ssh/id_rsa daemon-host:~/.ssh/
scp ~/.ssh/id_rsa.pub daemon-host:~/.ssh/

# Set correct permissions
ssh daemon-host "chmod 600 ~/.ssh/id_rsa"
```

For more help, check:
- [Architecture Documentation](ARCHITECTURE.md)
- [Project Status](PROJECT_STATUS.md)
- [GitHub Issues](https://github.com/SchirmForge/ssh-tunnel-manager/issues)

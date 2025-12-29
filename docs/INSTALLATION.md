# Installation Guide

## Debian/Ubuntu (Recommended)

Pre-built .deb packages are available for Ubuntu/Debian systems. Three separate packages are available:

```bash
# Download packages from releases
wget https://github.com/SchirmForge/ssh-tunnel-manager/releases/download/v0.1.8/ssh-tunnel-daemon_0.1.8-4_amd64.deb
wget https://github.com/SchirmForge/ssh-tunnel-manager/releases/download/v0.1.8/ssh-tunnel-cli_0.1.8-4_amd64.deb
wget https://github.com/SchirmForge/ssh-tunnel-manager/releases/download/v0.1.8/ssh-tunnel-gui-gtk_0.1.8-4_amd64.deb

# Install all components:
sudo dpkg -i ssh-tunnel-daemon_0.1.8-4_amd64.deb \
                ssh-tunnel-cli_0.1.8-4_amd64.deb \
                ssh-tunnel-gui-gtk_0.1.8-4_amd64.deb
sudo apt-get install -f  # Install dependencies if needed
```

**Or install only what you need:**

```bash
# Minimal: Daemon only (for headless servers)
sudo dpkg -i ssh-tunnel-daemon_0.1.8-4_amd64.deb

# CLI: Daemon + CLI tool
sudo dpkg -i ssh-tunnel-daemon_0.1.8-4_amd64.deb \
                ssh-tunnel-cli_0.1.8-4_amd64.deb

# GUI: All components
sudo dpkg -i ssh-tunnel-daemon_0.1.8-4_amd64.deb \
                ssh-tunnel-cli_0.1.8-4_amd64.deb \
                ssh-tunnel-gui-gtk_0.1.8-4_amd64.deb
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

```bash
# Run as your current user
sudo systemctl enable --now ssh-tunnel-daemon@$USER

# Or run as a specific user (must have a home directory for config storage)
sudo systemctl enable --now ssh-tunnel-daemon@username

# Check status
sudo systemctl status ssh-tunnel-daemon@$USER
```

**Important notes for system service:**
- The specified user must exist and have a home directory
- Configuration will be stored in that user's home: `~/.config/ssh-tunnel-manager/`
- The daemon runs with that user's permissions
- Can forward privileged ports (below 1024) when run as system service

**Manual start (testing only):**
```bash
ssh-tunnel-daemon &
```

### 3. Verify Installation

```bash
# Check daemon status
ssh-tunnel info

# Launch GUI (if installed)
ssh-tunnel-gtk
```

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
# Or: sudo journalctl -u ssh-tunnel-daemon@username -f
```

Or check if already running:
```bash
ps aux | grep ssh-tunnel-daemon
```

### GUI Won't Connect to Daemon

Ensure daemon is running:
```bash
ssh-tunnel info
```

If daemon is on a different host, configure CLI:
```bash
# Copy the config snippet generated by daemon
ssh-tunnel start <any-profile>  # Will prompt for config
```

See [Troubleshooting](TROUBLESHOOTING.md) for more help.

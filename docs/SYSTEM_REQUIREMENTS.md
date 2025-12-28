# System Requirements

## Supported Platforms

**Operating Systems:**
- Linux (x86_64, aarch64)
  - Ubuntu 22.04 LTS or newer
  - Debian 11 (Bullseye) or newer
  - Fedora 38 or newer
  - Arch Linux (rolling release)
  - Other distributions with compatible dependencies

**Not Supported:**
- Windows (not planned)
- macOS (not planned)
- BSD systems (may work but untested)

## Runtime Dependencies

### Core Dependencies (All Components)

**Required:**
- Linux kernel 5.10 or newer (for modern networking features)
- glibc 2.31 or newer (or musl libc)
- D-Bus session bus (optional but recommendend for keyring access)
- systemd (optional but recommended for service management)

### GUI-Specific Dependencies

**Required for GTK GUI:**
- GTK4 ≥ 4.10
- libadwaita ≥ 1.4
- Secret Service API provider:
  - gnome-keyring (GNOME/GTK environments)
  - KDE Wallet (KDE/Qt environments)
  - Any compatible Secret Service implementation

### Network Dependencies

**Remote SSH Server:**
- OpenSSH server (or compatible SSH server)
- SSH protocol version 2

**No OpenSSL Required:**
- Uses pure Rust TLS implementation (rustls)
- No system OpenSSL dependency needed

## Hardware Requirements

### Minimum Requirements

**CPU:**
- x86_64 or aarch64 processor
- 1 GHz or faster

**Memory:**
- 256 MB RAM (daemon only)
- 512 MB RAM (with GUI)

**Disk Space:**
- 50 MB for binaries
- 10 MB for configuration and logs

### Recommended Specifications

**For optimal performance:**
- 2 GHz dual-core processor or better
- 1 GB RAM or more
- SSD storage for faster profile loading

## Build Dependencies

Only needed if building from source.

### Compiler and Build Tools

**Required:**
- Rust ≥ 1.75 (stable toolchain)
- cargo (Rust package manager)
- C/C++ compiler (gcc or clang)
- pkg-config
- make (GNU Make)

### Development Libraries

**GTK/Libadwaita:**
```bash
# Ubuntu/Debian
sudo apt install libgtk-4-dev libadwaita-1-dev

# Fedora
sudo dnf install gtk4-devel libadwaita-devel

# Arch
sudo pacman -S gtk4 libadwaita
```

## Network Requirements

### Firewall Configuration

**For local-only use (default):**
- No firewall changes needed
- Daemon uses Unix socket: `$XDG_RUNTIME_DIR/ssh-tunnel-manager/daemon.sock`

**For network access (TCP HTTPS mode):**
- Open TCP port (default: 3443)
- Configure firewall to allow inbound connections:
  ```bash
  sudo firewall-cmd --permanent --add-port=3443/tcp
  sudo firewall-cmd --reload
  ```

### Remote SSH Access

**Outbound connections:**
- TCP port 22 (SSH) must be allowed outbound
- Or custom SSH port as configured in profiles

**No special configuration needed for:**
- Local port forwarding (default setup)
- NAT traversal (works behind NAT)

## Desktop Environment Compatibility

### Tested Environments

**Fully Supported:**
- ✅ GNOME 43+ (primary development environment)
- ✅ KDE Plasma 5.27+
- ✅ XFCE 4.18+

**Should Work (untested):**
- Cinnamon
- MATE
- Budgie
- Elementary OS

**Known Issues:**
- ⚠️ Tiling window managers may require manual configuration for dialogs

## Keyring/Secret Storage

### Requirements

**One of the following:**
- gnome-keyring (GNOME)
- KDE Wallet (KDE)
- Any Secret Service API provider

### Headless/Server Systems

For systems without a graphical session:

**Option 1: Skip keyring**
```bash
export SSH_TUNNEL_SKIP_KEYRING=1
```

**Option 2: Use gnome-keyring in headless mode**
```bash
# Start gnome-keyring daemon
eval $(gnome-keyring-daemon --start)
export $(gnome-keyring-daemon --start)
```

See [Headless Setup](headless-setup.md) for detailed instructions.

## Performance Characteristics

### Resource Usage (Typical)

**Daemon (idle):**
- CPU: < 1%
- Memory: ~10-20 MB
- Disk I/O: minimal (log rotation only)

**Daemon (active tunnels):**
- CPU: < 5% per tunnel
- Memory: ~5-10 MB per tunnel
- Network: transparent (no additional overhead beyond SSH)

**GUI (idle):**
- CPU: < 2%
- Memory: ~50-80 MB
- GPU: minimal (hardware accelerated rendering)

**GUI (active):**
- CPU: < 5%
- Memory: ~80-100 MB

### Scalability

**Tested limits:**
- Up to 50 concurrent tunnels per daemon
- 100+ profiles stored
- GUI handles 50+ profiles without performance degradation

**Practical limits:**
- System file descriptor limits
- Available network bandwidth
- SSH server connection limits

## Security Requirements

### File Permissions

**Automatic hardening:**
- All sensitive files created with 0600 permissions
- Configuration directory: 0700
- Restrictive umask (0077) enforced

### SELinux/AppArmor

**Currently:**
- ❌ No official SELinux policy
- ❌ No official AppArmor profile

**Workaround:**
- Run in permissive mode
- Or create custom policy (contributions welcome)

## Upgrading System Dependencies

### GTK4/Libadwaita Updates

The application is compatible with:
- GTK4: 4.10 through 4.16+
- libadwaita: 1.4 through 1.6+

Newer versions should work without issues.

### Rust Toolchain

**Minimum supported Rust version (MSRV):** 1.75

To update Rust:
```bash
rustup update stable
```

## Compatibility Notes

### Distribution-Specific Issues

**Ubuntu 20.04 LTS:**
- ❌ GTK4 < 4.10 - Upgrade to Ubuntu 22.04 or newer

**Debian 11 (Bullseye):**
- ⚠️ May need backports for GTK4/libadwaita
- Consider upgrading to Debian 12 (Bookworm)

**Fedora:**
- ✅ Fully compatible from Fedora 38 onwards

**Arch Linux:**
- ✅ Rolling release always compatible

See [Distribution Support](distribution-support.md) for detailed compatibility matrix.

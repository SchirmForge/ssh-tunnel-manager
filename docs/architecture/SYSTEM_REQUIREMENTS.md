# System Requirements

**Version**: v0.6.0

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

The broad Linux list above describes the CLI and daemon. The production GUI has a deliberately
narrower validation target:

- Bazzite based on Fedora 44
- GTK 4.22, libadwaita 1.9, GLib 2.88
- Rust 1.98.0

No compatibility claim is made yet for the production GUI on other Linux distributions,
Windows, or macOS. That work remains explicitly out of scope.

## Runtime Dependencies

### Core Dependencies (All Components)

**Required:**
- Linux kernel 5.10 or newer (for modern networking features)
- glibc 2.31 or newer (or musl libc)
- D-Bus session bus (optional but recommendend for keyring access)
- systemd (optional but recommended for service management)

### GUI-Specific Dependencies

**Required for the production GUI:**
- GTK 4.22
- libadwaita 1.9
- GLib 2.88
- A Secret Service provider for persistent client-held credentials; the application remains
  usable without one and reports credential-store errors

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
- Rust 1.98.0 (pinned by `rust-toolchain.toml`; rustup installs it automatically)
- cargo (Rust package manager)
- C/C++ compiler (gcc or clang)
- **cmake** — `aws-lc-sys`, pulled in by both `rustls` and `russh`, compiles C and will not
  build without it. This is easy to miss: `cargo metadata`, `cargo fmt` and
  `cargo update --dry-run` all succeed without cmake because they never compile.
- pkg-config
- make (GNU Make)

`containers/Containerfile.dev` defines an environment with all of the above, usable with
distrobox or Fedora toolbox. See [../DEVELOPMENT.md](../DEVELOPMENT.md).

**For the test suite only:** `openssh-server` (the tier-2 live SSH fixture) and
`gnome-keyring` plus `dbus-run-session` (the credential store tests). Neither is needed to
build or run the application.

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

The production GUI is validated in the Fedora 44 development environment only:

```bash
sudo dnf install gtk4-devel libadwaita-devel glib2-devel gcc cmake pkg-config
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

### Production GUI desktop environment

The accepted runtime target is the current Bazzite/Fedora 44 desktop. No broader desktop-
environment claim is made yet. Keyboard-only and mockup comparison checks remain tracked
separately.

## Keyring/Secret Storage

### Requirements

**Either** a Secret Service provider:
- gnome-keyring (GNOME)
- KDE Wallet (KDE)
- Any Secret Service API provider

**or nothing at all.** Since v0.3.0 the daemon falls back to the Linux kernel keyutils
keyring, which needs no D-Bus session and no desktop. Credentials there **do not survive a
reboot**, which is the trade-off; the daemon says which store it opened at startup and warns
when it is the kernel one.

Set `credential_store` in `daemon.toml` to force the choice: `auto` (default),
`secret-service`, `keyutils` or `none`.

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

See [SYSTEMD.md](SYSTEMD.md) and the "Server and Headless Environments" section of the
[README](../../README.md#server-and-headless-environments) for detailed instructions.

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

The production GUI's validated baseline is GTK 4.22 and libadwaita 1.9 on Bazzite/Fedora 44.
No broader version range is claimed. The obsolete GUI is compiled against the same binding
generation but is not a supported application target.

### Rust Toolchain

**Pinned Rust version:** 1.98.0

The repository pins the exact toolchain in `rust-toolchain.toml`, so CI and every developer
compile with the same rustc rather than whatever `stable` happens to be. The production GUI declares a
crate-level minimum of Rust 1.92 but is validated with the repository's pinned 1.98 toolchain.

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

See the [Installation Guide](../INSTALLATION.md) for per-distribution package names and
setup steps.

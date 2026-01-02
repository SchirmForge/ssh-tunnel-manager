# SSH Tunnel Manager

**Version**: 0.1.9

## Description

A modern GTK4/Libadwaita application for managing SSH tunnels through a background daemon. Provides an intuitive interface for creating, monitoring, and controlling SSH port forwarding connections.

Supports both local daemon connections via Unix socket and remote daemon connections over HTTPS, enabling you to manage tunnels on headless servers from your desktop.

## Features

### New in v0.1.9

- **First-Launch Configuration Wizard**: Automatic daemon configuration detection and setup
- **Remote Daemon Support**: Connect to daemons over HTTPS on other machines
- **Hybrid Profile Mode**: Profiles sent via API while SSH keys stay secure on daemon host

### Core Features

- **Profile Management**: Save and reuse SSH tunnel configurations
- **Real-time Monitoring**: Live status updates for all tunnels via Server-Sent Events
- **Authentication Support**: SSH keys, passwords, keyboard-interactive (2FA)
- **Daemon Architecture**: Background service for reliable tunnel management
- **Interactive Authentication**: Dynamic prompts for passwords and 2FA codes
- **SSH Host Key Verification**: OpenSSH-compatible known_hosts with SHA256 fingerprints
- **Keychain Integration**: Secure password storage in system keyring
- **Modern UI**: Built with GTK4 and Libadwaita for a native GNOME experience

## Components

- **GUI** (`ssh-tunnel-gtk`): This graphical interface
- **Daemon** (`ssh-tunnel-daemon`): Background service managing tunnels
- **CLI** (`ssh-tunnel`): Command-line interface for scripting and automation

## Technology Stack

- **Language**: Rust
- **GUI Framework**: GTK4 + Libadwaita (Gnome)
- **GUI Framework**: Qt6 +  (KDE)
- **SSH Library**: russh
- **HTTP Client**: reqwest with Unix socket and HTTPS support
- **Architecture**: Client-daemon with SSE for real-time updates

## Connection Modes

### Local Mode (Default)
- Unix domain socket communication
- No network exposure
- Profiles loaded from `~/.config/ssh-tunnel-manager/profiles/`

### Remote Mode (v0.1.9)
- HTTPS connection to remote daemon
- TLS certificate fingerprint pinning
- Token-based authentication
- Hybrid profile mode: SSH keys on daemon, configuration via API

## License

Apache-2.0

## Credits

Built with Rust and modern Gnome/KDE technologies.

Developed with assistance from Generative AI.

## Links

- **Documentation**: See Help menu for user guide
- **Project Repository**: https://github.com/SchirmForge/ssh-tunnel-manager
- **Report Issues**: https://github.com/SchirmForge/ssh-tunnel-manager/issues

---

© 2025 SchirmForge

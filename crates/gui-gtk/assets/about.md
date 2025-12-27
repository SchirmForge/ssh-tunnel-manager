# SSH Tunnel Manager

**Version**: 0.1.3

## Description

A modern GTK4/Libadwaita application for managing SSH tunnels through a background daemon. Provides an intuitive interface for creating, monitoring, and controlling SSH port forwarding connections.

## Features

- **Profile Management**: Save and reuse SSH tunnel configurations
- **Real-time Monitoring**: Live status updates for all tunnels
- **Authentication Support**: SSH keys, passwords, and 2FA
- **Daemon Architecture**: Background service for reliable tunnel management
- **Modern UI**: Built with GTK4 and Libadwaita for a native GNOME experience

## Components

- **GUI** (`ssh-tunnel-gui`): This graphical interface
- **Daemon** (`ssh-tunnel-daemon`): Background service managing tunnels
- **CLI** (`ssh-tunnel-cli`): Command-line interface for scripting

## Technology Stack

- **Language**: Rust
- **GUI Framework**: GTK4 + Libadwaita
- **SSH Library**: russh
- **Architecture**: Client-daemon with SSE for real-time updates

## License

This project is open source software.

## Credits

Built with Rust and modern GNOME technologies.

## Links

- Project Repository: [GitHub]
- Issue Tracker: [Report bugs]
- Documentation: [Online docs]

---

© 2025 SSH Tunnel Manager

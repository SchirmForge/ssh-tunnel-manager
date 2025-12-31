# Development Guide

**Version**: v0.1.9
**Last Updated**: 2025-12-31

## Development

### Prerequisites

#### Rust Toolchain
- Rust 1.75+ (install via [rustup](https://rustup.rs/))
- Linux (primary development platform)

#### System Dependencies

**For CLI and Daemon only:**
- No additional system dependencies required

**For GTK GUI:**
- GTK4 (≥4.12)
- libadwaita (≥1.5)
- GLib development files

**Installation on Debian/Ubuntu:**
```bash
sudo apt install libgtk-4-dev libadwaita-1-dev build-essential pkg-config
```

**Installation on Fedora:**
```bash
sudo dnf install gtk4-devel libadwaita-devel gcc pkg-config
```

**Installation on Arch:**
```bash
sudo pacman -S gtk4 libadwaita base-devel
```

**For Qt GUI (Optional, under development):**
- Qt6 base and declarative modules (required for building gui-qt)
- **Ubuntu/Debian:**
  ```bash
  sudo apt install qt6-base-dev qt6-declarative-dev qml6-module-qtquick qml6-module-qtquick-controls qml6-module-qtquick-layouts
  ```
- See [crates/gui-qt/README.md](crates/gui-qt/README.md) for other distributions

### Build

```bash
# Clone the repository
git clone https://github.com/SchirmForge/ssh-tunnel-manager.git
cd ssh-tunnel-manager

# Build CLI and daemon only
cargo build --release --package ssh-tunnel-cli --package ssh-tunnel-daemon

# Build GTK GUI (no Qt6 needed)
cargo build --release --package ssh-tunnel-gui-gtk

# Build Qt GUI (requires Qt6 - see above)
cargo build --release --package ssh-tunnel-gui-qt

# Build everything including both GUIs (requires both GTK4 and Qt6)
cargo build --release
```

### Basic Usage

#### Using the CLI

```bash
# Start the daemon (in one terminal)
./target/release/ssh-tunnel-daemon

# Create a profile (in another terminal)
./target/release/ssh-tunnel add myprofile

# Start the tunnel
./target/release/ssh-tunnel start myprofile

# List all profiles
./target/release/ssh-tunnel list

# Stop the tunnel
./target/release/ssh-tunnel stop myprofile
```

#### Using the GUI

```bash
# Start the daemon (if not already running)
RUST_LOG=info ./target/release/ssh-tunnel-daemon

# Launch the GTK GUI (in another terminal)
./target/release/ssh-tunnel-gtk

# Or launch the Qt GUI (requires Qt6 installation)
./target/release/ssh-tunnel-qt
```

### Run with Debug Logging

```bash
# Daemon
RUST_LOG=debug cargo run --package ssh-tunnel-daemon

# CLI
RUST_LOG=debug cargo run --package ssh-tunnel-cli -- start myprofile
```

### Build Options

```bash
# Debug build (faster compilation, slower runtime)
cargo build --package ssh-tunnel-cli --package ssh-tunnel-daemon

# Release build (optimized)
cargo build --release --package ssh-tunnel-cli --package ssh-tunnel-daemon

# Run tests
cargo test

# Lint
cargo clippy --all-targets

# Format
cargo fmt --all
```

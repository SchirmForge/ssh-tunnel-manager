# Development Guide

**Version**: v0.1.11
**Last Updated**: 2026-08-30

## Where documentation lives

| Directory | Holds | Update it when |
|---|---|---|
| [architecture/](architecture/) | Functional and technical specification, security, system requirements, systemd | You change how something is built |
| [user-stories/](user-stories/) | What the product does, by epic, with status | You change observable behaviour |
| [plans/](plans/) | Development plans for work not yet started | You plan something, or a plan's premises change |
| [releases/](releases/) | Per-release notes | You cut a release |
| [ROADMAP.md](ROADMAP.md) | What is planned and what is not | Priorities change |
| [PROJECT_STATUS.md](PROJECT_STATUS.md) | Implementation snapshot | A subsystem lands or its status changes |
| [KNOWN_ISSUES.md](KNOWN_ISSUES.md) | Defects and limitations | You find, fix or confirm one |
| [CHANGELOG.md](CHANGELOG.md) | Version history | Every user-visible change |

A behavioural change usually touches a user story **and** the changelog. A purely internal
change touches the architecture documents instead.

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
- See [crates/gui-qt/README.md](../crates/gui-qt/README.md) for other distributions

### Build

```bash
# Clone the repository
git clone https://github.com/SchirmForge/ssh-tunnel-manager.git
cd ssh-tunnel-manager

# Build CLI and daemon only (no system dependencies)
cargo build --release --package ssh-tunnel-cli --package ssh-tunnel-daemon

# Build GTK GUI (no Qt6 needed)
cargo build --release --package ssh-tunnel-gui-gtk

# Build everything in the default set: daemon, CLI, common, gui-core, gui-gtk
cargo build --release

# Build the Qt GUI (requires Qt6 - see above)
cargo build --release --package ssh-tunnel-gui-qt
```

**Note on gui-qt**: it is a workspace member but is excluded from `default-members` in the
root `Cargo.toml`, so a bare `cargo build` skips it. It requires Qt6 and does not currently
compile (see [crates/gui-qt/README.md](../crates/gui-qt/README.md)). Build it explicitly
with `-p ssh-tunnel-gui-qt` when working on it.

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

# Lint (must be clean: CI runs this with -D warnings)
cargo clippy --all-targets --all-features -- -D warnings

# Format
cargo fmt --all
```

## Testing

Tests run in two tiers, split by whether a real SSH server is needed.

### Test isolation

Every test runs in a **sandbox**: `XDG_CONFIG_HOME` and `XDG_RUNTIME_DIR` are redirected to
a temporary directory, so profiles, `cli.toml`, `daemon.toml`, `daemon.token`,
`known_hosts`, the daemon socket and its PID file all land there. Your real
`~/.config/ssh-tunnel-manager` and any daemon you have running are never touched.

If you write a test that reaches the filesystem, use the sandbox. Do not call
`PidFileGuard::create()` or the `profile_manager` functions directly in a test without
redirecting the environment first - that is how the old test suite ended up able to delete
a running daemon's PID file.

### Tier 1: hermetic - runs everywhere, always

```bash
make test          # or: cargo test
```

No network access and no credentials. `crates/daemon/tests/daemon_api.rs` spawns a real
daemon per test and drives its REST/SSE API through the client in `crates/common`.

```bash
make test-network-modes    # CLI end-to-end against all three listener modes, incl. TLS pinning
```

### Tier 2: live SSH - needs a real server

These cover the authentication flows, which cannot be exercised any other way. They are
`#[ignore]`d, so `cargo test` never runs them.

**Setup** - copy the template and fill it in:

```bash
mkdir -p .local/testing
cp docs/testing/ssh-target.env.template .local/testing/ssh-target.env
chmod 600 .local/testing/ssh-target.env
$EDITOR .local/testing/ssh-target.env
```

`.local/` is gitignored in its entirety. **Host names, user names, passwords, TOTP secrets
and private keys belong only there** - never in a committed file, script, test or CI
workflow. The tests read the target from that file; nothing is hardcoded.

The file describes three accounts on the target server: key-only, password-only, and
publickey + keyboard-interactive 2FA. Supplying the TOTP secret lets the tests generate
valid codes, so 2FA is covered without a human.

```bash
make test-live     # or: cargo test -- --ignored --nocapture
```

`--nocapture` matters: an unconfigured live test **skips but still reports `ok`**, and the
reason is only printed to stderr. If you do not see connections happening, read the SKIP
lines.

### Manual testing and the dev sandbox

The GTK GUI is not automated. `scripts/dev-env.sh` gives it a one-command environment:

```bash
make sandbox ARGS="--mode unix-socket --profiles key,password,2fa"
```

That builds debug binaries, starts a daemon, wires `cli.toml` to its generated token, seeds
a profile per auth type from `.local/testing/ssh-target.env`, and drops you into a shell
with `ssh-tunnel` and `ssh-tunnel-gtk` on `PATH` pointing at the sandbox:

```bash
ssh-tunnel list
ssh-tunnel start test-2fa
ssh-tunnel-gtk
```

Exit the shell to stop the daemon and delete the sandbox. Without the target env file it
still opens an empty sandbox and tells you what is missing.

### Before pushing

```bash
make check         # fmt-check, clippy -D warnings, tests
```

# Development Guide

**Version**: v0.6.0
**Last Updated**: 2026-09-03

## Where documentation lives

| Directory | Holds | Update it when |
|---|---|---|
| [architecture/](architecture/) | Functional and technical specification, security, system requirements, systemd | You change how something is built |
| [user-stories/](user-stories/) | What the product does, by epic, with status | You change observable behaviour |
| [releases/](releases/) | Per-release notes | You cut a release |
| [ROADMAP.md](ROADMAP.md) | What is planned and what is not | Priorities change |
| [PROJECT_STATUS.md](PROJECT_STATUS.md) | Implementation snapshot | A subsystem lands or its status changes |
| [KNOWN_ISSUES.md](KNOWN_ISSUES.md) | Defects and limitations | You find, fix or confirm one |
| [CHANGELOG.md](CHANGELOG.md) | Version history | Every user-visible change |

A behavioural change usually touches a user story **and** the changelog. A purely internal
change touches the architecture documents instead.

Development plans are local scratch material and are not part of the published repository.
Once a plan is acted on, what survives belongs in [ROADMAP.md](ROADMAP.md) if it is a
decision, or in the architecture documents if it is a design.

## Development

### Prerequisites

#### Rust Toolchain
- Pinned by [`rust-toolchain.toml`](../rust-toolchain.toml); rustup installs it automatically
  on the first `cargo` command. Do not rely on whatever `stable` happens to be.
- Linux (primary development platform)

#### The dev container (recommended)

**A bare workstation cannot build this project.** `aws-lc-sys`, pulled in by both `rustls`
and `russh`, compiles C and needs `cmake` plus a C toolchain. This is easy to miss: commands
that never compile — `cargo metadata`, `cargo fmt`, `cargo update --dry-run` — all succeed
without it, so the first sign of trouble is a failed build.

[`containers/Containerfile.dev`](../containers/Containerfile.dev) defines an environment with
everything, and CI installs the same set:

```bash
podman build -t ssh-tunnel-builder -f containers/Containerfile.dev containers/

# distrobox
distrobox create --name ssh-tunnel-builder --image ssh-tunnel-builder
distrobox enter ssh-tunnel-builder

# or Fedora toolbox
toolbox create --image ssh-tunnel-builder ssh-tunnel-builder
toolbox run -c ssh-tunnel-builder cargo build
```

Both share your home directory, so `~/.cargo` and `rust-toolchain.toml` supply the toolchain;
the image deliberately does not bake one in.

If you already had a container before this file existed, rebuild it — or install what it is
missing, most likely `openssh-server`, which `make test-live-fixture` needs to start its
local sshd.

#### System Dependencies

Only needed if you are building on the host rather than in the container.

**For CLI and Daemon:**
- `cmake`, a C compiler and `pkg-config` (for `aws-lc-sys`)

**For the production GTK GUI:**
- GTK 4.22
- libadwaita 1.9
- GLib development files

**Installation on Fedora:**
```bash
sudo dnf install gtk4-devel libadwaita-devel gcc pkg-config
```


**For the production GUI:** use the Fedora 44 development container. It intentionally targets GTK 4.22,
libadwaita 1.9, GLib 2.88, and Rust 1.98 as supplied by the current Bazzite/Fedora 44 system;
other distributions are not a compatibility target yet.

### Build

```bash
# Clone the repository
git clone https://github.com/SchirmForge/ssh-tunnel-manager.git
cd ssh-tunnel-manager

# Build CLI and daemon only (no system dependencies)
cargo build --release --package ssh-tunnel-cli --package ssh-tunnel-daemon

# Build the production GTK GUI
cargo build --release --package ssh-tunnel-gui

# Build the default workspace members, including the production GUI
cargo build --release

# Check the obsolete GUI explicitly (not a default member)
cargo check --package ssh-tunnel-gui-gtk --locked
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
./target/release/ssh-tunnel-gui
```

#### Using the production GUI

The GUI validates `cli.toml` before constructing its runtime. With no valid file it offers
daemon-snippet import or manual Unix socket/HTTP/HTTPS setup, then starts the runtime after a
validated atomic save. Build it in the Fedora 44 environment, then run:

```bash
cargo build --package ssh-tunnel-gui --locked
./target/debug/ssh-tunnel-gui
```

The production application ID is `io.github.schirmforge.SshTunnelManager`.

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

### Tier 2: live SSH against a local fixture - no setup

These cover the authentication flows, which cannot be exercised any other way. They are
`#[ignore]`d, so `cargo test` never runs them.

The quickest way to run them needs no server, no credentials and no root: `sshd` runs
perfectly well as a normal user on a high port with its own config and host key.

```bash
make test-live-fixture
```

That starts an unprivileged sshd on localhost, runs the live tier against it, and tears it
down. It covers the six key-based tests, **including both host key verification tests**.
Password and 2FA go through PAM, which must read the shadow database and therefore needs
privilege; those four skip here and are covered by tier 4.

Drive the fixture directly with `scripts/ssh-fixture.sh up|down|status` when debugging.

The fixture writes the same gitignored private target configuration used by tier 4. If a
provisioned target is already configured, `up` moves it aside and `down` restores it. The
fixture used to overwrite that configuration, which silently destroyed credentials and left
tier 4 skipping every test afterwards while still reporting `ok`.

This is what CI runs on every pull request, so the SSH client path is exercised on every
change rather than on demand.

### Tier 4: live SSH against a real server

For the full ten, including password and 2FA, you need a real target with three accounts.
[`scripts/provision-test-target.sh`](../scripts/provision-test-target.sh) builds one:

```bash
scripts/provision-test-target.sh --host <host> --user <admin-user> --key <ssh-key>
make test-live
```

It creates the accounts, generates every key and secret, configures sshd and PAM, and writes
the private test-target configuration consumed by the live tests. It is idempotent, takes the
host as an argument and hardcodes nothing. Every `sshd_config` change is scoped with `Match User` to the test
accounts and validated with `sshd -t` before reload, so it cannot lock you out of the host.

For hand setup, start with
[`docs/testing/ssh-target.env.template`](testing/ssh-target.env.template) and use the private
configuration location reported by `make test-live` when no target is configured. Protect the
result with mode `0600`. **Host names, user names, passwords, TOTP secrets, and private keys
must remain in that gitignored configuration**—never in a committed file, script, test, or CI
workflow. Nothing is hardcoded.

The file describes three accounts on the target server: key-only, password-only, and
password + keyboard-interactive 2FA. Supplying the TOTP secret lets the tests generate
valid codes, so 2FA is covered without a human.

```bash
make test-live     # or: cargo test -- --ignored --nocapture
```

`--nocapture` matters: an unconfigured live test **skips but still reports `ok`**, and the
reason is only printed to stderr. If you do not see connections happening, read the SKIP
lines.

`SSH_TUNNEL_TEST_STRICT` closes that trap. `=1` makes a missing target file a failure (tier
2); `=all` additionally requires every account a test asks for (tier 4). **Both `make` targets
now set it**, so a missing or incomplete configuration fails loudly instead of producing a
green run that tested nothing. Invoking `cargo test -- --ignored` by hand still skips silently
unless you set the variable yourself.

### Manual testing and the dev sandbox

The GTK GUI is not automated. `scripts/dev-env.sh` gives it a one-command environment:

```bash
make sandbox ARGS="--mode unix-socket --profiles key,password,2fa"
```

That builds debug binaries, starts a daemon, wires `cli.toml` to its generated token, seeds
a profile per auth type from the private test-target configuration, and drops you into a shell
with `ssh-tunnel` and `ssh-tunnel-gui` on `PATH` pointing at the sandbox:

```bash
ssh-tunnel list
ssh-tunnel start test-2fa
ssh-tunnel-gui
```

Exit the shell to stop the daemon and delete the sandbox. Without the target env file it
still opens an empty sandbox and tells you what is missing.

The production GUI is wired into `scripts/dev-env.sh`, the root Makefile, and Fedora 44 CI.
Its automated gates are:

```bash
cargo fmt --all -- --check
cargo test --package ssh-tunnel-gui --locked
cargo clippy --package ssh-tunnel-gui --locked --all-targets -- -D warnings
cargo build --package ssh-tunnel-gui --release --locked
cargo check --package ssh-tunnel-gui-gtk --locked
cargo deny check
```

Phase 7 validation and cutover evidence is recorded in
[`crates/gui-v2/PHASE7_VALIDATION.md`](../crates/gui-v2/PHASE7_VALIDATION.md). Keyboard-only and
screen-comparison follow-up is listed in [KNOWN_ISSUES.md](KNOWN_ISSUES.md).

### Before pushing

```bash
make check         # fmt-check, clippy -D warnings, tests
make check-all     # the above plus the live fixture tier and the audit
```

### Testing credential storage

```bash
make test-keychain-live
```

Runs the credential-store tests against a **real** Secret Service, inside a throwaway
`dbus-run-session` with its own `gnome-keyring-daemon`. Nothing touches your own keyring: the
session, the keyring and every entry are created and discarded.

The unit tests in `crates/common/src/keychain.rs` use an in-memory fake and prove the logic.
These prove the *backend*, and are the only thing that would catch a dependency upgrade
quietly changing which store credentials land in — which is what happened in v0.2.0, with the
whole suite green.

Needs `gnome-keyring` and `dbus-run-session`; both are in `containers/Containerfile.dev`.

### Auditing dependencies

```bash
make audit-tools   # once per environment: installs cargo-deny, machete, nextest
make audit         # advisories, licences, dependency sources, unused dependencies
```

`cargo deny check` is the gate and **fails CI** on a new advisory, a disallowed licence, or
a dependency from an unapproved source. It reads the same RustSec database as `cargo audit`
and, unlike it, honours the ignore list in `deny.toml`; running both would mean two ignore
lists drifting apart, so there is one. Use `cargo audit` directly for an ad hoc look.

`cargo machete` is reported but does not fail the build. The v0.4.0 GUI-core dependency
cleanup resolved its findings; obsolete `gui-gtk` is frozen, so any remaining report there is
documented rather than turned into maintenance work.

Policy lives in [`deny.toml`](../deny.toml). Every ignored advisory there carries a written
reason and the condition under which it should be revisited — an ignore without one hides
the next real finding.

The `sources` check bans git dependencies. That is deliberate: a git dependency has no
semver contract, and `cargo audit` matches crates.io name and version, so it cannot see one
at all. A clean advisory report only means something if every dependency is one the scanner
can actually read.

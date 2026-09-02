# Makefile for development tasks and basic binary installation
# For full installation with systemd support, use: ./scripts/install.sh

.PHONY: all build build-debug clean test test-live test-live-fixture test-keychain-live \
        test-network-modes \
        ssh-fixture-up ssh-fixture-down sandbox clippy fmt fmt-check run-daemon run-cli \
        run-gui check check-all audit audit-tools install help


# Default target
all: build

# Build all components
build:
	cargo build --release

# Build in debug mode
build-debug:
	cargo build

# Clean build artifacts
clean:
	cargo clean

# Live SSH tests are #[ignore]d and excluded here - see test-live.
test:
	cargo test

# Live SSH tests against the host in .local/testing/ssh-target.env.
# Without that file each test skips; --nocapture makes the skip reasons visible.
# See docs/testing/ssh-target.env.template.
# Tier-3/4 live tests against a provisioned host (.local/testing/ssh-target.env).
# STRICT=1 is not optional: without it a missing or incomplete target config makes every
# test skip itself and the suite still reports "ok", which is indistinguishable from a
# passing run. See scripts/provision-test-target.sh to create the config.
test-live:
	SSH_TUNNEL_TEST_STRICT=all cargo test -- --ignored --nocapture

# Tier-2 live tests: an unprivileged sshd on localhost. No root, no container,
# no secrets, nothing to install. Covers the key-based half of the live tier;
# password and 2FA need PAM and so run against a provisioned host (test-live).
# STRICT=1 makes a missing fixture fail instead of skipping every test.
test-live-fixture:
	./scripts/ssh-fixture.sh up
	SSH_TUNNEL_TEST_STRICT=1 cargo test -p ssh-tunnel-daemon --test live_ssh -- --ignored --nocapture; \
		status=$$?; ./scripts/ssh-fixture.sh down; exit $$status

ssh-fixture-up:
	./scripts/ssh-fixture.sh up

ssh-fixture-down:
	./scripts/ssh-fixture.sh down

# Keychain tests against the real OS credential store, in a throwaway session.
# The unit tests use an in-memory fake and prove the logic; these prove the backend,
# and are the only thing that would catch a dependency upgrade silently changing which
# store credentials land in - which is what happened in v0.2.0.
#
# Needs dbus-run-session and gnome-keyring-daemon. Nothing touches your real keyring:
# the session, the keyring and every entry are created and discarded here.
test-keychain-live:
	@command -v dbus-run-session >/dev/null || { echo 'need dbus-run-session (dbus-daemon package)'; exit 1; }
	@command -v gnome-keyring-daemon >/dev/null || { echo 'need gnome-keyring-daemon'; exit 1; }
	dbus-run-session -- sh -c '\
		echo -n "test-password" | gnome-keyring-daemon --unlock --replace --daemonize >/dev/null 2>&1; \
		cargo test -p ssh-tunnel-common --test keychain_live -- --ignored --nocapture \
			--skip helper_reads_back'

# End-to-end CLI test of all three daemon listener modes, in a sandbox
test-network-modes:
	./scripts/test-network-modes.sh

# Disposable sandbox shell with a daemon and seeded profiles
# e.g. make sandbox ARGS="--mode tcp-https --profiles key,2fa"
sandbox:
	./scripts/dev-env.sh $(ARGS)

# Run clippy linter
clippy:
	cargo clippy --all-targets --all-features -- -D warnings

# Format code
fmt:
	cargo fmt --all

# Check formatting
fmt-check:
	cargo fmt --all -- --check

# Run daemon in debug mode
run-daemon:
	RUST_LOG=debug cargo run -p ssh-tunnel-daemon

# Run CLI
run-cli:
	cargo run -p ssh-tunnel-cli -- $(ARGS)

# Run the production GTK GUI
run-gui:
	cargo run -p ssh-tunnel-gui

# Supply-chain and dependency audit. Policy lives in deny.toml; every ignored
# advisory there carries a written reason.
# `cargo deny check` covers advisories, licences, dependency sources and
# duplicates, and reads its policy from deny.toml. It uses the same RustSec
# database as `cargo audit`; running both would mean two ignore lists to keep in
# step, so this is the gate. Run `cargo audit` directly for an ad hoc look.
#
# `cargo machete` is reported but does not fail the build: gui-core and gui-gtk
# still declare dependencies they no longer use, and that is GUI work.
audit:
	cargo deny check
	@echo
	@echo 'unused dependencies (report only):'
	@cargo machete 2>/dev/null | sed -n '/found the following/,/^$$/p' || echo '  none'

# Install the audit tooling (slow; only needed once per environment)
audit-tools:
	cargo install --locked cargo-audit cargo-deny cargo-machete cargo-nextest

# Full check (format, clippy, test)
check: fmt-check clippy test

# Everything CI gates on, in one command
check-all: fmt-check clippy test test-live-fixture audit

# Install binaries and optionally systemd units
# NOTE: Use scripts/install.sh for full installation with systemd support
install:
	@echo "============================================================"
	@echo "For full installation with systemd support, use:"
	@echo "  ./scripts/install.sh --user-unit --enable"
	@echo ""
	@echo "Or for system-wide installation:"
	@echo "  sudo ./scripts/install.sh --system-unit --instance tunneld --enable"
	@echo ""
	@echo "Run './scripts/install.sh --help' for more options."
	@echo "============================================================"
	@echo ""
	@echo "Installing binaries to ~/.local/bin (no systemd)..."
	@cargo build --release
	@install -Dm755 target/release/ssh-tunnel-daemon ~/.local/bin/ssh-tunnel-daemon
	@install -Dm755 target/release/ssh-tunnel ~/.local/bin/ssh-tunnel
	@install -Dm755 target/release/ssh-tunnel-gui ~/.local/bin/ssh-tunnel-gui
	@install -Dm644 crates/gui-v2/data/io.github.schirmforge.SshTunnelManager.desktop \
		~/.local/share/applications/io.github.schirmforge.SshTunnelManager.desktop
	@echo "Installed to ~/.local/bin/"
	@echo "Make sure ~/.local/bin is in your PATH"

# Show available targets
help:
	@echo "Available targets:"
	@echo "  build          - Build all components in release mode"
	@echo "  build-debug    - Build all components in debug mode"
	@echo "  clean          - Clean build artifacts"
	@echo "  test           - Run tests (hermetic; no network, no secrets)"
	@echo "  test-live      - Live SSH tests vs a provisioned host (see scripts/provision-test-target.sh)"
	@echo "  test-live-fixture - Live SSH tests vs a local unprivileged sshd (no setup needed)"
	@echo "  test-keychain-live - Keychain tests vs a real credential store (throwaway session)"
	@echo "  test-network-modes - End-to-end CLI test of all three listener modes"
	@echo "  sandbox        - Disposable dev environment (ARGS='--profiles key,2fa')"
	@echo "  clippy         - Run clippy linter"
	@echo "  fmt            - Format code"
	@echo "  fmt-check      - Check code formatting"
	@echo "  check          - Run all checks (format, clippy, test)"
	@echo "  check-all      - Everything CI gates on (adds live fixture tests and audit)"
	@echo "  audit          - cargo audit + cargo deny + cargo machete"
	@echo "  audit-tools    - Install the audit tooling (once per environment)"
	@echo "  run-daemon     - Run daemon in debug mode"
	@echo "  run-cli        - Run CLI (use ARGS='your args' to pass arguments)"
	@echo "  run-gui        - Run the production GTK GUI (ssh-tunnel-gui)"
	@echo "  install        - Install binaries to ~/.local/bin (use scripts/install.sh for systemd)"
	@echo "  help           - Show this help message"
	@echo ""
	@echo "For full installation with systemd support:"
	@echo "  ./scripts/install.sh --help"

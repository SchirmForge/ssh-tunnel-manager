# Makefile for development tasks and basic binary installation
# For full installation with systemd support, use: ./scripts/install.sh

.PHONY: all build build-debug clean test test-live test-network-modes sandbox \
        clippy fmt fmt-check run-daemon run-cli run-gui check install help


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
test-live:
	cargo test -- --ignored --nocapture

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

# Run GTK GUI
run-gui:
	cargo run -p ssh-tunnel-gui-gtk

# Full check (format, clippy, test)
check: fmt-check clippy test

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
	@install -Dm755 target/release/ssh-tunnel-gtk ~/.local/bin/ssh-tunnel-gtk
	@echo "Installed to ~/.local/bin/"
	@echo "Make sure ~/.local/bin is in your PATH"

# Show available targets
help:
	@echo "Available targets:"
	@echo "  build          - Build all components in release mode"
	@echo "  build-debug    - Build all components in debug mode"
	@echo "  clean          - Clean build artifacts"
	@echo "  test           - Run tests (hermetic; no network, no secrets)"
	@echo "  test-live      - Run live SSH tests (needs .local/testing/ssh-target.env)"
	@echo "  test-network-modes - End-to-end CLI test of all three listener modes"
	@echo "  sandbox        - Disposable dev environment (ARGS='--profiles key,2fa')"
	@echo "  clippy         - Run clippy linter"
	@echo "  fmt            - Format code"
	@echo "  fmt-check      - Check code formatting"
	@echo "  check          - Run all checks (format, clippy, test)"
	@echo "  run-daemon     - Run daemon in debug mode"
	@echo "  run-cli        - Run CLI (use ARGS='your args' to pass arguments)"
	@echo "  run-gui        - Run the GTK GUI (ssh-tunnel-gtk)"
	@echo "  install        - Install binaries to ~/.local/bin (use scripts/install.sh for systemd)"
	@echo "  help           - Show this help message"
	@echo ""
	@echo "For full installation with systemd support:"
	@echo "  ./scripts/install.sh --help"

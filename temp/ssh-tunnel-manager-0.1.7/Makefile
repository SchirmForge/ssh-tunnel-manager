# Makefile for development tasks and basic binary installation
# For full installation with systemd support, use: ./scripts/install.sh

.PHONY: all build clean test clippy fmt run-daemon run-cli run-gui check install help


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

# Run tests
test:
	cargo test --all

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

# Run GUI
run-gui:
	cargo run -p ssh-tunnel-gui

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
	@install -Dm755 target/release/ssh-tunnel-gui ~/.local/bin/ssh-tunnel-gui
	@echo "Installed to ~/.local/bin/"
	@echo "Make sure ~/.local/bin is in your PATH"

# Show available targets
help:
	@echo "Available targets:"
	@echo "  build          - Build all components in release mode"
	@echo "  build-debug    - Build all components in debug mode"
	@echo "  clean          - Clean build artifacts"
	@echo "  test           - Run tests"
	@echo "  clippy         - Run clippy linter"
	@echo "  fmt            - Format code"
	@echo "  fmt-check      - Check code formatting"
	@echo "  check          - Run all checks (format, clippy, test)"
	@echo "  run-daemon     - Run daemon in debug mode"
	@echo "  run-cli        - Run CLI (use ARGS='your args' to pass arguments)"
	@echo "  run-gui        - Run GUI application"
	@echo "  install        - Install binaries to ~/.local/bin (use scripts/install.sh for systemd)"
	@echo "  help           - Show this help message"
	@echo ""
	@echo "For full installation with systemd support:"
	@echo "  ./scripts/install.sh --help"

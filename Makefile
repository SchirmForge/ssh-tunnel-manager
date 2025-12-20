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

# Install binaries to ~/.local/bin
install: build
	install -Dm755 target/release/ssh-tunnel-daemon ~/.local/bin/ssh-tunnel-daemon
	install -Dm755 target/release/ssh-tunnel-cli ~/.local/bin/ssh-tunnel
	install -Dm755 target/release/ssh-tunnel-gui ~/.local/bin/ssh-tunnel-gui
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
	@echo "  install        - Install binaries to ~/.local/bin"
	@echo "  help           - Show this help message"

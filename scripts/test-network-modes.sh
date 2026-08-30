#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2025 SSH Tunnel Manager Contributors
#
# SSH Tunnel Manager - Network Modes Test Script
# Tests UnixSocket, TcpHttp, and TcpHttps modes end to end through the CLI binary.
#
# Runs entirely inside a throwaway sandbox (see scripts/sandbox.sh): the
# developer's ~/.config/ssh-tunnel-manager and any running daemon are never
# touched. Complements crates/daemon/tests/daemon_api.rs, which drives the same
# daemon through the REST API rather than the CLI.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# shellcheck source=scripts/sandbox.sh
source "$SCRIPT_DIR/sandbox.sh"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

DAEMON_BIN="$REPO_ROOT/target/debug/ssh-tunnel-daemon"
CLI_BIN="$REPO_ROOT/target/debug/ssh-tunnel"

DAEMON_PID=""
FAILURES=0

print_header() { echo -e "\n${BLUE}========================================${NC}\n${BLUE}$1${NC}\n${BLUE}========================================${NC}\n"; }
print_success() { echo -e "${GREEN}✓ $1${NC}"; }
print_error() { echo -e "${RED}✗ $1${NC}"; FAILURES=$((FAILURES + 1)); }
print_info() { echo -e "${YELLOW}ℹ $1${NC}"; }

stop_daemon() {
    if [ -n "$DAEMON_PID" ]; then
        kill "$DAEMON_PID" 2>/dev/null || true
        wait "$DAEMON_PID" 2>/dev/null || true
        DAEMON_PID=""
    fi
}

cleanup() {
    stop_daemon
    sandbox_cleanup
}
trap cleanup EXIT INT TERM

# Start a fresh sandbox for one test case.
begin_case() {
    stop_daemon
    sandbox_cleanup
    sandbox_init >/dev/null
    sandbox_assert_isolated || exit 1
    CONFIG_DIR="$(sandbox_config_dir)"
    DAEMON_LOG="$SSH_TUNNEL_SANDBOX/daemon.log"
}

start_daemon() {
    "$DAEMON_BIN" > "$DAEMON_LOG" 2>&1 &
    DAEMON_PID=$!
}

# Run the CLI against the sandbox daemon; `watch` exits via timeout on success.
cli_can_connect() {
    timeout 3 "$CLI_BIN" watch >/dev/null 2>&1
    # timeout kills a working connection with 124; a real failure exits sooner.
    [ $? -eq 124 ]
}

build_project() {
    print_header "Building (debug)"
    (cd "$REPO_ROOT" && cargo build --package ssh-tunnel-daemon --package ssh-tunnel-cli)
    print_success "Build completed"
}

# --- Test 1: UnixSocket mode ------------------------------------------------

test_unix_socket_mode() {
    print_header "TEST 1: UnixSocket Mode (Default)"
    begin_case

    local socket_path="$XDG_RUNTIME_DIR/ssh-tunnel-manager/ssh-tunnel-manager.sock"

    print_info "Starting daemon in UnixSocket mode..."
    start_daemon

    if sandbox_wait_for 10 "[ -S '$socket_path' ]"; then
        print_success "Unix socket created inside the sandbox"
    else
        print_error "Unix socket not created"
        cat "$DAEMON_LOG"
        return
    fi

    local perms
    perms=$(stat -c "%a" "$socket_path")
    if [ "$perms" = "600" ]; then
        print_success "Socket has owner-only permissions (600)"
    else
        print_error "Socket has permissions $perms, expected 600"
    fi

    if [ -f "$CONFIG_DIR/daemon.token" ]; then
        print_success "Authentication token generated (required by default)"
    else
        print_error "Authentication token not found"
        cat "$DAEMON_LOG"
        return
    fi

    local token
    token=$(head -n1 "$CONFIG_DIR/daemon.token")
    cat > "$CONFIG_DIR/cli.toml" <<EOF
connection_mode = "unix-socket"
auth_token = "$token"
EOF

    if cli_can_connect; then
        print_success "CLI can connect to daemon"
    else
        print_error "CLI could not connect to daemon"
        cat "$DAEMON_LOG"
    fi
}

# --- Test 2: TcpHttp mode ---------------------------------------------------

test_tcp_http_mode() {
    print_header "TEST 2: TcpHttp Mode"
    begin_case

    local port=3443
    cat > "$CONFIG_DIR/daemon.toml" <<EOF
listener_mode = "tcp-http"
bind_host = "127.0.0.1"
bind_port = $port
require_auth = true
EOF

    print_info "Starting daemon in TcpHttp mode..."
    start_daemon

    if sandbox_wait_for_port 10 127.0.0.1 "$port"; then
        print_success "Daemon listening on 127.0.0.1:$port"
    else
        print_error "Daemon not listening on port $port"
        cat "$DAEMON_LOG"
        return
    fi

    local token
    token=$(head -n1 "$CONFIG_DIR/daemon.token" 2>/dev/null || true)
    if [ -z "$token" ]; then
        print_error "Failed to read authentication token"
        cat "$DAEMON_LOG"
        return
    fi
    print_success "Authentication token generated: ${token:0:8}..."

    write_http_cli_config() {
        cat > "$CONFIG_DIR/cli.toml" <<EOF
connection_mode = "http"
daemon_host = "127.0.0.1"
daemon_port = $port
auth_token = "$1"
EOF
    }

    print_info "Testing CLI without authentication token..."
    write_http_cli_config ""
    if cli_can_connect; then
        print_error "CLI connected without token (should have failed)"
    else
        print_success "CLI correctly rejected without token"
    fi

    print_info "Testing CLI with correct authentication token..."
    write_http_cli_config "$token"
    if cli_can_connect; then
        print_success "CLI connected with correct token"
    else
        print_error "CLI could not connect with the correct token"
        cat "$DAEMON_LOG"
    fi

    print_info "Testing CLI with wrong authentication token..."
    write_http_cli_config "wrong-token-12345"
    if cli_can_connect; then
        print_error "CLI connected with wrong token (should have failed)"
    else
        print_success "CLI correctly rejected with wrong token"
    fi
}

# --- Test 3: TcpHttps mode --------------------------------------------------

test_tcp_https_mode() {
    print_header "TEST 3: TcpHttps Mode"
    begin_case

    local port=3443
    cat > "$CONFIG_DIR/daemon.toml" <<EOF
listener_mode = "tcp-https"
bind_host = "127.0.0.1"
bind_port = $port
require_auth = true
EOF

    print_info "Starting daemon in TcpHttps mode..."
    start_daemon

    if sandbox_wait_for_port 15 127.0.0.1 "$port"; then
        print_success "Daemon listening on 127.0.0.1:$port"
    else
        print_error "Daemon not listening on port $port"
        cat "$DAEMON_LOG"
        return
    fi

    if [ -f "$CONFIG_DIR/server.crt" ] && [ -f "$CONFIG_DIR/server.key" ]; then
        print_success "TLS certificate generated"
    else
        print_error "TLS certificate not generated"
        cat "$DAEMON_LOG"
        return
    fi

    local fingerprint
    fingerprint=$(grep "Certificate fingerprint" "$DAEMON_LOG" | awk -F ': ' '{print $2}' | head -1)
    if [ -z "$fingerprint" ]; then
        print_error "Failed to extract certificate fingerprint"
        cat "$DAEMON_LOG"
        return
    fi
    print_success "Certificate fingerprint: $fingerprint"

    local token
    token=$(head -n1 "$CONFIG_DIR/daemon.token" 2>/dev/null || true)
    if [ -z "$token" ]; then
        print_error "Failed to read authentication token"
        return
    fi

    write_https_cli_config() {
        cat > "$CONFIG_DIR/cli.toml" <<EOF
connection_mode = "https"
daemon_host = "127.0.0.1"
daemon_port = $port
auth_token = "$token"
tls_cert_fingerprint = "$1"
EOF
    }

    # Without a pinned fingerprint the client validates against webpki roots
    # (see create_insecure_tls_config), so the daemon's self-signed certificate
    # is correctly refused. Pinning is what makes a self-signed daemon usable.
    print_info "Testing HTTPS without certificate pinning..."
    write_https_cli_config ""
    if cli_can_connect; then
        print_error "CLI accepted a self-signed certificate without pinning"
    else
        print_success "CLI correctly rejected the self-signed cert without pinning"
    fi

    print_info "Testing HTTPS with certificate pinning..."
    write_https_cli_config "$fingerprint"
    if cli_can_connect; then
        print_success "CLI connected via HTTPS with certificate pinning"
    else
        print_error "CLI could not connect with the correct fingerprint"
        cat "$DAEMON_LOG"
    fi

    print_info "Testing HTTPS with wrong certificate fingerprint..."
    write_https_cli_config "AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99"
    if cli_can_connect; then
        print_error "CLI connected with wrong fingerprint (should have failed)"
    else
        print_success "CLI correctly rejected with wrong fingerprint"
    fi

    print_info "Checking file permissions..."
    local key_perms token_perms
    key_perms=$(stat -c "%a" "$CONFIG_DIR/server.key" 2>/dev/null || echo unknown)
    token_perms=$(stat -c "%a" "$CONFIG_DIR/daemon.token" 2>/dev/null || echo unknown)

    if [ "$key_perms" = "600" ]; then
        print_success "Private key has correct permissions (600)"
    else
        print_error "Private key has permissions $key_perms, expected 600"
    fi

    if [ "$token_perms" = "600" ]; then
        print_success "Token file has correct permissions (600)"
    else
        print_error "Token file has permissions $token_perms, expected 600"
    fi
}

# --- Main -------------------------------------------------------------------

main() {
    print_header "SSH Tunnel Manager - Network Modes Test Suite"
    print_info "Each case runs in its own sandbox; your real config is not touched."

    build_project

    test_unix_socket_mode
    test_tcp_http_mode
    test_tcp_https_mode

    print_header "Summary"
    if [ "$FAILURES" -eq 0 ]; then
        print_success "All network mode tests passed"
        exit 0
    fi
    print_error "$FAILURES check(s) failed"
    exit 1
}

main "$@"

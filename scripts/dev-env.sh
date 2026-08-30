#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2025 SSH Tunnel Manager Contributors
#
# Disposable development environment for hand-testing features.
#
# Builds debug binaries, opens a sandbox (scripts/sandbox.sh), starts a daemon,
# wires cli.toml to the generated token, optionally seeds profiles against the
# test SSH host, and drops you into a shell where `ssh-tunnel` and
# `ssh-tunnel-gtk` already point at the sandbox. Exiting tears it all down.
#
# Your real ~/.config/ssh-tunnel-manager and any running daemon are untouched.
#
# Usage:
#   ./scripts/dev-env.sh                                   # unix-socket, no profiles
#   ./scripts/dev-env.sh --mode tcp-https
#   ./scripts/dev-env.sh --profiles key,password,2fa
#   ./scripts/dev-env.sh --no-daemon                       # sandbox shell only
#
# Seeded profiles read the target host and accounts from
# .local/testing/ssh-target.env (gitignored). Without that file the sandbox
# still opens; it just says what is missing. See
# docs/testing/ssh-target.env.template.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# shellcheck source=scripts/sandbox.sh
source "$SCRIPT_DIR/sandbox.sh"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

MODE="unix-socket"
PROFILES=""
START_DAEMON=1
PORT=3443

TARGET_ENV="$REPO_ROOT/.local/testing/ssh-target.env"
DAEMON_PID=""

usage() {
    sed -n '5,25p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --mode) MODE="$2"; shift 2 ;;
        --profiles) PROFILES="$2"; shift 2 ;;
        --port) PORT="$2"; shift 2 ;;
        --no-daemon) START_DAEMON=0; shift ;;
        -h|--help) usage 0 ;;
        *) echo -e "${RED}Unknown option: $1${NC}" >&2; usage 1 ;;
    esac
done

case "$MODE" in
    unix-socket|tcp-http|tcp-https) ;;
    *) echo -e "${RED}--mode must be unix-socket, tcp-http or tcp-https${NC}" >&2; exit 1 ;;
esac

cleanup() {
    if [ -n "$DAEMON_PID" ]; then
        kill "$DAEMON_PID" 2>/dev/null || true
        wait "$DAEMON_PID" 2>/dev/null || true
    fi
    sandbox_cleanup
    echo -e "${YELLOW}Sandbox torn down.${NC}"
}
trap cleanup EXIT INT TERM

echo -e "${BLUE}Building debug binaries...${NC}"
(cd "$REPO_ROOT" && cargo build --package ssh-tunnel-daemon --package ssh-tunnel-cli)

sandbox_init
sandbox_assert_isolated || exit 1
CONFIG_DIR="$(sandbox_config_dir)"
DAEMON_LOG="$SSH_TUNNEL_SANDBOX/daemon.log"

# --- Daemon -----------------------------------------------------------------

if [ "$START_DAEMON" -eq 1 ]; then
    if [ "$MODE" != "unix-socket" ]; then
        cat > "$CONFIG_DIR/daemon.toml" <<EOF
listener_mode = "$MODE"
bind_host = "127.0.0.1"
bind_port = $PORT
require_auth = true
EOF
    fi

    echo -e "${BLUE}Starting daemon ($MODE)...${NC}"
    "$REPO_ROOT/target/debug/ssh-tunnel-daemon" > "$DAEMON_LOG" 2>&1 &
    DAEMON_PID=$!

    if [ "$MODE" = "unix-socket" ]; then
        ready="[ -S '$XDG_RUNTIME_DIR/ssh-tunnel-manager/ssh-tunnel-manager.sock' ]"
        sandbox_wait_for 15 "$ready" || { echo -e "${RED}Daemon failed to start${NC}"; cat "$DAEMON_LOG"; exit 1; }
    else
        sandbox_wait_for_port 15 127.0.0.1 "$PORT" || { echo -e "${RED}Daemon failed to start${NC}"; cat "$DAEMON_LOG"; exit 1; }
    fi

    TOKEN=$(head -n1 "$CONFIG_DIR/daemon.token")

    case "$MODE" in
        unix-socket)
            cat > "$CONFIG_DIR/cli.toml" <<EOF
connection_mode = "unix-socket"
auth_token = "$TOKEN"
EOF
            ;;
        tcp-http)
            cat > "$CONFIG_DIR/cli.toml" <<EOF
connection_mode = "http"
daemon_host = "127.0.0.1"
daemon_port = $PORT
auth_token = "$TOKEN"
EOF
            ;;
        tcp-https)
            FINGERPRINT=$(grep "Certificate fingerprint" "$DAEMON_LOG" | awk -F ': ' '{print $2}' | head -1)
            cat > "$CONFIG_DIR/cli.toml" <<EOF
connection_mode = "https"
daemon_host = "127.0.0.1"
daemon_port = $PORT
auth_token = "$TOKEN"
tls_cert_fingerprint = "$FINGERPRINT"
EOF
            ;;
    esac

    echo -e "${GREEN}✓ Daemon running (pid $DAEMON_PID), CLI configured${NC}"
fi

# --- Seeded profiles --------------------------------------------------------

seed_profile() {
    local name="$1" user="$2" auth_type="$3" key_path="$4" local_port="$5"
    local id
    id=$(cat /proc/sys/kernel/random/uuid)
    local now
    now=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    mkdir -p "$CONFIG_DIR/profiles"
    {
        echo "id = \"$id\""
        echo "name = \"$name\""
        echo "created_at = \"$now\""
        echo "modified_at = \"$now\""
        echo
        echo "[connection]"
        echo "host = \"$SSH_TUNNEL_TEST_HOST\""
        echo "port = ${SSH_TUNNEL_TEST_PORT:-22}"
        echo "user = \"$user\""
        echo "auth_type = \"$auth_type\""
        [ -n "$key_path" ] && echo "key_path = \"$key_path\""
        echo "password_storage = \"none\""
        echo
        echo "[forwarding]"
        echo "type = \"local\""
        echo "bind_address = \"127.0.0.1\""
        echo "local_port = $local_port"
        echo "remote_host = \"${SSH_TUNNEL_TEST_FORWARD_HOST:-127.0.0.1}\""
        echo "remote_port = ${SSH_TUNNEL_TEST_FORWARD_PORT:-22}"
    } > "$CONFIG_DIR/profiles/$id.toml"

    echo -e "${GREEN}✓ Seeded profile '$name'${NC}"
}

if [ -n "$PROFILES" ]; then
    if [ ! -f "$TARGET_ENV" ]; then
        echo -e "${YELLOW}No $TARGET_ENV -- cannot seed profiles.${NC}"
        echo -e "${YELLOW}Copy docs/testing/ssh-target.env.template there and fill it in.${NC}"
    else
        # shellcheck disable=SC1090
        set -a; source "$TARGET_ENV"; set +a

        if [ -z "${SSH_TUNNEL_TEST_HOST:-}" ]; then
            echo -e "${YELLOW}SSH_TUNNEL_TEST_HOST is not set in $TARGET_ENV -- skipping profiles.${NC}"
        else
            port=8022
            IFS=',' read -ra wanted <<< "$PROFILES"
            for kind in "${wanted[@]}"; do
                case "$kind" in
                    key)
                        if [ -n "${SSH_TUNNEL_TEST_USER_KEY:-}" ] && [ -n "${SSH_TUNNEL_TEST_KEY_PATH:-}" ]; then
                            seed_profile "test-key" "$SSH_TUNNEL_TEST_USER_KEY" "key" "$SSH_TUNNEL_TEST_KEY_PATH" "$port"
                        else
                            echo -e "${YELLOW}Skipping 'key': SSH_TUNNEL_TEST_USER_KEY/KEY_PATH not set${NC}"
                        fi
                        ;;
                    password)
                        if [ -n "${SSH_TUNNEL_TEST_USER_PASSWORD:-}" ]; then
                            seed_profile "test-password" "$SSH_TUNNEL_TEST_USER_PASSWORD" "password" "" "$port"
                        else
                            echo -e "${YELLOW}Skipping 'password': SSH_TUNNEL_TEST_USER_PASSWORD not set${NC}"
                        fi
                        ;;
                    2fa)
                        if [ -n "${SSH_TUNNEL_TEST_USER_2FA:-}" ]; then
                            seed_profile "test-2fa" "$SSH_TUNNEL_TEST_USER_2FA" "passwordwith2fa" "" "$port"
                        else
                            echo -e "${YELLOW}Skipping '2fa': SSH_TUNNEL_TEST_USER_2FA not set${NC}"
                        fi
                        ;;
                    *)
                        echo -e "${YELLOW}Unknown profile kind '$kind' (expected key, password or 2fa)${NC}"
                        ;;
                esac
                port=$((port + 1))
            done
        fi
    fi
fi

# --- Interactive shell ------------------------------------------------------

export PATH="$REPO_ROOT/target/debug:$PATH"

cat <<EOF

$(echo -e "${BLUE}========================================${NC}")
$(echo -e "${BLUE}SSH Tunnel Manager development sandbox${NC}")
$(echo -e "${BLUE}========================================${NC}")

  Sandbox     : $SSH_TUNNEL_SANDBOX
  Config      : $CONFIG_DIR
  Daemon mode : $MODE$([ "$START_DAEMON" -eq 1 ] || echo " (not started)")
  Daemon log  : $DAEMON_LOG

  ssh-tunnel and ssh-tunnel-gtk are on PATH and point at this sandbox.

    ssh-tunnel list
    ssh-tunnel start test-key
    ssh-tunnel status --all
    ssh-tunnel-gtk
    tail -f "\$DAEMON_LOG"

  Exit this shell to stop the daemon and delete the sandbox.

EOF

"${SHELL:-/bin/bash}"

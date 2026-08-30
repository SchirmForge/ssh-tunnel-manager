#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2025 SSH Tunnel Manager Contributors
#
# Sandbox helper for SSH Tunnel Manager development and testing.
#
# Every config path in this project resolves through dirs::config_dir(), and the
# daemon socket and PID file through dirs::runtime_dir(). On Linux those honour
# XDG_CONFIG_HOME and XDG_RUNTIME_DIR, so redirecting those two variables gives a
# run its own private world -- profiles, cli.toml, daemon.toml, daemon.token,
# known_hosts, the Unix socket and the PID file -- without touching the
# developer's real ~/.config/ssh-tunnel-manager or a running daemon.
#
# Usage:
#   source scripts/sandbox.sh          # sandbox the current shell
#   sandbox_init                       # create a fresh sandbox, export the vars
#   sandbox_cleanup                    # remove it (also runs on EXIT via trap)
#
# Variables exported by sandbox_init:
#   SSH_TUNNEL_SANDBOX   root of the throwaway tree
#   XDG_CONFIG_HOME      $SSH_TUNNEL_SANDBOX/config
#   XDG_RUNTIME_DIR      $SSH_TUNNEL_SANDBOX/run
#   SSH_TUNNEL_SKIP_KEYRING=1   no Secret Service in CI or headless sessions

# Refuse to run with a config home that is not ours, so a caller can assert
# isolation before doing anything destructive.
sandbox_assert_isolated() {
    if [ -z "${SSH_TUNNEL_SANDBOX:-}" ]; then
        echo "ERROR: not in a sandbox (SSH_TUNNEL_SANDBOX unset)" >&2
        return 1
    fi
    case "${XDG_CONFIG_HOME:-}" in
        "$SSH_TUNNEL_SANDBOX"/*) ;;
        *)
            echo "ERROR: XDG_CONFIG_HOME (${XDG_CONFIG_HOME:-unset}) is outside the sandbox" >&2
            return 1
            ;;
    esac
    return 0
}

sandbox_init() {
    SSH_TUNNEL_SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/ssh-tunnel-sandbox.XXXXXXXX")"
    export SSH_TUNNEL_SANDBOX

    export XDG_CONFIG_HOME="$SSH_TUNNEL_SANDBOX/config"
    export XDG_RUNTIME_DIR="$SSH_TUNNEL_SANDBOX/run"
    export SSH_TUNNEL_SKIP_KEYRING=1

    mkdir -p "$XDG_CONFIG_HOME/ssh-tunnel-manager" "$XDG_RUNTIME_DIR"
    # The daemon expects an XDG_RUNTIME_DIR it alone can use.
    chmod 700 "$XDG_RUNTIME_DIR"

    echo "Sandbox: $SSH_TUNNEL_SANDBOX"
}

sandbox_cleanup() {
    # Only ever remove a directory this script created.
    if [ -n "${SSH_TUNNEL_SANDBOX:-}" ] && [ -d "$SSH_TUNNEL_SANDBOX" ]; then
        case "$SSH_TUNNEL_SANDBOX" in
            */ssh-tunnel-sandbox.*)
                rm -rf "$SSH_TUNNEL_SANDBOX"
                ;;
            *)
                echo "Refusing to remove unexpected sandbox path: $SSH_TUNNEL_SANDBOX" >&2
                ;;
        esac
    fi
}

# The config directory the daemon and clients will use inside the sandbox.
sandbox_config_dir() {
    echo "$XDG_CONFIG_HOME/ssh-tunnel-manager"
}

# Wait until `cmd` succeeds, up to `timeout` seconds. Returns non-zero on timeout.
#   sandbox_wait_for 10 "[ -S '$socket' ]"
sandbox_wait_for() {
    local timeout="$1"
    local check_cmd="$2"
    local waited=0

    while [ "$waited" -lt "$timeout" ]; do
        if eval "$check_cmd" 2>/dev/null; then
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done
    return 1
}

# Is something accepting TCP connections on host:port?
# Uses bash's /dev/tcp so no netcat dependency is needed.
sandbox_port_open() {
    (exec 3<>"/dev/tcp/$1/$2") 2>/dev/null
}

# Wait until host:port accepts connections, up to `timeout` seconds.
sandbox_wait_for_port() {
    local timeout="$1" host="$2" port="$3"
    sandbox_wait_for "$timeout" "sandbox_port_open '$host' '$port'"
}

# When executed rather than sourced, open a sandbox and run the given command
# (default: an interactive shell) inside it.
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    set -euo pipefail
    sandbox_init
    trap sandbox_cleanup EXIT INT TERM

    if [ "$#" -gt 0 ]; then
        "$@"
    else
        echo "Starting a shell in the sandbox. Exit to tear it down."
        "${SHELL:-/bin/bash}"
    fi
fi

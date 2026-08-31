#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright 2025 SSH Tunnel Manager Contributors
#
# Provision a remote host as a tier-4 live SSH test target.
#
# Creates three unprivileged test accounts covering the authentication methods the
# daemon supports, generates the keys, and writes `.local/testing/ssh-target.env`.
#
#   scripts/provision-test-target.sh --host <host> [--user <admin-user>] [--key <path>]
#
# The target host name and every credential this produces stay in `.local/testing/`,
# which is gitignored in its entirety. Nothing here hardcodes them (US-10.4).
#
# Safety: the sshd drop-in only ever uses `Match User` blocks scoped to the three test
# accounts, so the administrative account's access is never altered. The config is
# validated with `sshd -t` before anything is reloaded; if validation fails the drop-in
# is removed and nothing is reloaded.
#
# Idempotent: re-running replaces the accounts' keys and re-writes the config.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TESTING_DIR="$REPO_ROOT/.local/testing"

HOST=""
ADMIN_USER="guapo"
SSH_KEY="$TESTING_DIR/id_dev-schirmForge"
FORWARD_PORT=22

# Test accounts. Names are generic on purpose: they carry no information about the host.
U_KEY="tuntest-key"
U_PW="tuntest-pw"
U_2FA="tuntest-2fa"

usage() { sed -n '4,20p' "$0"; exit "${1:-0}"; }

while [ $# -gt 0 ]; do
    case "$1" in
        --host) HOST="$2"; shift 2 ;;
        --user) ADMIN_USER="$2"; shift 2 ;;
        --key)  SSH_KEY="$2"; shift 2 ;;
        -h|--help) usage 0 ;;
        *) echo "unknown argument: $1" >&2; usage 1 ;;
    esac
done

[ -n "$HOST" ] || { echo "error: --host is required" >&2; usage 1; }
[ -f "$SSH_KEY" ] || { echo "error: ssh key not found: $SSH_KEY" >&2; exit 1; }

KEYS_DIR="$TESTING_DIR/fixture-keys"
ENV_FILE="$TESTING_DIR/ssh-target.env"

say() { printf '\033[0;34m==>\033[0m %s\n' "$1"; }
ok()  { printf '\033[0;32m  ok\033[0m %s\n' "$1"; }

rsh() { ssh -i "$SSH_KEY" -o BatchMode=yes -o ConnectTimeout=15 "$ADMIN_USER@$HOST" "$@"; }

# --- 1. Credentials, generated locally ---------------------------------------

say "Generating credentials into $KEYS_DIR"
mkdir -p "$KEYS_DIR"; chmod 700 "$TESTING_DIR" "$KEYS_DIR"

# `head -c` on the source, not the sink: piping /dev/urandom straight into `head`
# gives `tr` a SIGPIPE, which `set -o pipefail` turns into a spurious failure.
rand_pw()  { head -c 512 /dev/urandom | LC_ALL=C tr -dc 'A-Za-z0-9' | cut -c1-24; }
# RFC 4648 base32, no padding: what an authenticator enrolment string looks like.
rand_b32() { head -c 512 /dev/urandom | LC_ALL=C tr -dc 'A-Z2-7'    | cut -c1-32; }

PW_PASSWORD="$(rand_pw)"
TFA_PASSWORD="$(rand_pw)"
KEY_PASSPHRASE="$(rand_pw)"
TOTP_SECRET="$(rand_b32)"

rm -f "$KEYS_DIR"/id_*
ssh-keygen -q -t ed25519 -f "$KEYS_DIR/id_key"     -N "" -C "ssh-tunnel-test-key"
ssh-keygen -q -t ed25519 -f "$KEYS_DIR/id_enc"     -N "$KEY_PASSPHRASE" -C "ssh-tunnel-test-encrypted"
ssh-keygen -q -t ed25519 -f "$KEYS_DIR/id_2fa"     -N "" -C "ssh-tunnel-test-2fa"
chmod 600 "$KEYS_DIR"/id_*
ok "keys and secrets generated"

# --- 2. Remote provisioning ---------------------------------------------------

say "Provisioning accounts on $HOST"
rsh "sudo -n bash -s" <<REMOTE
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive
if ! dpkg -s libpam-google-authenticator >/dev/null 2>&1; then
    apt-get update -qq
    apt-get install -y -qq libpam-google-authenticator >/dev/null
fi

for u in $U_KEY $U_PW $U_2FA; do
    id -u "\$u" >/dev/null 2>&1 || useradd -m -s /bin/bash "\$u"
    install -d -m 700 -o "\$u" -g "\$u" "/home/\$u/.ssh"
done

echo "$U_PW:$PW_PASSWORD"   | chpasswd
echo "$U_2FA:$TFA_PASSWORD" | chpasswd
# The key-only account must not be reachable by password at all.
passwd -l $U_KEY >/dev/null

# TOTP state for the 2FA account. Written directly so the secret is known to the tests.
cat > /home/$U_2FA/.google_authenticator <<GA
$TOTP_SECRET
" RATE_LIMIT 3 30
" WINDOW_SIZE 17
" TOTP_AUTH
GA
chown $U_2FA:$U_2FA /home/$U_2FA/.google_authenticator
chmod 400 /home/$U_2FA/.google_authenticator
REMOTE
ok "accounts, packages and TOTP state in place"

say "Installing public keys"
# `install /dev/stdin` is not dependable under `sudo` over a non-interactive ssh
# session, so pipe through `tee` and set ownership explicitly.
push_key() {
    local user="$1"; shift
    cat "$@" | rsh "sudo -n bash -c '
        cat > /home/$user/.ssh/authorized_keys &&
        chown $user:$user /home/$user/.ssh/authorized_keys &&
        chmod 600 /home/$user/.ssh/authorized_keys'"
}
# The key account accepts both the plain and the passphrase-protected key.
push_key "$U_KEY" "$KEYS_DIR/id_key.pub" "$KEYS_DIR/id_enc.pub"
push_key "$U_2FA" "$KEYS_DIR/id_2fa.pub"
ok "authorized_keys installed"

# --- 3. sshd + PAM, validated before reload ----------------------------------

say "Writing sshd drop-in and PAM rule (validated before reload)"
rsh "sudo -n bash -s" <<REMOTE
set -euo pipefail

cat > /etc/ssh/sshd_config.d/60-ssh-tunnel-tests.conf <<'CFG'
# ssh-tunnel-manager live test accounts. Scoped with Match so no other
# account's authentication is affected. Managed by scripts/provision-test-target.sh.
Match User $U_KEY
    PubkeyAuthentication yes
    PasswordAuthentication no
    AuthenticationMethods publickey

Match User $U_PW
    PubkeyAuthentication no
    PasswordAuthentication yes
    AuthenticationMethods password

# The daemon's AuthType::PasswordWith2FA goes straight to keyboard-interactive and
# expects PAM to ask for the password and then the verification code. Requiring
# publickey first would be a different scheme than the one under test.
Match User $U_2FA
    PubkeyAuthentication no
    PasswordAuthentication no
    KbdInteractiveAuthentication yes
    AuthenticationMethods keyboard-interactive
CFG

# Require TOTP for the 2FA account only. The pam_succeed_if line skips the
# google-authenticator module for every other user, so administrative access
# is untouched.
if ! grep -q "ssh-tunnel-manager tests" /etc/pam.d/sshd; then
    cp /etc/pam.d/sshd /etc/pam.d/sshd.pre-ssh-tunnel-tests
    printf '%s\n' \
      '' \
      '# ssh-tunnel-manager tests: TOTP for the 2FA account only.' \
      'auth [success=1 default=ignore] pam_succeed_if.so user != $U_2FA quiet' \
      'auth required pam_google_authenticator.so secret=/home/$U_2FA/.google_authenticator' \
      >> /etc/pam.d/sshd
fi

if ! sshd -t; then
    echo "sshd config validation FAILED - reverting, nothing reloaded" >&2
    rm -f /etc/ssh/sshd_config.d/60-ssh-tunnel-tests.conf
    [ -f /etc/pam.d/sshd.pre-ssh-tunnel-tests ] && mv /etc/pam.d/sshd.pre-ssh-tunnel-tests /etc/pam.d/sshd
    exit 1
fi

systemctl reload ssh.service 2>/dev/null || systemctl restart ssh.socket
REMOTE
ok "sshd validated and reloaded"

say "Verifying administrative access still works"
rsh 'true' && ok "admin access intact"

# --- 4. Test configuration ----------------------------------------------------

say "Writing $ENV_FILE"
cat > "$ENV_FILE" <<ENV
# GENERATED by scripts/provision-test-target.sh - do not commit.
# .local/ is gitignored in its entirety (US-10.4).

SSH_TUNNEL_TEST_HOST=$HOST
SSH_TUNNEL_TEST_PORT=22

SSH_TUNNEL_TEST_FORWARD_HOST=127.0.0.1
SSH_TUNNEL_TEST_FORWARD_PORT=$FORWARD_PORT

SSH_TUNNEL_TEST_USER_KEY=$U_KEY
SSH_TUNNEL_TEST_KEY_PATH=$KEYS_DIR/id_key

SSH_TUNNEL_TEST_ENCRYPTED_KEY_PATH=$KEYS_DIR/id_enc
SSH_TUNNEL_TEST_KEY_PASSPHRASE=$KEY_PASSPHRASE

SSH_TUNNEL_TEST_USER_PASSWORD=$U_PW
SSH_TUNNEL_TEST_PASSWORD=$PW_PASSWORD

SSH_TUNNEL_TEST_USER_2FA=$U_2FA
SSH_TUNNEL_TEST_2FA_PASSWORD=$TFA_PASSWORD
SSH_TUNNEL_TEST_TOTP_SECRET=$TOTP_SECRET
ENV
chmod 600 "$ENV_FILE"
ok "$ENV_FILE written (0600)"

echo
say "Done. Run the live tier with:  make test-live"

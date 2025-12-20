#!/usr/bin/env bash
set -euo pipefail

PREFIX=${PREFIX:-/usr/local}
MODE=""
ENABLE=0
INSTANCE="tunneld" # used for system unit instance name

usage() {
  cat <<'EOF'
Usage: install.sh [--prefix /usr/local] [--user-unit | --system-unit] [--instance NAME] [--enable]

Builds release binaries, installs them to PREFIX/bin, and optionally installs systemd units.

  --prefix PATH     Install prefix (default: /usr/local)
  --user-unit       Install per-user systemd service to ~/.config/systemd/user
  --system-unit     Install system service template to /etc/systemd/system (needs sudo)
  --instance NAME   Instance name for system unit (default: tunneld)
  --enable          Reload systemd and enable/start the chosen unit

Examples:
  ./scripts/install.sh --user-unit --enable
  sudo ./scripts/install.sh --system-unit --instance tunneld --enable
  sudo ./scripts/install.sh --prefix /opt --system-unit --enable
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix) PREFIX="$2"; shift 2 ;;
    --user-unit) MODE="user"; shift ;;
    --system-unit) MODE="system"; shift ;;
    --instance) INSTANCE="$2"; shift 2 ;;
    --enable) ENABLE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage; exit 1 ;;
  esac
done

echo "==> Building release binaries"
cargo build --release --package ssh-tunnel-cli --package ssh-tunnel-daemon

echo "==> Using installation prefix: ${PREFIX}"

echo "==> Installing binaries to ${PREFIX}/bin"
install -Dm755 target/release/ssh-tunnel-daemon "${PREFIX}/bin/ssh-tunnel-daemon"
install -Dm755 target/release/ssh-tunnel "${PREFIX}/bin/ssh-tunnel"

if [[ "${MODE}" == "user" ]]; then
  UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
  echo "==> Installing user unit to ${UNIT_DIR}"
  # Replace /usr/local with actual PREFIX in the unit file
  sed "s|/usr/local|${PREFIX}|g" docs/systemd/ssh-tunnel-daemon.user.service > /tmp/ssh-tunnel-daemon.user.service
  install -Dm644 /tmp/ssh-tunnel-daemon.user.service "${UNIT_DIR}/ssh-tunnel-daemon.service"
  rm /tmp/ssh-tunnel-daemon.user.service
  if [[ "${ENABLE}" -eq 1 ]]; then
    systemctl --user daemon-reload
    systemctl --user enable --now ssh-tunnel-daemon.service
    echo "User service enabled. Logs: journalctl --user-unit ssh-tunnel-daemon -f"
  else
    echo "Reload and enable with: systemctl --user daemon-reload && systemctl --user enable --now ssh-tunnel-daemon.service"
  fi
elif [[ "${MODE}" == "system" ]]; then
  UNIT_DIR="/etc/systemd/system"
  echo "==> Installing system unit to ${UNIT_DIR}"
  # Replace /usr/local with actual PREFIX in the unit file
  sed "s|/usr/local|${PREFIX}|g" docs/systemd/ssh-tunnel-daemon@.service > /tmp/ssh-tunnel-daemon@.service
  install -Dm644 /tmp/ssh-tunnel-daemon@.service "${UNIT_DIR}/ssh-tunnel-daemon@.service"
  rm /tmp/ssh-tunnel-daemon@.service
  if [[ "${ENABLE}" -eq 1 ]]; then
    systemctl daemon-reload
    systemctl enable --now "ssh-tunnel-daemon@${INSTANCE}.service"
    echo "System service enabled for instance '${INSTANCE}'. Logs: journalctl -u ssh-tunnel-daemon@${INSTANCE} -f"
  else
    echo "Reload and enable with: systemctl daemon-reload && systemctl enable --now ssh-tunnel-daemon@${INSTANCE}.service"
  fi
else
  echo "Units not installed (no --user-unit or --system-unit provided)."
fi

echo "Done."

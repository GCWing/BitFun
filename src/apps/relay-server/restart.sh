#!/usr/bin/env bash
# BitFun Relay Server — restart script.
# Run this script on the target server itself after SSH login.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "${SCRIPT_DIR}/common.sh"

usage() {
  cat <<'EOF'
BitFun Relay Server restart script

Usage:
  bash restart.sh

Run location:
  Execute this script on the target server itself after SSH login.

Behavior:
  If the relay service is already running, this script restarts it.
  If it is not running, this script starts it.
EOF
}

for arg in "$@"; do
  case "$arg" in
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $arg"
      usage
      exit 1
      ;;
  esac
done

echo "=== BitFun Relay Server Restart ==="
require_docker_daemon
resolve_compose
cd "$SCRIPT_DIR"

if container_running; then
  echo "Relay service is running. Restarting it..."
  compose up -d --force-recreate
else
  echo "Relay service is not running. Starting it instead..."
  compose up -d
fi

echo ""
echo "Relay service is ready."
echo "Relay endpoint: http://<this-server-ip>:${RELAY_PORT:-9700}"
echo "Check status:  ${COMPOSE[*]} ps"
echo "View logs:     ${COMPOSE[*]} logs -f relay-server"

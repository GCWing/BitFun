#!/usr/bin/env bash
# BitFun Relay Server — start script.
# Run this script on the target server itself after SSH login.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "${SCRIPT_DIR}/common.sh"

usage() {
  cat <<'EOF'
BitFun Relay Server start script

Usage:
  bash start.sh

Run location:
  Execute this script on the target server itself after SSH login.

Behavior:
  If the relay service is already running, this script exits without starting it again.
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

echo "=== BitFun Relay Server Start ==="
require_docker_daemon
resolve_compose
cd "$SCRIPT_DIR"

if container_running; then
  echo "Relay service is already running. Nothing to do."
  exit 0
fi

if container_exists; then
  echo "Relay service exists but is stopped. Starting it..."
else
  echo "Relay service is not created yet. Creating and starting it..."
fi

compose up -d

echo ""
echo "Relay service started."
echo "Relay endpoint: http://<this-server-ip>:${RELAY_PORT:-9700}"
echo "Check status:  ${COMPOSE[*]} ps"
echo "View logs:     ${COMPOSE[*]} logs -f relay-server"

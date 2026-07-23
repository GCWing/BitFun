#!/usr/bin/env bash
# BitFun Relay Server — stop script.
# Run this script on the target server itself after SSH login.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "${SCRIPT_DIR}/common.sh"

usage() {
  cat <<'EOF'
BitFun Relay Server stop script

Usage:
  bash stop.sh

Run location:
  Execute this script on the target server itself after SSH login.
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

echo "=== BitFun Relay Server Stop ==="
require_docker_daemon
resolve_compose
cd "$SCRIPT_DIR"

if ! container_running; then
  echo "Relay service is already stopped. Nothing to do."
  exit 0
fi

compose stop

echo ""
echo "Relay service stopped."
echo "Check status:  ${COMPOSE[*]} ps"
echo "Start again:   bash start.sh"

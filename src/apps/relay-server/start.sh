#!/usr/bin/env bash
# BitFun Relay Server — start script.
# Run this script on the target server itself after SSH login.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTAINER_NAME="bitfun-relay"
RELAY_HOST_BIND_IP="127.0.0.1"

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

check_command() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Error: '$cmd' is required but not installed."
    exit 1
  fi
}

check_docker_compose() {
  if docker compose version >/dev/null 2>&1; then
    return 0
  fi
  echo "Error: Docker Compose (docker compose) is required."
  exit 1
}

container_exists() {
  docker container inspect "$CONTAINER_NAME" >/dev/null 2>&1
}

container_running() {
  [ "$(docker inspect -f '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null || echo false)" = "true" ]
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
check_command docker
check_docker_compose

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

RELAY_HOST_BIND_IP="$RELAY_HOST_BIND_IP" docker compose up -d

echo ""
echo "Relay service started."
echo "Relay endpoint: http://127.0.0.1:9700"
echo "Check status:  docker compose ps"
echo "View logs:     docker compose logs -f relay-server"

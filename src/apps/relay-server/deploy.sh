#!/usr/bin/env bash
# BitFun Relay Server — one-click deploy script.
# Usage:  bash deploy.sh [--skip-build] [--skip-health-check]
#
# Run this script on the target server itself after SSH login.
# It deploys to the current machine only; it does not SSH to a remote host.
#
# Supported hosts: Linux amd64 (x86_64) and arm64 (aarch64) with Docker.
#
# Prerequisites: Docker + Compose V2 (`docker compose`) or legacy docker-compose
#
# Low-memory VPS tip (especially arm64):
#   RELAY_CARGO_BUILD_JOBS=1 bash deploy.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "${SCRIPT_DIR}/common.sh"

SKIP_BUILD=false
SKIP_HEALTH_CHECK=false

usage() {
  cat <<'EOF'
BitFun Relay Server deploy script

Usage:
  bash deploy.sh [options]

Run location:
  Execute this script on the target server itself after SSH login.
  This script only deploys to the current machine.

Supported architectures:
  linux/amd64 (x86_64), linux/arm64 (aarch64)

Options:
  --skip-build         Skip docker compose build, only recreate/start services
  --skip-health-check  Skip post-deploy health check
  -h, --help           Show this help message

Environment:
  RELAY_HOST_BIND_IP       Host bind address for published port (default 0.0.0.0)
  RELAY_CARGO_BUILD_JOBS   Limit rustc parallelism inside Docker (e.g. 1 on small VPS)
  DOCKER_DEFAULT_PLATFORM  Leave unset for native host builds (recommended)
EOF
}

for arg in "$@"; do
  case "$arg" in
    --skip-build) SKIP_BUILD=true ;;
    --skip-health-check) SKIP_HEALTH_CHECK=true ;;
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

HOST_ARCH="$(host_arch_label)"

echo "=== BitFun Relay Server Deploy ==="
echo "Target: current machine ($(uname -s) / ${HOST_ARCH}, uname=$(uname -m))"
echo "Note: run this script on the target server after SSH login."

assert_supported_arch
require_docker_daemon
resolve_compose
warn_if_forced_foreign_platform

echo "Compose: ${COMPOSE[*]}"
cd "$SCRIPT_DIR"

# Build first so a compile failure does not take down a running relay.
if [ "$SKIP_BUILD" = true ]; then
  echo "[1/2] Skipping Docker build (--skip-build)"
else
  echo "[1/2] Building Docker image for host architecture (${HOST_ARCH})..."
  BUILD_ARGS=()
  if [ -n "${RELAY_CARGO_BUILD_JOBS:-}" ]; then
    BUILD_ARGS+=(--build-arg "CARGO_BUILD_JOBS=${RELAY_CARGO_BUILD_JOBS}")
    echo "  Using CARGO_BUILD_JOBS=${RELAY_CARGO_BUILD_JOBS}"
  fi
  # Plain progress so nohup/file-redirected deploys still stream build lines.
  export BUILDKIT_PROGRESS="${BUILDKIT_PROGRESS:-plain}"
  # Do not pass --platform unless the user explicitly set DOCKER_DEFAULT_PLATFORM;
  # native builds on amd64/arm64 servers are the supported path.
  # Compose V2 wants --progress as a global flag; honor BITFUN_DOCKER_MODE from common.sh.
  case "${BITFUN_DOCKER_MODE:-direct}" in
    sudo)
      sudo docker compose --progress=plain build "${BUILD_ARGS[@]}"
      ;;
    sg)
      # shellcheck disable=SC2086
      sg docker -c "docker compose --progress=plain build ${BUILD_ARGS[*]}"
      ;;
    *)
      if [ "${#COMPOSE[@]}" -ge 2 ] && [ "${COMPOSE[0]}" = "docker" ] && [ "${COMPOSE[1]}" = "compose" ]; then
        docker compose --progress=plain build "${BUILD_ARGS[@]}"
      else
        compose build "${BUILD_ARGS[@]}"
      fi
      ;;
  esac
fi

echo "[2/2] Starting / recreating services..."
compose up -d --force-recreate --remove-orphans

if [ "$SKIP_HEALTH_CHECK" = false ]; then
  echo "Waiting for services to start..."
  sleep 2
  wait_for_relay_health 12
fi

RELAY_PORT="${RELAY_PORT:-9700}"
echo ""
echo "=== Deploy complete ==="
echo "Relay server running on port ${RELAY_PORT} (host arch: ${HOST_ARCH})"
echo ""
check_relay_accounts_or_remind
echo ""
echo "Point BitFun Desktop / CLI Auth Server URL to:"
echo "  Direct:   http://<YOUR_SERVER_IP>:${RELAY_PORT}"
echo "  Proxy:    https://<YOUR_DOMAIN>/relay  (recommended, matches official server)"
echo "See README.md for reverse proxy setup, sync, and Peer Device Mode."
echo ""
echo "Check status:  bash -c 'cd \"${SCRIPT_DIR}\" && ${COMPOSE[*]} ps'"
echo "Start:         bash start.sh"
echo "Restart:       bash restart.sh"
echo "Stop:          bash stop.sh"
echo "View logs:     ${COMPOSE[*]} logs -f relay-server"

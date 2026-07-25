#!/usr/bin/env bash
# BitFun Relay Server — one-click deploy script.
# Usage:  bash deploy.sh [--skip-build] [--skip-health-check] [--cn-mirror|--global-mirror]
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
#
# China hosts: auto-detects mainland China and configures apt/Docker/cargo/GitHub
# mirrors (override with BITFUN_MIRROR=cn|global or --cn-mirror/--global-mirror).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=common.sh
source "${SCRIPT_DIR}/common.sh"
# shellcheck source=mirror.sh
source "${SCRIPT_DIR}/mirror.sh"

SKIP_BUILD=false
SKIP_HEALTH_CHECK=false
MIRROR_ARGS=()

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
  --cn-mirror          Force China mirrors (apt/Docker/cargo/GitHub)
  --global-mirror      Force global upstream mirrors
  -h, --help           Show this help message

Environment:
  RELAY_HOST_BIND_IP       Host bind address for published port (default 0.0.0.0)
  RELAY_CARGO_BUILD_JOBS   Limit rustc parallelism inside Docker (e.g. 1 on small VPS)
  DOCKER_DEFAULT_PLATFORM  Leave unset for native host builds (recommended)
  BITFUN_MIRROR            auto|cn|global (default auto)
  BITFUN_APT_MIRROR        Debian/Ubuntu apt host (default mirrors.aliyun.com)
  BITFUN_DOCKER_REGISTRY_MIRRORS  Space/comma-separated Docker Hub mirrors
  BITFUN_CARGO_SPARSE_URL  Cargo sparse registry URL (default rsproxy)
  BITFUN_GITHUB_PROXY      GitHub HTTPS proxy prefix (default https://ghfast.top/)
EOF
}

for arg in "$@"; do
  case "$arg" in
    --skip-build) SKIP_BUILD=true ;;
    --skip-health-check) SKIP_HEALTH_CHECK=true ;;
    --cn-mirror|--global-mirror|--no-cn-mirror|--skip-mirror-apply)
      MIRROR_ARGS+=("$arg")
      ;;
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
# Detect region and persist host mirrors before Docker pulls / image build.
# Validate the host first so unsupported machines are not modified.
bitfun_mirror_init "${MIRROR_ARGS[@]+"${MIRROR_ARGS[@]}"}"
require_docker_daemon
resolve_compose
warn_if_forced_foreign_platform

echo "Compose: ${COMPOSE[*]}"
cd "$SCRIPT_DIR"

# Persist compose build-args for CN builds (and subsequent restarts).
touch .env
chmod 600 .env 2>/dev/null || true
# Refresh BitFun-managed mirror keys without wiping unrelated .env entries.
if [ -f .env ]; then
  tmp_env="$(mktemp)"
  grep -Ev '^(BITFUN_USE_CN_MIRROR|BITFUN_APT_MIRROR|BITFUN_CARGO_SPARSE_URL)=' .env >"$tmp_env" || true
  mv "$tmp_env" .env
fi
{
  echo "BITFUN_USE_CN_MIRROR=${BITFUN_USE_CN_MIRROR:-0}"
  echo "BITFUN_APT_MIRROR=${BITFUN_APT_MIRROR:-mirrors.aliyun.com}"
  echo "BITFUN_CARGO_SPARSE_URL=${BITFUN_CARGO_SPARSE_URL:-sparse+https://rsproxy.cn/index/}"
} >>.env

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
  BUILD_ARGS+=(--build-arg "BITFUN_USE_CN_MIRROR=${BITFUN_USE_CN_MIRROR:-0}")
  BUILD_ARGS+=(--build-arg "BITFUN_APT_MIRROR=${BITFUN_APT_MIRROR:-mirrors.aliyun.com}")
  BUILD_ARGS+=(--build-arg "BITFUN_CARGO_SPARSE_URL=${BITFUN_CARGO_SPARSE_URL:-sparse+https://rsproxy.cn/index/}")
  if [ "${BITFUN_USE_CN_MIRROR:-0}" = "1" ]; then
    echo "  Using China mirrors inside Docker build (apt + cargo)"
  fi
  # BuildKit is required for Dockerfile cargo registry/git/target cache mounts.
  # Plain progress so nohup/file-redirected deploys still stream build lines.
  export DOCKER_BUILDKIT=1
  export COMPOSE_DOCKER_CLI_BUILD=1
  export BUILDKIT_PROGRESS="${BUILDKIT_PROGRESS:-plain}"
  echo "  Using Docker BuildKit (cargo cache mounts enabled)"
  # Do not pass --platform unless the user explicitly set DOCKER_DEFAULT_PLATFORM;
  # native builds on amd64/arm64 servers are the supported path.
  # Compose V2 wants --progress as a global flag; honor BITFUN_DOCKER_MODE from common.sh.
  case "${BITFUN_DOCKER_MODE:-direct}" in
    sudo)
      sudo env DOCKER_BUILDKIT=1 COMPOSE_DOCKER_CLI_BUILD=1 BUILDKIT_PROGRESS="${BUILDKIT_PROGRESS}" \
        BITFUN_USE_CN_MIRROR="${BITFUN_USE_CN_MIRROR:-0}" \
        BITFUN_APT_MIRROR="${BITFUN_APT_MIRROR:-mirrors.aliyun.com}" \
        BITFUN_CARGO_SPARSE_URL="${BITFUN_CARGO_SPARSE_URL:-sparse+https://rsproxy.cn/index/}" \
        docker compose --progress=plain build "${BUILD_ARGS[@]}"
      ;;
    sg)
      # shellcheck disable=SC2086
      sg docker -c "env DOCKER_BUILDKIT=1 COMPOSE_DOCKER_CLI_BUILD=1 BUILDKIT_PROGRESS='${BUILDKIT_PROGRESS}' BITFUN_USE_CN_MIRROR='${BITFUN_USE_CN_MIRROR:-0}' BITFUN_APT_MIRROR='${BITFUN_APT_MIRROR:-mirrors.aliyun.com}' BITFUN_CARGO_SPARSE_URL='${BITFUN_CARGO_SPARSE_URL:-sparse+https://rsproxy.cn/index/}' docker compose --progress=plain build ${BUILD_ARGS[*]}"
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

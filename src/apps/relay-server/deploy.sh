#!/usr/bin/env bash
# BitFun Relay Server — one-click deploy script.
# Usage:  bash deploy.sh [--skip-build] [--skip-health-check]
#
# Run this script on the target server itself after SSH login.
# It deploys to the current machine only; it does not SSH to a remote host.
#
# Prerequisites: Docker, Docker Compose

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

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

Options:
  --skip-build         Skip docker compose build, only restart services
  --skip-health-check  Skip post-deploy health check
  -h, --help           Show this help message
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

echo "=== BitFun Relay Server Deploy ==="
echo "Target: current machine"
echo "Note: run this script on the target server after SSH login."
check_command docker
check_docker_compose

cd "$SCRIPT_DIR"

# Stop old containers if running
echo "[1/3] Stopping old containers (if running)..."
docker compose down 2>/dev/null || true
echo "  Done."

# Build
if [ "$SKIP_BUILD" = true ]; then
  echo "[2/3] Skipping Docker build (--skip-build)"
else
  echo "[2/3] Building Docker images..."
  docker compose build
fi

# Start
echo "[3/3] Starting services..."
docker compose up -d

if [ "$SKIP_HEALTH_CHECK" = false ]; then
  echo "Waiting for services to start..."
  sleep 5
  echo "Checking relay health endpoint..."
  if command -v curl >/dev/null 2>&1; then
    MAX_RETRIES=6
    RETRY=0
    while [ $RETRY -lt $MAX_RETRIES ]; do
      if curl -fsS --max-time 5 "http://127.0.0.1:9700/health" >/dev/null 2>&1; then
        echo "Health check passed: http://127.0.0.1:9700/health"
        break
      fi
      RETRY=$((RETRY + 1))
      if [ $RETRY -lt $MAX_RETRIES ]; then
        echo "  Retry $RETRY/$MAX_RETRIES in 3s..."
        sleep 3
      else
        echo "Warning: health check failed after $MAX_RETRIES attempts. Check logs:"
        docker compose logs --tail=30 relay-server
      fi
    done
  else
    echo "Warning: 'curl' not found, skipped health check."
  fi
fi

echo ""
echo "=== Deploy complete ==="
echo "Relay server running on port 9700"
echo ""

CONTAINER_NAME="bitfun-relay"
RELAY_ADMIN_DB="/app/data/bitfun_relay.db"
ADD_USER_CMD="docker exec -it ${CONTAINER_NAME} /app/relay-admin --db ${RELAY_ADMIN_DB} add-user --username <name>"

print_client_url_hint() {
  echo "Point BitFun Desktop / CLI Auth Server URL to:"
  echo "  http://<YOUR_SERVER_IP>:9700"
  echo "See README.md for sync, Peer Device Mode, and proxy timeouts."
}

if docker container inspect "$CONTAINER_NAME" >/dev/null 2>&1 \
  && [ "$(docker inspect -f '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null || echo false)" = "true" ]; then
  # Empty DB prints "No accounts found."; otherwise a USERNAME header + rows.
  USER_LIST="$(
    docker exec "$CONTAINER_NAME" /app/relay-admin --db "$RELAY_ADMIN_DB" list-users 2>/dev/null || true
  )"
  if echo "$USER_LIST" | grep -q '^No accounts found\.'; \
    || ! echo "$USER_LIST" | grep -q '^USERNAME'; then
    echo "No relay accounts yet. Account login will not work until you create one."
    echo "Run:"
    echo "  ${ADD_USER_CMD}"
    echo "(omit --password to enter the password interactively)"
    echo ""
    print_client_url_hint
  else
    USER_COUNT="$(
      echo "$USER_LIST" | awk 'NR>2 && NF { count++ } END { print count+0 }'
    )"
    echo "Relay accounts found: ${USER_COUNT}"
    echo ""
    print_client_url_hint
  fi
else
  echo "Warning: container '${CONTAINER_NAME}' is not running; skipped account check."
  echo "After it is up, create an account with:"
  echo "  ${ADD_USER_CMD}"
fi

echo ""
echo "Check status:  docker compose ps"
echo "Start:         bash start.sh"
echo "Restart:       bash restart.sh"
echo "Stop:          bash stop.sh"
echo "View logs:     docker compose logs -f relay-server"

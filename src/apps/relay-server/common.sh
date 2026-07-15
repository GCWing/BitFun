#!/usr/bin/env bash
# Shared helpers for BitFun relay-server deploy/start/stop/restart scripts.
# Sourced by the other *.sh files in this directory (not executed directly).

CONTAINER_NAME="${CONTAINER_NAME:-bitfun-relay}"
RELAY_ADMIN_DB="${RELAY_ADMIN_DB:-/app/data/bitfun_relay.db}"
RELAY_HEALTH_URL="${RELAY_HEALTH_URL:-http://127.0.0.1:9700/health}"
COMPOSE=()

check_command() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Error: '$cmd' is required but not installed."
    exit 1
  fi
}

# Prefer Compose V2 plugin (`docker compose`); fall back to legacy binary.
resolve_compose() {
  if docker compose version >/dev/null 2>&1; then
    COMPOSE=(docker compose)
    return 0
  fi
  if command -v docker-compose >/dev/null 2>&1; then
    COMPOSE=(docker-compose)
    return 0
  fi
  echo "Error: Docker Compose is required."
  echo "Install either:"
  echo "  - Docker Compose V2 plugin (docker compose), or"
  echo "  - legacy docker-compose binary"
  exit 1
}

compose() {
  if [ "${#COMPOSE[@]}" -eq 0 ]; then
    resolve_compose
  fi
  "${COMPOSE[@]}" "$@"
}

require_docker_daemon() {
  check_command docker
  if ! docker info >/dev/null 2>&1; then
    echo "Error: Docker daemon is not running or this user cannot access it."
    echo "Try: sudo systemctl start docker"
    echo "Or add your user to the 'docker' group and re-login."
    exit 1
  fi
}

# Normalize uname -m to a short label used in logs / docs.
host_arch_label() {
  local m
  m="$(uname -m 2>/dev/null || echo unknown)"
  case "$m" in
    x86_64 | amd64) echo "amd64" ;;
    aarch64 | arm64) echo "arm64" ;;
    armv7l | armhf) echo "armv7" ;;
    *) echo "$m" ;;
  esac
}

# Refuse obscure arches early; amd64 + arm64 are the supported deploy targets.
assert_supported_arch() {
  local arch
  arch="$(host_arch_label)"
  case "$arch" in
    amd64 | arm64) ;;
    *)
      echo "Error: unsupported host architecture '$arch' ($(uname -m))."
      echo "One-click Docker deploy is supported on linux/amd64 and linux/arm64."
      exit 1
      ;;
  esac
}

# Warn if the environment forces a foreign Docker platform (common on mixed hosts).
warn_if_forced_foreign_platform() {
  local host_arch docker_platform normalized_host
  host_arch="$(host_arch_label)"
  docker_platform="${DOCKER_DEFAULT_PLATFORM:-}"
  [ -z "$docker_platform" ] && return 0

  normalized_host="linux/${host_arch}"
  case "$docker_platform" in
    *"${host_arch}"* | *"$(uname -m)"*) return 0 ;;
  esac

  echo "Warning: DOCKER_DEFAULT_PLATFORM=${docker_platform} differs from host ${normalized_host}."
  echo "         Native deploy builds for the host CPU. Unset DOCKER_DEFAULT_PLATFORM"
  echo "         unless you intentionally cross-build (needs qemu/binfmt)."
}

container_exists() {
  docker container inspect "$CONTAINER_NAME" >/dev/null 2>&1
}

container_running() {
  [ "$(docker inspect -f '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null || echo false)" = "true" ]
}

# Health probe that works without host curl/wget when possible.
probe_relay_health() {
  # 1) Inside the container (most reliable; works even if host bind IP != 127.0.0.1)
  if container_running; then
    if docker exec "$CONTAINER_NAME" curl -fsS --max-time 5 "$RELAY_HEALTH_URL" >/dev/null 2>&1; then
      return 0
    fi
    # BusyBox-style wget (if present)
    if docker exec "$CONTAINER_NAME" wget -q -O /dev/null --timeout=5 "$RELAY_HEALTH_URL" >/dev/null 2>&1; then
      return 0
    fi
  fi

  # 2) Host curl / wget against published port
  if command -v curl >/dev/null 2>&1; then
    curl -fsS --max-time 5 "$RELAY_HEALTH_URL" >/dev/null 2>&1 && return 0
  fi
  if command -v wget >/dev/null 2>&1; then
    wget -q -O /dev/null --timeout=5 "$RELAY_HEALTH_URL" >/dev/null 2>&1 && return 0
  fi

  return 1
}

wait_for_relay_health() {
  local max_retries="${1:-12}"
  local retry=0
  echo "Checking relay health (${RELAY_HEALTH_URL})..."
  while [ "$retry" -lt "$max_retries" ]; do
    if probe_relay_health; then
      echo "Health check passed."
      return 0
    fi
    retry=$((retry + 1))
    if [ "$retry" -lt "$max_retries" ]; then
      echo "  Retry $retry/$max_retries in 3s..."
      sleep 3
    fi
  done
  echo "Error: health check failed after $max_retries attempts."
  compose logs --tail=40 relay-server || true
  return 1
}

print_add_user_command() {
  echo "  docker exec -it ${CONTAINER_NAME} /app/relay-admin --db ${RELAY_ADMIN_DB} add-user --username <name>"
}

check_relay_accounts_or_remind() {
  if ! container_running; then
    echo "Warning: container '${CONTAINER_NAME}' is not running; skipped account check."
    echo "After it is up, create an account with:"
    print_add_user_command
    return 0
  fi

  local user_list
  user_list="$(
    docker exec "$CONTAINER_NAME" /app/relay-admin --db "$RELAY_ADMIN_DB" list-users 2>/dev/null || true
  )"

  local empty=0
  if echo "$user_list" | grep -q '^No accounts found\.'; then
    empty=1
  elif ! echo "$user_list" | grep -q '^USERNAME'; then
    empty=1
  fi

  if [ "$empty" -eq 1 ]; then
    echo "No relay accounts yet. Account login will not work until you create one."
    echo "Run:"
    print_add_user_command
    echo "(omit --password to enter the password interactively)"
  else
    local user_count
    user_count="$(
      echo "$user_list" | awk 'NR>2 && NF { count++ } END { print count+0 }'
    )"
    echo "Relay accounts found: ${user_count}"
  fi
}

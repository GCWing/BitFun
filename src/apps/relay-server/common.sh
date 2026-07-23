#!/usr/bin/env bash
# Shared helpers for BitFun relay-server deploy/start/stop/restart scripts.
# Sourced by the other *.sh files in this directory (not executed directly).

CONTAINER_NAME="${CONTAINER_NAME:-bitfun-relay}"
RELAY_ADMIN_DB="${RELAY_ADMIN_DB:-/app/data/bitfun_relay.db}"
RELAY_PORT="${RELAY_PORT:-9700}"
RELAY_HEALTH_URL="${RELAY_HEALTH_URL:-http://127.0.0.1:${RELAY_PORT}/health}"
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
  case "${BITFUN_DOCKER_MODE:-direct}" in
    sudo)
      # Prefer Compose V2 via sudo docker when the daemon needs root.
      if sudo docker compose version >/dev/null 2>&1; then
        sudo docker compose "$@"
        return
      fi
      ;;
    sg)
      if sg docker -c 'docker compose version' >/dev/null 2>&1; then
        sg docker -c "docker compose $*"
        return
      fi
      ;;
  esac
  "${COMPOSE[@]}" "$@"
}

# Resolve how to talk to the Docker daemon for the current shell.
# Sets BITFUN_DOCKER_MODE to: direct | sg | sudo
resolve_docker_access() {
  check_command docker
  export DOCKER_CONFIG="${DOCKER_CONFIG:-$HOME/.bitfun/docker-config}"
  mkdir -p "$DOCKER_CONFIG" 2>/dev/null || true

  if [ -e "$HOME/.docker" ] && [ ! -w "$HOME/.docker" ]; then
    echo "Warning: $HOME/.docker is not writable (often root-owned after sudo docker)."
    echo "         Attempting chown; sudo password may be required..."
    if [ "$(id -u)" = "0" ]; then
      chown -R "$(id -un):$(id -gn)" "$HOME/.docker" || true
    elif sudo -n chown -R "$(id -un):$(id -gn)" "$HOME/.docker" 2>/dev/null; then
      :
    else
      sudo chown -R "$(id -un):$(id -gn)" "$HOME/.docker" || true
    fi
    if [ -e "$HOME/.docker" ] && [ ! -w "$HOME/.docker" ]; then
      echo "         Using isolated DOCKER_CONFIG=$DOCKER_CONFIG instead."
    fi
  fi

  if docker info >/dev/null 2>&1; then
    BITFUN_DOCKER_MODE=direct
    return 0
  fi
  if id -nG 2>/dev/null | tr ' ' '\n' | grep -qx docker \
    || getent group docker 2>/dev/null | grep -qE "(^|:|,)$(id -un)(,|$)"; then
    if sg docker -c 'docker info' >/dev/null 2>&1; then
      BITFUN_DOCKER_MODE=sg
      echo "Note: using 'sg docker' (group membership not active in this session)."
      return 0
    fi
  fi
  if sudo -n docker info >/dev/null 2>&1 || sudo docker info >/dev/null 2>&1; then
    BITFUN_DOCKER_MODE=sudo
    echo "Note: using sudo for Docker access."
    return 0
  fi
  echo "Error: Docker daemon is not running or this user cannot access it."
  echo "Try: sudo systemctl start docker"
  echo "Or add your user to the 'docker' group and re-login (or: newgrp docker)."
  exit 1
}

docker_cmd() {
  case "${BITFUN_DOCKER_MODE:-direct}" in
    sg) sg docker -c "docker $*" ;;
    sudo) sudo docker "$@" ;;
    *) docker "$@" ;;
  esac
}

require_docker_daemon() {
  resolve_docker_access
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
  docker_cmd container inspect "$CONTAINER_NAME" >/dev/null 2>&1
}

container_running() {
  [ "$(docker_cmd inspect -f '{{.State.Running}}' "$CONTAINER_NAME" 2>/dev/null || echo false)" = "true" ]
}

# Health probe that works without host curl/wget when possible.
probe_relay_health() {
  # 1) Inside the container (most reliable; works even if host bind IP != 127.0.0.1)
  if container_running; then
    if docker_cmd exec "$CONTAINER_NAME" curl -fsS --max-time 5 "$RELAY_HEALTH_URL" >/dev/null 2>&1; then
      return 0
    fi
    # BusyBox-style wget (if present)
    if docker_cmd exec "$CONTAINER_NAME" wget -q -O /dev/null --timeout=5 "$RELAY_HEALTH_URL" >/dev/null 2>&1; then
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
    docker_cmd exec "$CONTAINER_NAME" /app/relay-admin --db "$RELAY_ADMIN_DB" list-users 2>/dev/null || true
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

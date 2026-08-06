#!/usr/bin/env bash
# BitFun Relay Server — pull and start the published multi-platform image.
#
# Shared by:
#   - src/apps/relay-server/deploy.sh
#   - remote_ssh/relay_deploy.rs (embedded with include_str!)
#
# The Desktop path supplies BITFUN_RELAY_IMAGE_DIGEST from a minisign-verified
# release descriptor. Docker then verifies every manifest/layer against that
# immutable digest while pulling through either the official registry or a
# China acceleration prefix. The old download-archive + local image-construction
# path is intentionally gone.
#
# Configuration:
#   BITFUN_RELAY_IMAGE          canonical image repository
#   BITFUN_RELAY_IMAGE_DIGEST   sha256:<64 lowercase hex> (required by Desktop)
#   BITFUN_RELAY_IMAGE_TAG      manual-script fallback tag (default release tag)
#   BITFUN_IMAGE_PULL_TIMEOUT   per-route pull timeout in seconds (default 900)
#   BITFUN_GITHUB_PROBE_WINDOW  GitHub throughput probe window (default 10s)
#   BITFUN_GITHUB_HEALTHY_BPS   keep GitHub first at/above this rate (default 512 KiB/s)
#   BITFUN_MIRROR_MODE          cn | global
#   RELAY_PORT                  published/container port (default 9700)
#   RELAY_HOST_BIND_IP          host bind address (default 0.0.0.0)

BITFUN_RELAY_IMAGE="${BITFUN_RELAY_IMAGE:-ghcr.io/gcwing/bitfun-relay-server}"
BITFUN_RELAY_IMAGE_TAG="${BITFUN_RELAY_IMAGE_TAG:-${BITFUN_RELEASE_TAG:-latest}}"
BITFUN_IMAGE_PULL_TIMEOUT="${BITFUN_IMAGE_PULL_TIMEOUT:-900}"
BITFUN_GITHUB_PROBE_WINDOW="${BITFUN_GITHUB_PROBE_WINDOW:-10}"
BITFUN_GITHUB_HEALTHY_BPS="${BITFUN_GITHUB_HEALTHY_BPS:-524288}"
BITFUN_GITHUB_RELEASE_BASE="${BITFUN_GITHUB_RELEASE_BASE:-https://github.com/GCWing/BitFun/releases}"

# The embedded Desktop helpers call Docker through bitfun_docker; the manual
# script exposes docker_cmd. Keep this file independent of either caller.
bitfun_image_docker() {
  if declare -F bitfun_docker >/dev/null 2>&1; then
    bitfun_docker "$@"
  elif declare -F docker_cmd >/dev/null 2>&1; then
    docker_cmd "$@"
  else
    docker "$@"
  fi
}

# POSIX-quote arguments passed through `sg docker -c`.
bitfun_image_shell_join() {
  local out="" arg
  for arg in "$@"; do
    out="$out'$(printf '%s' "$arg" | sed "s/'/'\\\\''/g")' "
  done
  printf '%s' "$out"
}

# Bound each registry route. A dead accelerator must not prevent falling back
# to the next route forever. Hosts without GNU timeout still retain Docker's
# own network timeouts and progress reporting.
bitfun_image_docker_with_timeout() {
  local seconds="$1"
  shift
  if ! command -v timeout >/dev/null 2>&1; then
    bitfun_image_docker "$@"
    return
  fi
  case "${BITFUN_DOCKER_MODE:-direct}" in
    sg)
      sg docker -c "$(bitfun_image_shell_join timeout "$seconds" docker "$@")"
      ;;
    sudo)
      if sudo -n true >/dev/null 2>&1; then
        sudo -n timeout "$seconds" docker "$@"
      else
        sudo timeout "$seconds" docker "$@"
      fi
      ;;
    *) timeout "$seconds" docker "$@" ;;
  esac
}

bitfun_relay_native_platform() {
  case "$(uname -m 2>/dev/null)" in
    x86_64 | amd64) echo linux/amd64 ;;
    aarch64 | arm64) echo linux/arm64 ;;
    *) return 1 ;;
  esac
}

bitfun_relay_image_ref() {
  local repository="$1"
  if [ -n "${BITFUN_RELAY_IMAGE_DIGEST:-}" ]; then
    printf '%s@%s' "$repository" "$BITFUN_RELAY_IMAGE_DIGEST"
  else
    printf '%s:%s' "$repository" "$BITFUN_RELAY_IMAGE_TAG"
  fi
}

bitfun_relay_archive_target() {
  case "$1" in
    linux/amd64) echo x86_64-unknown-linux-gnu ;;
    linux/arm64) echo aarch64-unknown-linux-gnu ;;
    *) return 1 ;;
  esac
}

# Measure the GitHub release CDN that carries the exact Relay binary bytes.
# Relay itself stays a digest-pinned OCI deployment; this byte probe decides
# whether official GHCR or its accelerators are attempted first.
bitfun_probe_github_throughput() {
  local platform="$1" target tag url metrics speed
  target="$(bitfun_relay_archive_target "$platform")" || {
    echo 0
    return 0
  }
  tag="${BITFUN_RELEASE_TAG:-latest}"
  if [ "$tag" = latest ]; then
    url="${BITFUN_GITHUB_RELEASE_BASE}/latest/download/bitfun-relay-server-${target}.tar.gz"
  else
    url="${BITFUN_GITHUB_RELEASE_BASE}/download/${tag}/bitfun-relay-server-${target}.tar.gz"
  fi
  if ! command -v curl >/dev/null 2>&1; then
    echo ">>> GitHub throughput probe skipped: curl is unavailable" >&2
    echo 0
    return 0
  fi
  case "$BITFUN_GITHUB_PROBE_WINDOW" in
    '' | *[!0-9]*) BITFUN_GITHUB_PROBE_WINDOW=10 ;;
  esac
  case "$BITFUN_GITHUB_HEALTHY_BPS" in
    '' | *[!0-9]*) BITFUN_GITHUB_HEALTHY_BPS=524288 ;;
  esac
  metrics="$(curl -LsS \
    --range 0-4194303 \
    --connect-timeout 5 \
    --max-time "$BITFUN_GITHUB_PROBE_WINDOW" \
    -o /dev/null \
    -w '%{http_code} %{size_download} %{time_total}' \
    "$url" 2>/dev/null || true)"
  speed="$(printf '%s\n' "$metrics" | awk '
    ($1 == 200 || $1 == 206) && $3 > 0 { printf "%.0f\n", $2 / $3; ok=1 }
    END { if (!ok) print 0 }
  ')"
  case "$speed" in
    '' | *[!0-9]*) speed=0 ;;
  esac
  echo ">>> GitHub Relay probe: $((speed / 1024)) KiB/s (healthy floor: $((BITFUN_GITHUB_HEALTHY_BPS / 1024)) KiB/s)" >&2
  echo "$speed"
}

# GitHub is the default route. In auto mode, a measured GitHub rate below
# 512 KiB/s moves the registry accelerators first. Explicit cn/global choices
# remain authoritative. The digest is identical across routes, so transport
# selection never changes the image Desktop authenticated.
bitfun_pull_relay_image() {
  local platform="$1" routes route_name repository image_ref selected=""
  local requested_mode github_speed mirror_first=0
  local pull_timeout="${BITFUN_IMAGE_PULL_TIMEOUT:-900}"
  case "$pull_timeout" in
    '' | *[!0-9]*) pull_timeout=900 ;;
  esac

  routes="$(mktemp)"
  requested_mode="${BITFUN_MIRROR_REQUESTED_MODE:-${BITFUN_MIRROR_MODE:-auto}}"
  case "$BITFUN_GITHUB_HEALTHY_BPS" in
    '' | *[!0-9]*) BITFUN_GITHUB_HEALTHY_BPS=524288 ;;
  esac
  case "$requested_mode" in
    cn) mirror_first=1 ;;
    global) mirror_first=0 ;;
    *)
      github_speed="$(bitfun_probe_github_throughput "$platform")"
      if [ "$github_speed" -lt "$BITFUN_GITHUB_HEALTHY_BPS" ]; then
        mirror_first=1
      fi
      ;;
  esac

  if [ "$requested_mode" = global ]; then
    printf '%s\t%s\n' "official GHCR" "$BITFUN_RELAY_IMAGE" >"$routes"
  elif [ "$mirror_first" = "1" ]; then
    printf '%s\t%s\n' \
      "NJU GHCR accelerator" "ghcr.nju.edu.cn/${BITFUN_RELAY_IMAGE#ghcr.io/}" \
      "DaoCloud GHCR accelerator" "m.daocloud.io/${BITFUN_RELAY_IMAGE}" \
      "official GHCR fallback" "$BITFUN_RELAY_IMAGE" >"$routes"
  else
    printf '%s\t%s\n' \
      "official GHCR" "$BITFUN_RELAY_IMAGE" \
      "NJU GHCR accelerator fallback" "ghcr.nju.edu.cn/${BITFUN_RELAY_IMAGE#ghcr.io/}" \
      "DaoCloud GHCR accelerator fallback" "m.daocloud.io/${BITFUN_RELAY_IMAGE}" >"$routes"
  fi

  while IFS=$'\t' read -r route_name repository; do
    [ -n "$repository" ] || continue
    image_ref="$(bitfun_relay_image_ref "$repository")"
    echo ">>> Pulling Relay image via ${route_name}: ${image_ref}" >&2
    if bitfun_image_docker_with_timeout "$pull_timeout" pull --platform "$platform" "$image_ref" >&2; then
      selected="$image_ref"
      break
    fi
    echo ">>> ${route_name} failed or timed out; trying the next route." >&2
  done <"$routes"
  rm -f "$routes"

  if [ -z "$selected" ]; then
    echo ">>> ERROR: Relay image pull failed on every ${BITFUN_MIRROR_MODE:-global} route." >&2
    return 1
  fi

  local expected_arch="${platform#linux/}" actual_arch
  actual_arch="$(bitfun_image_docker image inspect -f '{{.Architecture}}' "$selected" 2>/dev/null || true)"
  if [ "$actual_arch" != "$expected_arch" ]; then
    echo ">>> ERROR: pulled image architecture '$actual_arch' does not match '$expected_arch'." >&2
    return 1
  fi
  printf '%s' "$selected"
}

bitfun_restore_previous_relay() {
  bitfun_image_docker rm -f bitfun-relay >/dev/null 2>&1 || true
  if [ -n "${BITFUN_RELAY_BACKUP_CONTAINER:-}" ]; then
    bitfun_image_docker rename "$BITFUN_RELAY_BACKUP_CONTAINER" bitfun-relay >/dev/null 2>&1 || true
    bitfun_image_docker start bitfun-relay >/dev/null 2>&1 || true
  fi
}

bitfun_run_relay_image() {
  local image_ref="$1" platform="$2" attempt stale
  bitfun_image_docker volume create relay-server_relay-db >/dev/null
  bitfun_image_docker volume create relay-server_room-web >/dev/null

  BITFUN_RELAY_BACKUP_CONTAINER=""
  if bitfun_image_docker container inspect bitfun-relay >/dev/null 2>&1; then
    BITFUN_RELAY_BACKUP_CONTAINER="bitfun-relay-before-image-$$"
    bitfun_image_docker stop bitfun-relay >/dev/null 2>&1 || true
    if ! bitfun_image_docker rename bitfun-relay "$BITFUN_RELAY_BACKUP_CONTAINER"; then
      echo ">>> ERROR: could not stage the existing Relay container." >&2
      bitfun_image_docker start bitfun-relay >/dev/null 2>&1 || true
      return 1
    fi
  fi

  # A cancelled wizard must put the previously healthy Relay back.
  trap 'bitfun_restore_previous_relay; trap - INT TERM; exit 1' INT TERM

  echo ">>> Starting Relay image on port ${RELAY_PORT:-9700}..."
  if ! bitfun_image_docker run -d \
    --name bitfun-relay \
    --platform "$platform" \
    --restart unless-stopped \
    --label com.docker.compose.project=relay-server \
    --label com.docker.compose.service=relay-server \
    --label "com.bitfun.relay.image=${BITFUN_RELAY_IMAGE}" \
    --label "com.bitfun.relay.digest=${BITFUN_RELAY_IMAGE_DIGEST:-unlocked}" \
    -p "${RELAY_HOST_BIND_IP:-0.0.0.0}:${RELAY_PORT:-9700}:${RELAY_PORT:-9700}" \
    -e "RELAY_PORT=${RELAY_PORT:-9700}" \
    -e RELAY_STATIC_DIR=/app/static \
    -e RELAY_ROOM_WEB_DIR=/app/room-web \
    -e RELAY_ROOM_TTL=300 \
    -e RELAY_ASSET_STORE_MAX_BYTES=1073741824 \
    -e RELAY_DB_PATH=/app/data/bitfun_relay.db \
    -e "RELAY_PAGE_PUBLIC_BASE_URL=${RELAY_PAGE_PUBLIC_BASE_URL:-}" \
    -e "RELAY_PAGE_AUTH_BASE_URL=${RELAY_PAGE_AUTH_BASE_URL:-}" \
    -v relay-server_room-web:/app/room-web \
    -v relay-server_relay-db:/app/data \
    "$image_ref" >/dev/null; then
    echo ">>> ERROR: the published Relay image could not start; restoring the previous container." >&2
    bitfun_restore_previous_relay
    trap - INT TERM
    return 1
  fi

  # Probe inside the image. This avoids requiring curl on an otherwise ready
  # Docker host and verifies the exact container that will be kept.
  for attempt in $(seq 1 30); do
    if bitfun_image_docker exec bitfun-relay \
      curl -fsS --max-time 3 "http://127.0.0.1:${RELAY_PORT:-9700}/health" >/dev/null 2>&1; then
      trap - INT TERM
      if [ -n "$BITFUN_RELAY_BACKUP_CONTAINER" ]; then
        bitfun_image_docker rm "$BITFUN_RELAY_BACKUP_CONTAINER" >/dev/null 2>&1 || true
      fi
      for stale in $(bitfun_image_docker ps -aq \
        --filter 'name=^bitfun-relay-before-image-' \
        --filter 'name=^bitfun-relay-before-release-' 2>/dev/null); do
        bitfun_image_docker rm -f "$stale" >/dev/null 2>&1 || true
      done
      echo ">>> Published Relay image is healthy."
      return 0
    fi
    if ! bitfun_image_docker inspect -f '{{.State.Running}}' bitfun-relay 2>/dev/null | grep -qx true; then
      break
    fi
    sleep 2
  done

  echo ">>> ERROR: published Relay image failed its health check; restoring the previous container." >&2
  echo ">>> Container state: $(bitfun_image_docker inspect \
    -f 'running={{.State.Running}} exit={{.State.ExitCode}} oom={{.State.OOMKilled}} err={{.State.Error}}' \
    bitfun-relay 2>&1 || true)"
  echo ">>> Last 40 log lines from bitfun-relay:"
  bitfun_image_docker logs --tail 40 bitfun-relay 2>&1 | sed 's/^/    /' || true
  bitfun_restore_previous_relay
  trap - INT TERM
  return 1
}

bitfun_try_release_deploy() {
  local platform image_ref
  platform="$(bitfun_relay_native_platform)" || {
    echo ">>> ERROR: no published Relay image for architecture $(uname -m)." >&2
    return 1
  }

  if [ -n "${BITFUN_RELAY_IMAGE_DIGEST:-}" ] && \
     [[ ! "$BITFUN_RELAY_IMAGE_DIGEST" =~ ^sha256:[0-9a-f]{64}$ ]]; then
    echo ">>> ERROR: invalid Relay image digest; refusing to pull executable code." >&2
    return 1
  fi
  if [ -n "${BITFUN_REQUIRE_IMAGE_DIGEST:-}" ] && [ -z "${BITFUN_RELAY_IMAGE_DIGEST:-}" ]; then
    echo ">>> ERROR: a signed Relay image descriptor is required for one-click deployment." >&2
    return 1
  fi

  # Ignore a user-level foreign-platform default. Relay one-click deployment is
  # deliberately native on its two supported server architectures.
  export DOCKER_DEFAULT_PLATFORM="$platform"
  image_ref="$(bitfun_pull_relay_image "$platform")" || return 1
  bitfun_run_relay_image "$image_ref" "$platform"
}

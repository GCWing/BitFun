#!/usr/bin/env bash

# Start every published Relay platform under the runner's native/QEMU runtime
# and require the image's own HEALTHCHECK to pass before release metadata is
# signed. Usage: smoke-image.sh <repository>@sha256:<multi-platform-digest>

set -euo pipefail

IMAGE_REF="${1:?usage: smoke-image.sh <repository>@sha256:<digest>}"
if [[ ! "$IMAGE_REF" =~ @sha256:[0-9a-f]{64}$ ]]; then
  echo "ERROR: Relay smoke test requires a digest-pinned image." >&2
  exit 2
fi

containers=()
cleanup() {
  local container
  for container in "${containers[@]}"; do
    docker rm -fv "$container" >/dev/null 2>&1 || true
  done
  docker image rm "$IMAGE_REF" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM

for arch in amd64 arm64; do
  container="openbitfun-relay-smoke-${arch}"
  containers+=("$container")
  echo ">>> Smoke-testing ${IMAGE_REF} on linux/${arch}..."
  docker run -d \
    --name "$container" \
    --platform "linux/${arch}" \
    -e RELAY_PORT=9700 \
    -e RELAY_STATIC_DIR=/app/static \
    -e RELAY_ROOM_WEB_DIR=/app/room-web \
    -e RELAY_DB_PATH=/app/data/openbitfun_relay.db \
    "$IMAGE_REF" >/dev/null

  healthy=0
  for _attempt in $(seq 1 45); do
    status="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container")"
    if [ "$status" = "healthy" ]; then
      healthy=1
      break
    fi
    if [ "$status" = "exited" ] || [ "$status" = "dead" ]; then
      break
    fi
    sleep 2
  done

  if [ "$healthy" != "1" ]; then
    echo "ERROR: linux/${arch} Relay image did not become healthy." >&2
    docker inspect -f 'status={{.State.Status}} exit={{.State.ExitCode}} oom={{.State.OOMKilled}} error={{.State.Error}}' "$container" >&2 || true
    docker logs --tail 80 "$container" >&2 || true
    exit 1
  fi
  docker rm -fv "$container" >/dev/null
  # Docker's classic image store cannot resolve one multi-platform digest to
  # amd64 and then overwrite that local resolution with arm64. Drop the first
  # platform before pulling the next so both runs are independent.
  docker image rm "$IMAGE_REF" >/dev/null 2>&1 || true
  echo ">>> linux/${arch} Relay image is healthy."
done

trap - EXIT INT TERM

#!/usr/bin/env bash
# Persistent musl build container for Harbor-compatible bitfun-cli.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DOCKERFILE="${ROOT}/scripts/harbor-build-musl/Dockerfile"
IMAGE="${BITFUN_HARBOR_MUSL_IMAGE:-bitfun-harbor-musl-build:bookworm}"
CONTAINER="${BITFUN_HARBOR_MUSL_CONTAINER:-bitfun-harbor-musl-build}"
REGISTRY_VOLUME="${BITFUN_HARBOR_MUSL_REGISTRY_VOLUME:-bitfun-harbor-musl-cargo-registry}"
GIT_VOLUME="${BITFUN_HARBOR_MUSL_GIT_VOLUME:-bitfun-harbor-musl-cargo-git}"
TARGET_TRIPLE="x86_64-unknown-linux-musl"
BINARY="${ROOT}/target/${TARGET_TRIPLE}/release/bitfun-cli"

usage() {
  cat <<EOF
Usage: $(basename "$0") <command>

Commands:
  build-image       Build (or rebuild) the persistent musl build image
  start             Create/start the long-running build container
  stop              Stop the build container
  restart           stop + start
  shell             Open an interactive shell in the build container
  compile           Run: cargo build --release -p bitfun-cli --target ${TARGET_TRIPLE}
  test-binary       Run the built binary in Ubuntu and Alpine containers
  compile-and-test  compile + test-binary
  status            Show container/image/binary status
  logs              Follow container logs (usually empty for sleep infinity)

Environment overrides:
  BITFUN_HARBOR_MUSL_IMAGE, BITFUN_HARBOR_MUSL_CONTAINER
  BITFUN_HARBOR_MUSL_REGISTRY_VOLUME, BITFUN_HARBOR_MUSL_GIT_VOLUME

Output binary:
  ${BINARY}

Harbor mount example:
  {"type":"bind","source":"${BINARY}","target":"/usr/local/bin/bitfun-cli","read_only":true}
EOF
}

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    echo "error: docker not found" >&2
    exit 1
  fi
}

container_running() {
  docker inspect -f '{{.State.Running}}' "${CONTAINER}" 2>/dev/null | grep -q true
}

container_exists() {
  docker inspect "${CONTAINER}" >/dev/null 2>&1
}

docker_exec() {
  if [[ -t 0 && -t 1 ]]; then
    docker exec -it "${CONTAINER}" "$@"
  else
    docker exec "${CONTAINER}" "$@"
  fi
}

cmd_build_image() {
  docker build -f "${DOCKERFILE}" -t "${IMAGE}" "${ROOT}"
  echo "Built image: ${IMAGE}"
}

cmd_start() {
  docker volume create "${REGISTRY_VOLUME}" >/dev/null
  docker volume create "${GIT_VOLUME}" >/dev/null

  if container_exists; then
    if container_running; then
      echo "Container already running: ${CONTAINER}"
      return 0
    fi
    docker start "${CONTAINER}" >/dev/null
    echo "Started existing container: ${CONTAINER}"
    return 0
  fi

  cmd_build_image
  docker run -d \
    --name "${CONTAINER}" \
    -v "${ROOT}:/src" \
    -v "${REGISTRY_VOLUME}:/usr/local/cargo/registry" \
    -v "${GIT_VOLUME}:/usr/local/cargo/git" \
    -w /src \
    "${IMAGE}" \
    sleep infinity >/dev/null

  echo "Created and started container: ${CONTAINER}"
  echo "  source mount : ${ROOT} -> /src"
  echo "  cargo registry: volume ${REGISTRY_VOLUME}"
  echo "  cargo git     : volume ${GIT_VOLUME}"
}

cmd_stop() {
  if container_exists; then
    docker stop "${CONTAINER}" >/dev/null || true
    echo "Stopped: ${CONTAINER}"
  else
    echo "Container not found: ${CONTAINER}"
  fi
}

cmd_shell() {
  cmd_start
  docker exec -it "${CONTAINER}" bash
}

cmd_compile() {
  cmd_start
  docker_exec bash -lc "cargo build --release -p bitfun-cli --target ${TARGET_TRIPLE}"
  echo "Binary: ${BINARY}"
}

cmd_test_binary() {
  if [[ ! -x "${BINARY}" ]]; then
    echo "error: binary not found or not executable: ${BINARY}" >&2
    echo "run: $(basename "$0") compile" >&2
    exit 1
  fi

  echo "Host binary:"
  file "${BINARY}"
  ldd "${BINARY}" || true

  echo
  echo "Ubuntu smoke test:"
  docker run --rm \
    -v "${BINARY}:/usr/local/bin/bitfun-cli:ro" \
    ubuntu:22.04 \
    /usr/local/bin/bitfun-cli --version

  echo
  echo "Alpine smoke test:"
  docker run --rm \
    -v "${BINARY}:/usr/local/bin/bitfun-cli:ro" \
    alpine:3.20 \
    /usr/local/bin/bitfun-cli --version
}

cmd_status() {
  echo "Image: ${IMAGE}"
  docker image inspect "${IMAGE}" --format '  created: {{.Created}}' 2>/dev/null || echo "  (image not built yet)"
  echo "Container: ${CONTAINER}"
  if container_exists; then
    docker inspect "${CONTAINER}" --format '  status: {{.State.Status}}'
    docker inspect "${CONTAINER}" --format '  started: {{.State.StartedAt}}'
  else
    echo "  status: not created"
  fi
  echo "Volumes:"
  echo "  ${REGISTRY_VOLUME}"
  echo "  ${GIT_VOLUME}"
  echo "Binary:"
  if [[ -e "${BINARY}" ]]; then
    ls -lh "${BINARY}"
  else
    echo "  ${BINARY} (not built yet)"
  fi
}

cmd_logs() {
  docker logs -f "${CONTAINER}"
}

main() {
  require_docker
  local cmd="${1:-}"
  case "${cmd}" in
    build-image) cmd_build_image ;;
    start) cmd_start ;;
    stop) cmd_stop ;;
    restart) cmd_stop; cmd_start ;;
    shell) cmd_shell ;;
    compile) cmd_compile ;;
    test-binary) cmd_test_binary ;;
    compile-and-test) cmd_compile; cmd_test_binary ;;
    status) cmd_status ;;
    logs) cmd_logs ;;
    -h|--help|help|"") usage ;;
    *)
      echo "error: unknown command: ${cmd}" >&2
      usage
      exit 1
      ;;
  esac
}

main "$@"

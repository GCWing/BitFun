#!/usr/bin/env bash

set -euo pipefail

if [ "$#" -lt 2 ] || [ "$#" -gt 4 ]; then
  echo "Usage: package-unix.sh <version> <target> [release-dir] [output-dir]" >&2
  exit 2
fi

VERSION="$1"
TARGET="$2"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
RELEASE_DIR="${3:-${REPO_ROOT}/target/${TARGET}/release}"
OUTPUT_DIR="${4:-${REPO_ROOT}}"
SERVER="${RELEASE_DIR}/openbitfun-relay-server"
ADMIN="${RELEASE_DIR}/relay-admin"
STAGE_NAME="openbitfun-relay-server-${VERSION}-${TARGET}"
STAGE_DIR="${OUTPUT_DIR}/dist-relay/${STAGE_NAME}"
# Keep the release asset name stable so Desktop can use GitHub's
# releases/<tag>/download URL without querying the GitHub API for a versioned name.
ARCHIVE_NAME="openbitfun-relay-server-${TARGET}.tar.gz"
ARCHIVE="${OUTPUT_DIR}/${ARCHIVE_NAME}"

smoke_server() {
  local executable="$1"
  local temp_dir port pid log_contents result
  temp_dir="$(mktemp -d)"
  port=19700
  mkdir -p "$temp_dir/room-web"
  RELAY_PORT="$port" \
    RELAY_DB_PATH="$temp_dir/relay.db" \
    RELAY_ROOM_WEB_DIR="$temp_dir/room-web" \
    "$executable" >"$temp_dir/server.log" 2>&1 &
  pid=$!
  result=1
  local attempt
  for attempt in $(seq 1 30); do
    if curl -fsS --max-time 2 "http://127.0.0.1:${port}/health" \
      2>/dev/null | grep -F "\"version\":\"${VERSION%%+*}\"" >/dev/null; then
      result=0
      break
    fi
    if ! kill -0 "$pid" >/dev/null 2>&1; then
      echo "Error: relay server exited during smoke test" >&2
      break
    fi
    sleep 1
  done

  log_contents="$(cat "$temp_dir/server.log")"
  kill "$pid" >/dev/null 2>&1 || true
  wait "$pid" >/dev/null 2>&1 || true
  rm -rf "$temp_dir"
  if [ "$result" -ne 0 ]; then
    echo "Error: relay server health check failed" >&2
    printf '%s\n' "$log_contents" >&2
  fi
  return "$result"
}

"$ADMIN" --help >/dev/null
smoke_server "$SERVER"

rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"
cp "$SERVER" "$ADMIN" "$STAGE_DIR/"
cp "${REPO_ROOT}/LICENSE" "$STAGE_DIR/" 2>/dev/null || true
cp "${REPO_ROOT}/src/apps/relay-server/README.md" "$STAGE_DIR/README.md"
cp "${REPO_ROOT}/README.md" "$STAGE_DIR/PROJECT-README.md"
cp -R "${REPO_ROOT}/src/apps/relay-server/static" "$STAGE_DIR/static"

tar -C "$(dirname "$STAGE_DIR")" -czf "$ARCHIVE" "$(basename "$STAGE_DIR")"

if command -v sha256sum >/dev/null 2>&1; then
  ARCHIVE_HASH="$(sha256sum "$ARCHIVE" | awk '{print $1}')"
  printf '%s  %s\n' "$ARCHIVE_HASH" "$ARCHIVE_NAME" >"${ARCHIVE}.sha256"
  (cd "$OUTPUT_DIR" && sha256sum -c "${ARCHIVE_NAME}.sha256")
else
  ARCHIVE_HASH="$(shasum -a 256 "$ARCHIVE" | awk '{print $1}')"
  printf '%s  %s\n' "$ARCHIVE_HASH" "$ARCHIVE_NAME" >"${ARCHIVE}.sha256"
  (cd "$OUTPUT_DIR" && shasum -a 256 -c "${ARCHIVE_NAME}.sha256")
fi

EXTRACT_DIR="$(mktemp -d)"
trap 'rm -rf "$EXTRACT_DIR"' EXIT
tar -xzf "$ARCHIVE" -C "$EXTRACT_DIR"

EXTRACTED="$EXTRACT_DIR/$STAGE_NAME"
[ -x "$EXTRACTED/openbitfun-relay-server" ]
[ -x "$EXTRACTED/relay-admin" ]
[ -f "$EXTRACTED/static/index.html" ]
"$EXTRACTED/relay-admin" --help >/dev/null
smoke_server "$EXTRACTED/openbitfun-relay-server"

if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "archive=$ARCHIVE_NAME" >>"$GITHUB_OUTPUT"
  echo "checksum=${ARCHIVE_NAME}.sha256" >>"$GITHUB_OUTPUT"
fi

echo "Packaged and verified: $ARCHIVE"

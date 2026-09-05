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
PRIMARY="${RELEASE_DIR}/openbitfun"
PLUGIN_HOST_DIST="${REPO_ROOT}/src/apps/extension-host/dist"
PLUGIN_HOST_RESOURCE_DIR="resources/ext-host"

assert_plugin_host_resources() {
  local directory="$1"
  for entry in extension-host.js; do
    if [ ! -f "${directory}/${entry}" ]; then
      echo "Error: plugin Host resource is missing: ${directory}/${entry}" >&2
      return 1
    fi
  done
}

"$PRIMARY" --version
"$PRIMARY" --help >/dev/null
assert_plugin_host_resources "$PLUGIN_HOST_DIST"

STAGE_NAME="openbitfun-cli-${VERSION}-${TARGET}"
STAGE_DIR="${OUTPUT_DIR}/dist-cli/${STAGE_NAME}"
mkdir -p "$STAGE_DIR"
cp "$PRIMARY" "$STAGE_DIR/"
cp "${REPO_ROOT}/LICENSE" "$STAGE_DIR/" 2>/dev/null || true
cp "${REPO_ROOT}/src/apps/cli/README.md" "$STAGE_DIR/README.md"
cp "${REPO_ROOT}/README.md" "$STAGE_DIR/PROJECT-README.md"
cp "${REPO_ROOT}/THIRD_PARTY_NOTICES.md" "$STAGE_DIR/THIRD_PARTY_NOTICES.md"
mkdir -p "$STAGE_DIR/third-party/models.dev"
cp "${REPO_ROOT}/src/crates/services/services-integrations/assets/models-dev.LICENSE.txt" \
  "$STAGE_DIR/third-party/models.dev/LICENSE.txt"
cp "${REPO_ROOT}/src/crates/services/services-integrations/assets/models-dev.provenance.json" \
  "$STAGE_DIR/third-party/models.dev/provenance.json"

if [ -d "${REPO_ROOT}/src/apps/cli/themes" ]; then
  cp -R "${REPO_ROOT}/src/apps/cli/themes" "$STAGE_DIR/themes"
fi
if [ -d "${REPO_ROOT}/src/apps/cli/prompts" ]; then
  cp -R "${REPO_ROOT}/src/apps/cli/prompts" "$STAGE_DIR/prompts"
fi
mkdir -p "$STAGE_DIR/$PLUGIN_HOST_RESOURCE_DIR"
cp "$PLUGIN_HOST_DIST/extension-host.js" "$STAGE_DIR/$PLUGIN_HOST_RESOURCE_DIR/"
assert_plugin_host_resources "$STAGE_DIR/$PLUGIN_HOST_RESOURCE_DIR"

ARCHIVE="${OUTPUT_DIR}/${STAGE_NAME}.tar.gz"
tar -C "$(dirname "$STAGE_DIR")" -czf "$ARCHIVE" "$(basename "$STAGE_DIR")"

if command -v sha256sum >/dev/null 2>&1; then
  ARCHIVE_HASH="$(sha256sum "$ARCHIVE" | awk '{print $1}')"
  printf '%s  %s\n' "$ARCHIVE_HASH" "$(basename "$ARCHIVE")" >"${ARCHIVE}.sha256"
  (cd "$OUTPUT_DIR" && sha256sum -c "$(basename "${ARCHIVE}.sha256")")
else
  ARCHIVE_HASH="$(shasum -a 256 "$ARCHIVE" | awk '{print $1}')"
  printf '%s  %s\n' "$ARCHIVE_HASH" "$(basename "$ARCHIVE")" >"${ARCHIVE}.sha256"
  (cd "$OUTPUT_DIR" && shasum -a 256 -c "$(basename "${ARCHIVE}.sha256")")
fi

EXTRACT_DIR="$(mktemp -d)"
trap 'rm -rf "$EXTRACT_DIR"' EXIT
tar -xzf "$ARCHIVE" -C "$EXTRACT_DIR"

shopt -s nullglob
PRIMARY_CANDIDATES=("$EXTRACT_DIR"/*/openbitfun)
[ "${#PRIMARY_CANDIDATES[@]}" -eq 1 ]
[ -f "$EXTRACT_DIR/$STAGE_NAME/README.md" ]
[ -f "$EXTRACT_DIR/$STAGE_NAME/PROJECT-README.md" ]
[ -f "$EXTRACT_DIR/$STAGE_NAME/THIRD_PARTY_NOTICES.md" ]
[ -f "$EXTRACT_DIR/$STAGE_NAME/third-party/models.dev/LICENSE.txt" ]
[ -f "$EXTRACT_DIR/$STAGE_NAME/third-party/models.dev/provenance.json" ]
assert_plugin_host_resources "$EXTRACT_DIR/$STAGE_NAME/$PLUGIN_HOST_RESOURCE_DIR"
"${PRIMARY_CANDIDATES[0]}" --version
"${PRIMARY_CANDIDATES[0]}" --help >/dev/null

if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "archive=$(basename "$ARCHIVE")" >>"$GITHUB_OUTPUT"
  echo "checksum=$(basename "$ARCHIVE").sha256" >>"$GITHUB_OUTPUT"
fi

echo "Packaged and verified: $ARCHIVE"

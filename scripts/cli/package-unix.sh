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
PRIMARY="${RELEASE_DIR}/bitfun"
LEGACY="${RELEASE_DIR}/bitfun-cli"
DEPRECATION='Warning: `bitfun-cli` is deprecated; use `bitfun` instead.'

assert_legacy_entrypoint() {
  local executable="$1"
  local stderr_file warning
  stderr_file="$(mktemp)"
  if ! "$executable" --version >/dev/null 2>"$stderr_file"; then
    rm -f "$stderr_file"
    echo "Error: deprecated bitfun-cli entrypoint failed" >&2
    return 1
  fi
  warning="$(cat "$stderr_file")"
  rm -f "$stderr_file"
  if [ "$warning" != "$DEPRECATION" ]; then
    echo "Error: unexpected deprecated entrypoint warning: $warning" >&2
    return 1
  fi
}

"$PRIMARY" --version
"$PRIMARY" --help >/dev/null
assert_legacy_entrypoint "$LEGACY"

STAGE_NAME="bitfun-cli-${VERSION}-${TARGET}"
STAGE_DIR="${OUTPUT_DIR}/dist-cli/${STAGE_NAME}"
mkdir -p "$STAGE_DIR"
cp "$PRIMARY" "$STAGE_DIR/"
cp "$LEGACY" "$STAGE_DIR/"
cp "${REPO_ROOT}/LICENSE" "$STAGE_DIR/" 2>/dev/null || true
cp "${REPO_ROOT}/src/apps/cli/README.md" "$STAGE_DIR/README.md"
cp "${REPO_ROOT}/README.md" "$STAGE_DIR/PROJECT-README.md"

if [ -d "${REPO_ROOT}/src/apps/cli/themes" ]; then
  cp -R "${REPO_ROOT}/src/apps/cli/themes" "$STAGE_DIR/themes"
fi
if [ -d "${REPO_ROOT}/src/apps/cli/prompts" ]; then
  cp -R "${REPO_ROOT}/src/apps/cli/prompts" "$STAGE_DIR/prompts"
fi

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
PRIMARY_CANDIDATES=("$EXTRACT_DIR"/*/bitfun)
LEGACY_CANDIDATES=("$EXTRACT_DIR"/*/bitfun-cli)
[ "${#PRIMARY_CANDIDATES[@]}" -eq 1 ]
[ "${#LEGACY_CANDIDATES[@]}" -eq 1 ]
[ -f "$EXTRACT_DIR/$STAGE_NAME/README.md" ]
[ -f "$EXTRACT_DIR/$STAGE_NAME/PROJECT-README.md" ]
"${PRIMARY_CANDIDATES[0]}" --version
"${PRIMARY_CANDIDATES[0]}" --help >/dev/null
assert_legacy_entrypoint "${LEGACY_CANDIDATES[0]}"

if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "archive=$(basename "$ARCHIVE")" >>"$GITHUB_OUTPUT"
  echo "checksum=$(basename "$ARCHIVE").sha256" >>"$GITHUB_OUTPUT"
fi

echo "Packaged and verified: $ARCHIVE"

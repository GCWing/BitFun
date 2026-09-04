#!/usr/bin/env bash

set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "Usage: test-install-unix.sh <target>" >&2
  exit 2
fi

TARGET="$1"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TEST_ROOT="$(mktemp -d)"
trap 'rm -rf "$TEST_ROOT"' EXIT

ORIGINAL_HOME="$HOME"
export RUSTUP_HOME="${RUSTUP_HOME:-${ORIGINAL_HOME}/.rustup}"
export CARGO_HOME="${CARGO_HOME:-${ORIGINAL_HOME}/.cargo}"
export HOME="${TEST_ROOT}/home"
export OPENBITFUN_CLI_BIN_DIR="${TEST_ROOT}/bin"
export CARGO_BUILD_TARGET="$TARGET"
mkdir -p "$HOME"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

bash "${REPO_ROOT}/src/apps/cli/install.sh"
bash "${REPO_ROOT}/src/apps/cli/install.sh"

"${OPENBITFUN_CLI_BIN_DIR}/openbitfun" --version >/dev/null
[ -f "${OPENBITFUN_CLI_BIN_DIR}/resources/ext-host/extension-host.js" ]

for rc_file in "$HOME/.bashrc" "$HOME/.zshrc"; do
  [ "$(grep -Fc '# >>> OpenBitFun CLI PATH (managed by src/apps/cli/install.sh) >>>' "$rc_file")" -eq 1 ]
done

PRIMARY_HASH="$(hash_file "${OPENBITFUN_CLI_BIN_DIR}/openbitfun")"
REAL_MV="$(command -v mv)"
SHIM_DIR="${TEST_ROOT}/shim"
mkdir -p "$SHIM_DIR"
cat >"${SHIM_DIR}/mv" <<'EOF'
#!/bin/sh
if [ "${1:-}" = "${OPENBITFUN_CLI_TEST_FAIL_SOURCE:-}" ]; then
  exit 91
fi
exec "${OPENBITFUN_CLI_TEST_REAL_MV}" "$@"
EOF
chmod +x "${SHIM_DIR}/mv"

if PATH="${SHIM_DIR}:$PATH" \
  OPENBITFUN_CLI_SKIP_SHELLRC=1 \
  OPENBITFUN_CLI_TEST_REAL_MV="$REAL_MV" \
  OPENBITFUN_CLI_TEST_FAIL_SOURCE="${OPENBITFUN_CLI_BIN_DIR}/openbitfun" \
  bash "${REPO_ROOT}/src/apps/cli/install.sh"; then
  echo "Error: installer unexpectedly succeeded during injected replacement failure" >&2
  exit 1
fi

[ "$(hash_file "${OPENBITFUN_CLI_BIN_DIR}/openbitfun")" = "$PRIMARY_HASH" ]

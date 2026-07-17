#!/usr/bin/env bash
# BitFun CLI — one-click build + install into PATH.
#
# Usage (from anywhere inside the repo, or this directory):
#   bash src/apps/cli/install.sh
#   bash install.sh                 # when cwd is src/apps/cli
#
# What it does:
#   1. cargo build -p bitfun-cli --release (native host arch)
#   2. Install binary to ~/.local/bin/bitfun-cli (override with BITFUN_CLI_BIN_DIR)
#   3. Idempotently append a PATH block to ~/.bashrc and ~/.zshrc
#   4. Source the matching rc for the current shell when interactive
#
# Supported hosts: Linux/macOS on amd64 (x86_64) and arm64 (aarch64).
#
# Environment:
#   BITFUN_CLI_BIN_DIR       Install directory (default: ~/.local/bin)
#   BITFUN_CLI_SKIP_SHELLRC  Set to 1 to skip bashrc/zshrc edits
#   CARGO_TARGET_DIR         Optional cargo target dir (e.g. $HOME/bitfun-build/target)
#   CARGO_BUILD_JOBS         Optional rustc parallelism limit for small VPS

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ── Resolve repository root (directory that owns the workspace Cargo.toml) ──
resolve_repo_root() {
  local dir candid
  dir="$SCRIPT_DIR"
  while [ "$dir" != "/" ]; do
    if [ -f "$dir/Cargo.toml" ] && [ -d "$dir/src/apps/cli" ]; then
      # Prefer the workspace root that lists members / workspace deps.
      if grep -q '^\[workspace\]' "$dir/Cargo.toml" 2>/dev/null; then
        echo "$dir"
        return 0
      fi
      candid="$dir"
    fi
    dir="$(dirname "$dir")"
  done
  if [ -n "${candid:-}" ]; then
    echo "$candid"
    return 0
  fi
  echo "Error: could not locate BitFun repository root from $SCRIPT_DIR" >&2
  exit 1
}

host_arch_label() {
  local m
  m="$(uname -m 2>/dev/null || echo unknown)"
  case "$m" in
    x86_64 | amd64) echo "amd64" ;;
    aarch64 | arm64) echo "arm64" ;;
    *) echo "$m" ;;
  esac
}

assert_supported_host() {
  local os arch
  os="$(uname -s 2>/dev/null || echo unknown)"
  arch="$(host_arch_label)"
  case "$os" in
    Linux | Darwin) ;;
    *)
      echo "Error: unsupported OS '$os'. install.sh supports Linux and macOS."
      exit 1
      ;;
  esac
  case "$arch" in
    amd64 | arm64) ;;
    *)
      echo "Error: unsupported CPU '$arch' ($(uname -m))."
      echo "install.sh supports amd64 (x86_64) and arm64 (aarch64)."
      exit 1
      ;;
  esac
}

require_cargo() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo not found. Install Rust from https://rustup.rs and re-run."
    exit 1
  fi
  if ! command -v rustc >/dev/null 2>&1; then
    echo "Error: rustc not found. Install Rust from https://rustup.rs and re-run."
    exit 1
  fi
}

# Marker keeps shellrc edits idempotent across re-installs.
SHELLRC_MARKER_BEGIN="# >>> BitFun CLI PATH (managed by src/apps/cli/install.sh) >>>"
SHELLRC_MARKER_END="# <<< BitFun CLI PATH (managed by src/apps/cli/install.sh) <<<"

ensure_bin_dir_on_path_block() {
  local bin_dir="$1"
  cat <<EOF
${SHELLRC_MARKER_BEGIN}
# Added so \`bitfun-cli\` is available in new shells after install.sh.
case ":\$PATH:" in
  *":${bin_dir}:"*) ;;
  *) export PATH="${bin_dir}:\$PATH" ;;
esac
${SHELLRC_MARKER_END}
EOF
}

upsert_shellrc_path() {
  local rc_file="$1"
  local bin_dir="$2"
  local tmp block
  mkdir -p "$(dirname "$rc_file")"
  touch "$rc_file"

  block="$(ensure_bin_dir_on_path_block "$bin_dir")"

  if grep -Fq "$SHELLRC_MARKER_BEGIN" "$rc_file" 2>/dev/null; then
    tmp="$(mktemp)"
    # Replace existing managed block.
    awk -v begin="$SHELLRC_MARKER_BEGIN" -v end="$SHELLRC_MARKER_END" '
      $0 == begin { in_block=1; next }
      $0 == end { in_block=0; next }
      !in_block { print }
    ' "$rc_file" >"$tmp"
    printf '\n%s\n' "$block" >>"$tmp"
    mv "$tmp" "$rc_file"
    echo "Updated PATH block in $rc_file"
  else
    printf '\n%s\n' "$block" >>"$rc_file"
    echo "Appended PATH block to $rc_file"
  fi
}

maybe_source_shellrc() {
  local bin_dir="$1"
  # Always export for the remainder of this install.sh process.
  case ":${PATH}:" in
    *":${bin_dir}:"*) ;;
    *) export PATH="${bin_dir}:$PATH" ;;
  esac

  if [ ! -t 0 ] || [ ! -t 1 ]; then
    echo "Non-interactive shell: open a new terminal, or run:"
    echo "  export PATH=\"${bin_dir}:\$PATH\""
    return 0
  fi

  local shell_name
  shell_name="$(basename "${SHELL:-}")"
  case "$shell_name" in
    zsh)
      # shellcheck disable=SC1090
      source "${HOME}/.zshrc" 2>/dev/null || true
      echo "Sourced ~/.zshrc for this session."
      ;;
    bash)
      # shellcheck disable=SC1090
      source "${HOME}/.bashrc" 2>/dev/null || true
      echo "Sourced ~/.bashrc for this session."
      ;;
    *)
      echo "Current SHELL=${SHELL:-unknown}: PATH updated for this install process."
      echo "For new terminals, ensure ${bin_dir} is on PATH (bashrc/zshrc were updated)."
      ;;
  esac
}

usage() {
  cat <<'EOF'
BitFun CLI install script

Usage:
  bash install.sh [--help]

Builds a release bitfun-cli and installs it for interactive use.

Options:
  -h, --help    Show this help

Environment:
  BITFUN_CLI_BIN_DIR       Install directory (default: ~/.local/bin)
  BITFUN_CLI_SKIP_SHELLRC  Set to 1 to skip ~/.bashrc and ~/.zshrc edits
  CARGO_TARGET_DIR         Cargo target directory override
  CARGO_BUILD_JOBS         Limit rustc parallelism (useful on small VPS)
EOF
}

for arg in "$@"; do
  case "$arg" in
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

REPO_ROOT="$(resolve_repo_root)"
BIN_DIR="${BITFUN_CLI_BIN_DIR:-${HOME}/.local/bin}"
HOST_ARCH="$(host_arch_label)"
HOST_OS="$(uname -s)"

echo "=== BitFun CLI Install ==="
echo "Repo:   $REPO_ROOT"
echo "Host:   ${HOST_OS} / ${HOST_ARCH} ($(uname -m))"
echo "Install dir: $BIN_DIR"

assert_supported_host
require_cargo

mkdir -p "$BIN_DIR"

echo ""
echo "[1/3] Building bitfun-cli (release)..."
cd "$REPO_ROOT"
# Build from workspace root so path deps resolve.
cargo build -p bitfun-cli --release

TARGET_DIR="${CARGO_TARGET_DIR:-${REPO_ROOT}/target}"
BUILT_BIN="${TARGET_DIR}/release/bitfun-cli"
if [ ! -x "$BUILT_BIN" ]; then
  echo "Error: built binary not found at $BUILT_BIN"
  exit 1
fi

echo ""
echo "[2/3] Installing binary..."
install -m 755 "$BUILT_BIN" "${BIN_DIR}/bitfun-cli"
echo "Installed: ${BIN_DIR}/bitfun-cli"
"${BIN_DIR}/bitfun-cli" --version 2>/dev/null || "${BIN_DIR}/bitfun-cli" -V 2>/dev/null || true

echo ""
echo "[3/3] Configuring shell PATH..."
if [ "${BITFUN_CLI_SKIP_SHELLRC:-0}" = "1" ]; then
  echo "Skipped shell rc edits (BITFUN_CLI_SKIP_SHELLRC=1)."
  case ":${PATH}:" in
    *":${BIN_DIR}:"*) ;;
    *) export PATH="${BIN_DIR}:$PATH" ;;
  esac
else
  upsert_shellrc_path "${HOME}/.bashrc" "$BIN_DIR"
  upsert_shellrc_path "${HOME}/.zshrc" "$BIN_DIR"
  maybe_source_shellrc "$BIN_DIR"
fi

echo ""
echo "=== Install complete ==="
echo "Run:  bitfun-cli"
echo "Login (Peer Host): open /login inside the TUI after start."
echo "Server use: after /login, run \`bitfun-cli daemon install\` to keep this"
echo "device reachable by your account even after exit or reboot."
if ! command -v bitfun-cli >/dev/null 2>&1; then
  echo ""
  echo "Note: \`bitfun-cli\` is not yet visible in this shell's command lookup."
  echo "Use one of:"
  echo "  hash -r && bitfun-cli"
  echo "  export PATH=\"${BIN_DIR}:\$PATH\" && bitfun-cli"
  echo "  ${BIN_DIR}/bitfun-cli"
fi

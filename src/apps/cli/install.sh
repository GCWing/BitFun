#!/usr/bin/env bash
# BitFun CLI — one-click build + install into PATH.
#
# Usage (from anywhere inside the repo, or this directory):
#   bash src/apps/cli/install.sh
#   bash install.sh                 # when cwd is src/apps/cli
#
# What it does:
#   1. cargo build -p bitfun-cli --release (native host arch)
#   2. Install bitfun plus the deprecated bitfun-cli compatibility entrypoint
#      to ~/.local/bin (override with BITFUN_CLI_BIN_DIR)
#   3. Idempotently append a PATH block to ~/.bashrc and ~/.zshrc
#   4. Print explicit current-shell and new-terminal instructions
#   5. Restart the account daemon's auto-start service when installed, so
#      upgrades take effect (a running daemon keeps the old binary otherwise)
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
# Added so \`bitfun\` is available in new shells after install.sh.
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

print_path_guidance() {
  local bin_dir="$1"
  echo "Open a new terminal, then run: bitfun"
  echo "Current shell: export PATH=\"${bin_dir}:\$PATH\" && bitfun"
  echo "Direct path: ${bin_dir}/bitfun"
}

assert_entrypoint_pair() {
  local primary="$1"
  local legacy="$2"
  local stderr_file warning

  "$primary" --version >/dev/null || return 1
  stderr_file="$(mktemp)" || return 1
  if ! "$legacy" --version >/dev/null 2>"$stderr_file"; then
    rm -f "$stderr_file"
    return 1
  fi
  warning="$(cat "$stderr_file")"
  rm -f "$stderr_file"
  if [ "$warning" != 'Warning: `bitfun-cli` is deprecated; use `bitfun` instead.' ]; then
    echo "Error: deprecated bitfun-cli entrypoint emitted an unexpected warning: $warning" >&2
    return 1
  fi
}

install_entrypoint_pair() {
  local primary_source="$1"
  local legacy_source="$2"
  local destination="$3"
  local stage_dir staged_primary staged_legacy primary_target legacy_target
  local primary_backup legacy_backup
  local primary_backed_up=0 legacy_backed_up=0 primary_committed=0 legacy_committed=0
  local failed=0

  mkdir -p "$destination"
  stage_dir="$(mktemp -d "${destination}/.bitfun-install.XXXXXX")"
  staged_primary="${stage_dir}/bitfun"
  staged_legacy="${stage_dir}/bitfun-cli"
  primary_target="${destination}/bitfun"
  legacy_target="${destination}/bitfun-cli"
  primary_backup="${stage_dir}/previous-bitfun"
  legacy_backup="${stage_dir}/previous-bitfun-cli"

  install -m 755 "$primary_source" "$staged_primary" || failed=1
  if [ "$failed" -eq 0 ]; then
    install -m 755 "$legacy_source" "$staged_legacy" || failed=1
  fi
  if [ "$failed" -eq 0 ]; then
    assert_entrypoint_pair "$staged_primary" "$staged_legacy" || failed=1
  fi
  if [ "$failed" -eq 0 ] && { [ -e "$primary_target" ] || [ -L "$primary_target" ]; }; then
    if mv "$primary_target" "$primary_backup"; then primary_backed_up=1; else failed=1; fi
  fi
  if [ "$failed" -eq 0 ] && { [ -e "$legacy_target" ] || [ -L "$legacy_target" ]; }; then
    if mv "$legacy_target" "$legacy_backup"; then legacy_backed_up=1; else failed=1; fi
  fi
  if [ "$failed" -eq 0 ]; then
    if mv "$staged_primary" "$primary_target"; then primary_committed=1; else failed=1; fi
  fi
  if [ "$failed" -eq 0 ]; then
    if mv "$staged_legacy" "$legacy_target"; then legacy_committed=1; else failed=1; fi
  fi
  if [ "$failed" -eq 0 ]; then
    assert_entrypoint_pair "$primary_target" "$legacy_target" || failed=1
  fi

  if [ "$failed" -ne 0 ]; then
    if [ "$legacy_committed" -eq 1 ]; then rm -f "$legacy_target"; fi
    if [ "$primary_committed" -eq 1 ]; then rm -f "$primary_target"; fi
    if [ "$legacy_backed_up" -eq 1 ]; then mv "$legacy_backup" "$legacy_target"; fi
    if [ "$primary_backed_up" -eq 1 ]; then mv "$primary_backup" "$primary_target"; fi
    rm -rf "$stage_dir"
    echo "Error: CLI installation failed; the previous entrypoint pair was restored." >&2
    return 1
  fi

  rm -rf "$stage_dir"
}

# A running daemon keeps executing the previous binary (old inode) until it is
# restarted; `systemctl enable --now` does NOT restart an already-active
# service either. Restart the installed auto-start service so upgrades take
# effect. Best-effort: skips cleanly when nothing is installed.
restart_daemon_for_upgrade() {
  local os unit_name agent_label config_home plist uid
  os="$(uname -s)"
  unit_name="bitfun-cli-daemon.service"
  agent_label="com.bitfun.cli.daemon"

  case "$os" in
    Linux)
      config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
      if [ -f "$config_home/systemd/user/$unit_name" ]; then
        if command -v systemctl >/dev/null 2>&1 && systemctl --user try-restart "$unit_name" 2>/dev/null; then
          echo "Restarted $unit_name to pick up the new binary."
        else
          echo "Note: daemon service installed but could not be restarted from this shell."
          echo "Run: systemctl --user restart $unit_name"
        fi
        return 0
      fi
      ;;
    Darwin)
      plist="$HOME/Library/LaunchAgents/${agent_label}.plist"
      if [ -f "$plist" ]; then
        uid="$(id -u)"
        if launchctl kickstart -k "gui/${uid}/${agent_label}" 2>/dev/null; then
          echo "Restarted LaunchAgent ${agent_label} to pick up the new binary."
        else
          echo "Note: daemon LaunchAgent installed but could not be restarted."
          echo "Run: launchctl kickstart -k gui/${uid}/${agent_label}"
        fi
        return 0
      fi
      ;;
  esac

  # No auto-start service installed — warn only when a manually supervised
  # daemon is still running the old binary.
  if "${BIN_DIR}/bitfun" daemon status 2>/dev/null | grep -q "daemon process: running"; then
    echo "Note: a running daemon was detected (not service-managed); restart it"
    echo "so it picks up the new binary."
  fi
}

usage() {
  cat <<'EOF'
BitFun CLI install script

Usage:
  bash install.sh [--help]

Builds the release CLI and installs the primary bitfun command plus the
deprecated bitfun-cli compatibility entrypoint.

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
echo "[1/4] Building bitfun and the deprecated bitfun-cli entrypoint (release)..."
cd "$REPO_ROOT"
# Build from workspace root so path deps resolve.
cargo build -p bitfun-cli --release

TARGET_DIR="${CARGO_TARGET_DIR:-${REPO_ROOT}/target}"
if [ -n "${CARGO_BUILD_TARGET:-}" ]; then
  RELEASE_DIR="${TARGET_DIR}/${CARGO_BUILD_TARGET}/release"
else
  RELEASE_DIR="${TARGET_DIR}/release"
fi
BUILT_BIN="${RELEASE_DIR}/bitfun"
BUILT_LEGACY_BIN="${RELEASE_DIR}/bitfun-cli"
for binary in "$BUILT_BIN" "$BUILT_LEGACY_BIN"; do
  if [ ! -x "$binary" ]; then
    echo "Error: built binary not found at $binary"
    exit 1
  fi
done

echo ""
echo "[2/4] Installing binaries..."
install_entrypoint_pair "$BUILT_BIN" "$BUILT_LEGACY_BIN" "$BIN_DIR"
echo "Installed: ${BIN_DIR}/bitfun"
echo "Installed deprecated compatibility entrypoint: ${BIN_DIR}/bitfun-cli"
assert_entrypoint_pair "${BIN_DIR}/bitfun" "${BIN_DIR}/bitfun-cli"

echo ""
echo "[3/4] Configuring shell PATH..."
if [ "${BITFUN_CLI_SKIP_SHELLRC:-0}" = "1" ]; then
  echo "Skipped shell rc edits (BITFUN_CLI_SKIP_SHELLRC=1)."
else
  upsert_shellrc_path "${HOME}/.bashrc" "$BIN_DIR"
  upsert_shellrc_path "${HOME}/.zshrc" "$BIN_DIR"
fi

echo ""
echo "[4/4] Restarting account daemon (if installed)..."
restart_daemon_for_upgrade

echo ""
echo "=== Install complete ==="
print_path_guidance "$BIN_DIR"
echo "Deprecated compatibility command: bitfun-cli"
echo "Login (Peer Host): open /login inside the TUI after start."
echo "Server use: after /login, run \`bitfun daemon install\` to keep this"
echo "device reachable by your account even after exit or reboot."

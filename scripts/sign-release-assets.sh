#!/usr/bin/env bash
#
# Sign release assets with the project's minisign key — the same key the Tauri
# updater already uses, so every artifact shares one trust root.
#
# Usage: sign-release-assets.sh <file> [<file>...]
#
# Environment:
#   BITFUN_SIGNING_KEY       minisign secret key, base64 (Tauri's wrapper format)
#   BITFUN_SIGNING_PASSWORD  password for that key
#   BITFUN_SIGNING_PUBKEY    minisign public key, base64; used to self-verify
#
# With no signing key configured this is a no-op, so forks keep building.
#
# Produces `<file>.sig` containing base64 of the whole minisign signature file,
# matching what the Tauri bundler emits for updater artifacts and what
# `minisign_verify` expects after one base64 decode.
#
# Files that already carry a `.sig` are left alone: the Tauri bundler signs the
# updater artifacts during `tauri build`, and re-signing them here would replace
# a signature the updater manifest already references.

set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "usage: sign-release-assets.sh <file> [<file>...]" >&2
  exit 2
fi

if [ -z "${BITFUN_SIGNING_KEY:-}" ]; then
  echo "[sign] No signing key configured; assets ship with checksums only."
  exit 0
fi

if ! command -v minisign >/dev/null 2>&1; then
  echo "[sign] Installing minisign..."
  # Ubuntu's package is not available in the default repositories on every
  # runner image (notably ubuntu-24.04-arm). Prefer it when present, then use
  # the upstream portable Linux distribution as a fallback.
  if ! sudo apt-get install -y --no-install-recommends minisign >/dev/null 2>&1; then
    if ! command -v curl >/dev/null 2>&1 || ! command -v sha256sum >/dev/null 2>&1 || ! command -v tar >/dev/null 2>&1; then
      echo "[sign] ERROR: minisign is unavailable and the Linux fallback prerequisites are missing." >&2
      exit 1
    fi
    MINISIGN_ROOT="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/bitfun-minisign"
    MINISIGN_VERSION="0.12"
    case "$(uname -m)" in
      x86_64) MINISIGN_ARCH="x86_64" ;;
      aarch64) MINISIGN_ARCH="aarch64" ;;
      *)
        echo "[sign] ERROR: unsupported Linux architecture for minisign fallback: $(uname -m)" >&2
        exit 1
        ;;
    esac
    MINISIGN_ARCHIVE="$MINISIGN_ROOT/minisign-linux.tar.gz"
    mkdir -p "$MINISIGN_ROOT/bin"
    curl -fsSL --retry 3 \
      "https://github.com/jedisct1/minisign/releases/download/${MINISIGN_VERSION}/minisign-${MINISIGN_VERSION}-linux.tar.gz" \
      -o "$MINISIGN_ARCHIVE"
    echo "9a599b48ba6eb7b1e80f12f36b94ceca7c00b7a5173c95c3efc88d9822957e73  $MINISIGN_ARCHIVE" | sha256sum -c -
    tar -xzf "$MINISIGN_ARCHIVE" -C "$MINISIGN_ROOT"
    install -m 0755 \
      "$MINISIGN_ROOT/minisign-linux/$MINISIGN_ARCH/minisign" \
      "$MINISIGN_ROOT/bin/minisign"
    export PATH="$MINISIGN_ROOT/bin:$PATH"
  fi
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
umask 077

# Tauri stores the minisign secret key base64-wrapped; unwrap it to the on-disk
# format minisign expects.
printf '%s' "$BITFUN_SIGNING_KEY" | base64 -d >"$WORK/release.key"
printf '%s' "${BITFUN_SIGNING_PUBKEY:-}" | base64 -d >"$WORK/release.pub" 2>/dev/null || true

signed=0
skipped=0
for target in "$@"; do
  if [ ! -f "$target" ]; then
    continue
  fi
  case "$target" in
    *.sig | *.minisig)
      continue
      ;;
  esac
  if [ -f "${target}.sig" ]; then
    echo "[sign] Already signed upstream, leaving alone: $(basename "$target")"
    skipped=$((skipped + 1))
    continue
  fi

  printf '%s\n' "${BITFUN_SIGNING_PASSWORD:-}" |
    minisign -S -s "$WORK/release.key" -m "$target" -x "${target}.minisig" >/dev/null

  # Verify before publishing. A signature nobody checked is worse than none,
  # because clients are about to treat it as proof.
  if [ -s "$WORK/release.pub" ]; then
    minisign -Vm "$target" -p "$WORK/release.pub" -x "${target}.minisig" >/dev/null
  else
    echo "[sign] ERROR: BITFUN_SIGNING_PUBKEY is required to self-verify signatures." >&2
    exit 1
  fi

  # `tr -d` rather than `base64 -w0`: the latter is GNU-only and this script is
  # also run by hand on macOS.
  base64 <"${target}.minisig" | tr -d '\n' >"${target}.sig"
  rm -f "${target}.minisig"
  echo "[sign] Signed and verified: $(basename "$target").sig"
  signed=$((signed + 1))
done

echo "[sign] ${signed} signed, ${skipped} already signed upstream."

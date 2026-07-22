#!/usr/bin/env bash
set -euo pipefail

# Inject extra TXT files into a Tauri-generated DMG.
# Usage: ./scripts/patch-dmg-extras.sh <path-to.dmg>
#
# The script converts the read-only DMG to read-write, mounts it,
# copies the helper TXT files, unmounts, and converts back to
# a compressed read-only DMG (overwriting the original).

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
EXTRAS_DIR="$ROOT_DIR/src/apps/desktop/dmg-extras"

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <path-to.dmg>"
  exit 1
fi

DMG_PATH="$1"

if [[ ! -f "$DMG_PATH" ]]; then
  echo "Error: DMG not found at $DMG_PATH"
  exit 1
fi

if [[ ! -d "$EXTRAS_DIR" ]]; then
  echo "Error: dmg-extras directory not found at $EXTRAS_DIR"
  exit 1
fi

echo "==> Patching DMG: $DMG_PATH"

WORK_DIR="$(mktemp -d)"
RW_DMG="$WORK_DIR/rw.dmg"
MOUNT_POINT="$WORK_DIR/mnt"

cleanup() {
  if mount | grep -q "$MOUNT_POINT"; then
    hdiutil detach "$MOUNT_POINT" -quiet -force 2>/dev/null || true
  fi
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

echo "    Converting to read-write..."
hdiutil convert "$DMG_PATH" -format UDRW -o "$RW_DMG" -quiet

echo "    Mounting read-write DMG..."
mkdir -p "$MOUNT_POINT"
hdiutil attach "$RW_DMG" -mountpoint "$MOUNT_POINT" -nobrowse -quiet

echo "    Copying extra files..."
for f in "$EXTRAS_DIR"/*.txt; do
  if [[ -f "$f" ]]; then
    cp "$f" "$MOUNT_POINT/"
    echo "      + $(basename "$f")"
  fi
done

echo "    Unmounting..."
hdiutil detach "$MOUNT_POINT" -quiet

echo "    Converting back to compressed read-only..."
rm -f "$DMG_PATH"
hdiutil convert "$RW_DMG" -format UDZO -o "$DMG_PATH" -quiet

echo "==> Done: $DMG_PATH"

#!/usr/bin/env bash
#
# record-demo.sh — Record the README demo GIF of OpenBitFun running a real task.
#
# The README leads with a static screenshot, which cannot show what an Agent
# actually does. A short loop of one real task running end to end is the single
# highest-conversion asset on the page. This script removes the fiddly parts:
# screen capture, palette-optimised GIF encoding, and staying under GitHub's
# rendering limits.
#
# Usage:
#   scripts/record-demo.sh [seconds] [output]
#
#   scripts/record-demo.sh                 # 30s -> png/demo.gif
#   scripts/record-demo.sh 20              # 20s -> png/demo.gif
#   scripts/record-demo.sh 25 png/x.gif    # 25s -> png/x.gif
#
# Requires ffmpeg (brew install ffmpeg) and Screen Recording permission for
# your terminal in System Settings > Privacy & Security > Screen Recording.
#
# What to record (in order of how much it matters):
#   1. Open OpenBitFun on a real repository, type a real task, hit enter.
#   2. Let the Agent plan, edit files, and run something that visibly succeeds.
#   3. End on the result — a passing test, a diff, a finished document.
# Keep it under 30s. No title cards, no cursor hunting, no dead air.

set -euo pipefail

DURATION="${1:-30}"
OUTPUT="${2:-png/demo.gif}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "error: ffmpeg not found. Install it with: brew install ffmpeg" >&2
  exit 1
fi

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT
RAW="$WORKDIR/raw.mov"
PALETTE="$WORKDIR/palette.png"

# Width of the final GIF. 1200 is wide enough to read UI text on GitHub
# without pushing the file past a few megabytes.
WIDTH="${DEMO_WIDTH:-1200}"
FPS="${DEMO_FPS:-12}"

echo "Available capture devices:"
ffmpeg -f avfoundation -list_devices true -i "" 2>&1 | sed -n '/AVFoundation video devices/,/AVFoundation audio devices/p' || true
echo

SCREEN_INDEX="${DEMO_SCREEN_INDEX:-1}"
echo "Recording screen index $SCREEN_INDEX for ${DURATION}s (override with DEMO_SCREEN_INDEX=n)."
echo "Starting in 3 seconds — switch to OpenBitFun now."
sleep 3

ffmpeg -hide_banner -loglevel error \
  -f avfoundation -capture_cursor 1 -framerate 30 -i "$SCREEN_INDEX:none" \
  -t "$DURATION" -c:v libx264 -pix_fmt yuv420p "$RAW"

echo "Encoding GIF (two-pass palette)…"
FILTERS="fps=$FPS,scale=$WIDTH:-1:flags=lanczos"

ffmpeg -hide_banner -loglevel error -i "$RAW" \
  -vf "$FILTERS,palettegen=stats_mode=diff" -y "$PALETTE"

ffmpeg -hide_banner -loglevel error -i "$RAW" -i "$PALETTE" \
  -lavfi "$FILTERS[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" \
  -y "$OUTPUT"

SIZE_BYTES="$(wc -c < "$OUTPUT" | tr -d ' ')"
SIZE_MB="$(echo "$SIZE_BYTES" | awk '{printf "%.1f", $1/1048576}')"
echo "Wrote $OUTPUT (${SIZE_MB} MB)"

if [ "$SIZE_BYTES" -gt 10485760 ]; then
  echo
  echo "warning: over GitHub's 10 MB image limit — it will not render." >&2
  echo "  Re-run shorter, or with DEMO_FPS=10 DEMO_WIDTH=1000." >&2
  exit 1
fi

echo
echo "Next: swap the screenshot at the top of README.md and README.zh-CN.md for:"
echo "  ![OpenBitFun in action](./${OUTPUT})"

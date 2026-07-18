#!/usr/bin/env bash
#
# sync-release.sh — Mirror BitFun release assets from GitHub to openbitfun.com.
#
# Flow:
#   1. Fetch latest.json from GitHub (follows /releases/latest/download/ redirect)
#   2. Download every platform installer package into release/{version}/
#   3. Rewrite download URLs in latest.json to point at openbitfun.com
#   4. Publish release/{version}/latest.json and release/latest.json
#   5. Remove old version dirs, keeping only the two most recent
#
# The published release/latest.json is the Tauri updater fallback endpoint.
# When GitHub is unreachable, the desktop client automatically falls through
# to https://openbitfun.com/release/latest.json and downloads from this mirror.
#
# Cron (every 12 hours):
#   0 */12 * * * /root/lwb/repo/BitFun-AutoUpdate/sync-release.sh \
#       >> /root/lwb/repo/BitFun-AutoUpdate/sync.log 2>&1
#
set -euo pipefail

# ── Configuration ──────────────────────────────────────────────
GITHUB_LATEST_JSON_URL="https://github.com/GCWing/BitFun/releases/latest/download/latest.json"
OPENBITFUN_BASE_URL="https://openbitfun.com/release"
WEBSITE_RELEASE_DIR="/root/lwb/repo/BitFun-Website/dist/release"
KEEP_VERSIONS=2
CONNECT_TIMEOUT=30
MAX_TIME=1800          # per-request ceiling (30 min; installer packages can be large)
MAX_RETRIES=3
RETRY_DELAY=5
PYTHON="${PYTHON:-python3}"

# ── Helpers ────────────────────────────────────────────────────
log() { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*"; }

# ── Main ───────────────────────────────────────────────────────
main() {
  log "=== BitFun release sync started ==="

  mkdir -p "$WEBSITE_RELEASE_DIR"

  # 1. Fetch latest.json from GitHub
  log "Fetching latest.json from GitHub..."
  LATEST_JSON=$(curl -fsSL \
    --connect-timeout "$CONNECT_TIMEOUT" \
    --max-time "$MAX_TIME" \
    "$GITHUB_LATEST_JSON_URL") || {
    log "ERROR: Failed to fetch latest.json from GitHub"
    exit 1
  }

  # 2. Extract version
  VERSION=$(printf '%s' "$LATEST_JSON" | "$PYTHON" -c \
    "import sys,json;print(json.load(sys.stdin)['version'])") || {
    log "ERROR: Failed to parse version from latest.json"
    exit 1
  }
  log "Latest version: $VERSION"

  # 3. Create version directory
  VERSION_DIR="${WEBSITE_RELEASE_DIR}/${VERSION}"
  mkdir -p "$VERSION_DIR"

  # 4. Download all platform installer packages
  #    Extract "<url>\t<filename>" pairs, then curl each one.
  ASSET_LIST=$(printf '%s' "$LATEST_JSON" | "$PYTHON" -c "
import sys, json
data = json.load(sys.stdin)
for p, info in data.get('platforms', {}).items():
    url = info['url']
    fname = url.split('/')[-1]
    print(f'{url}\t{fname}')
") || {
    log "ERROR: Failed to extract asset list from latest.json"
    exit 1
  }

  while IFS=$'\t' read -r url filename; do
    [ -z "$url" ] && continue
    dest="${VERSION_DIR}/${filename}"

    if [ -f "$dest" ]; then
      log "  Already exists: $filename"
      continue
    fi

    log "  Downloading: $filename"
    ok=0
    for attempt in $(seq 1 "$MAX_RETRIES"); do
      if curl -fsSL \
          --connect-timeout "$CONNECT_TIMEOUT" \
          --max-time "$MAX_TIME" \
          -o "$dest" "$url"; then
        ok=1
        break
      fi
      log "  Retry $attempt/$MAX_RETRIES for $filename"
      sleep "$RETRY_DELAY"
    done

    if [ "$ok" -ne 1 ]; then
      log "ERROR: Failed to download $filename after $MAX_RETRIES attempts"
      rm -f "$dest"
      exit 1
    fi
  done <<< "$ASSET_LIST"

  # 5. Rewrite URLs in latest.json to point at openbitfun.com
  printf '%s' "$LATEST_JSON" | "$PYTHON" -c "
import sys, json
data = json.load(sys.stdin)
version = data['version']
base = '${OPENBITFUN_BASE_URL}/' + version
for p, info in data.get('platforms', {}).items():
    fname = info['url'].split('/')[-1]
    info['url'] = base + '/' + fname
print(json.dumps(data, indent=2))
" > "${VERSION_DIR}/latest.json"
  log "Saved ${VERSION_DIR}/latest.json"

  # 6. Publish root latest.json (Tauri fallback endpoint)
  cp "${VERSION_DIR}/latest.json" "${WEBSITE_RELEASE_DIR}/latest.json"
  log "Updated ${WEBSITE_RELEASE_DIR}/latest.json"

  # 7. Clean up old versions — keep only the latest KEEP_VERSIONS dirs
  ALL_DIRS=()
  while IFS= read -r d; do
    ALL_DIRS+=("$d")
  done < <(find "$WEBSITE_RELEASE_DIR" -mindepth 1 -maxdepth 1 -type d | sort -V)
  TOTAL=${#ALL_DIRS[@]}
  if [ "$TOTAL" -gt "$KEEP_VERSIONS" ]; then
    REMOVE_COUNT=$((TOTAL - KEEP_VERSIONS))
    for ((i = 0; i < REMOVE_COUNT; i++)); do
      log "Removing old version: $(basename "${ALL_DIRS[$i]}")"
      rm -rf "${ALL_DIRS[$i]}"
    done
  fi

  log "=== Sync complete: version $VERSION ==="
}

main "$@"

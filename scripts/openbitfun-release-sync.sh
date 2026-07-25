#!/usr/bin/env bash
#
# sync-release.sh — Mirror BitFun release assets from GitHub to openbitfun.com.
#
# Flow:
#   1. Fetch latest.json from GitHub (follows /releases/latest/download/ redirect)
#   2. Download every Desktop updater package into release/{version}/
#   3. If present, download linux-binaries.json plus its CLI/Relay assets
#   4. Rewrite all mirrored URLs to point at openbitfun.com
#   5. Publish versioned and root manifests
#   6. Remove old version dirs, keeping only the two most recent
#
# The published release/latest.json is the Tauri updater fallback endpoint.
# When GitHub is unreachable, the desktop client automatically falls through
# to https://openbitfun.com/release/latest.json and downloads from this mirror.
#
# Cron (every 10 minutes):
#   */10 * * * * /root/repos/BitFun-AutoUpdate/openbitfun-release-sync.sh \
#       >> /root/repos/BitFun-AutoUpdate/sync.log 2>&1
#
set -euo pipefail

# ── Configuration ──────────────────────────────────────────────
GITHUB_LATEST_JSON_URL="https://github.com/GCWing/BitFun/releases/latest/download/latest.json"
GITHUB_LINUX_BINARIES_URL="https://github.com/GCWing/BitFun/releases/latest/download/linux-binaries.json"
OPENBITFUN_BASE_URL="https://openbitfun.com/release"
WEBSITE_RELEASE_DIR="/root/repos/BitFun-Website/dist/release"
LOCK_FILE="/root/repos/BitFun-AutoUpdate/sync.lock"
KEEP_VERSIONS=2
CONNECT_TIMEOUT=30
MAX_TIME=1800          # per-request ceiling (30 min; installer packages can be large)
MAX_RETRIES=3
RETRY_DELAY=5
PYTHON="${PYTHON:-python3}"

# ── Helpers ────────────────────────────────────────────────────
log() { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*"; }

download_asset() {
  local url="$1"
  local dest="$2"
  local filename tmp ok attempt
  filename="$(basename "$dest")"
  if [ -f "$dest" ]; then
    log "  Already exists: $filename"
    return 0
  fi

  tmp="${dest}.part"
  ok=0
  for attempt in $(seq 1 "$MAX_RETRIES"); do
    if curl -fsSL \
        --connect-timeout "$CONNECT_TIMEOUT" \
        --max-time "$MAX_TIME" \
        -o "$tmp" "$url"; then
      mv "$tmp" "$dest"
      ok=1
      break
    fi
    log "  Retry $attempt/$MAX_RETRIES for $filename"
    sleep "$RETRY_DELAY"
  done
  if [ "$ok" -ne 1 ]; then
    rm -f "$tmp"
    log "ERROR: Failed to download $filename after $MAX_RETRIES attempts"
    return 1
  fi
}

# Fetch linux-binaries.json into $LINUX_MANIFEST_TMP, retrying transient
# failures. Sets LINUX_MANIFEST_STATE to one of:
#   ok        — downloaded
#   missing   — GitHub answered 404: the release genuinely has no manifest
#   unhealthy — network/5xx: unknown, so callers must keep the published mirror
fetch_linux_manifest() {
  local attempt status
  LINUX_MANIFEST_STATE="unhealthy"
  for attempt in $(seq 1 "$MAX_RETRIES"); do
    status="$(curl -sSL \
      --connect-timeout "$CONNECT_TIMEOUT" \
      --max-time "$MAX_TIME" \
      -o "$LINUX_MANIFEST_TMP" \
      -w '%{http_code}' \
      "$GITHUB_LINUX_BINARIES_URL" || echo "000")"
    if [ "$status" = "200" ]; then
      LINUX_MANIFEST_STATE="ok"
      return 0
    fi
    rm -f "$LINUX_MANIFEST_TMP"
    if [ "$status" = "404" ]; then
      LINUX_MANIFEST_STATE="missing"
      return 0
    fi
    log "  Retry $attempt/$MAX_RETRIES for linux-binaries.json (HTTP $status)"
    sleep "$RETRY_DELAY"
  done
  return 0
}

# ── Main ───────────────────────────────────────────────────────
main() {
  mkdir -p "$(dirname "$LOCK_FILE")"
  exec 9>"$LOCK_FILE"
  if command -v flock >/dev/null 2>&1 && ! flock -n 9; then
    log "Another release sync is still running; skipping this interval."
    exit 0
  fi

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
    log "  Mirroring Desktop asset: $filename"
    download_asset "$url" "${VERSION_DIR}/${filename}" || exit 1
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

  # 7. Mirror the CLI + Relay release manifest and every asset it references.
  LINUX_MANIFEST_TMP="${VERSION_DIR}/linux-binaries.github.json.part"
  LINUX_MANIFEST_STATE="missing"
  fetch_linux_manifest
  if [ "$LINUX_MANIFEST_STATE" = "ok" ]; then
    LINUX_VERSION=$("$PYTHON" -c \
      "import json,sys;print(json.load(open(sys.argv[1], encoding='utf-8'))['version'])" \
      "$LINUX_MANIFEST_TMP")
    if [ "$LINUX_VERSION" != "$VERSION" ]; then
      log "ERROR: Linux manifest version $LINUX_VERSION does not match Desktop version $VERSION"
      rm -f "$LINUX_MANIFEST_TMP"
      exit 1
    fi

    LINUX_ASSET_LIST=$("$PYTHON" - "$LINUX_MANIFEST_TMP" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    data = json.load(f)
seen = set()
for platform in data.get("platforms", {}).values():
    for product in ("cli", "relay"):
        entry = platform.get(product, {})
        for key in ("url", "sha256Url"):
            url = entry.get(key)
            if not url:
                continue
            filename = url.rsplit("/", 1)[-1]
            if filename not in seen:
                seen.add(filename)
                print(f"{url}\t{filename}")
PY
)
    while IFS=$'\t' read -r url filename; do
      [ -z "$url" ] && continue
      log "  Mirroring Linux binary asset: $filename"
      download_asset "$url" "${VERSION_DIR}/${filename}" || exit 1
    done <<< "$LINUX_ASSET_LIST"

    "$PYTHON" - "$LINUX_MANIFEST_TMP" "${VERSION_DIR}/linux-binaries.json" \
      "$OPENBITFUN_BASE_URL" <<'PY'
import json, sys
source, dest, base = sys.argv[1:]
with open(source, encoding="utf-8") as f:
    data = json.load(f)
version_base = f"{base}/{data['version']}"
for platform in data.get("platforms", {}).values():
    for product in ("cli", "relay"):
        entry = platform.get(product, {})
        for key in ("url", "sha256Url"):
            if entry.get(key):
                entry[key] = f"{version_base}/{entry[key].rsplit('/', 1)[-1]}"
with open(dest, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY
    rm -f "$LINUX_MANIFEST_TMP"
    cp "${VERSION_DIR}/linux-binaries.json" "${WEBSITE_RELEASE_DIR}/linux-binaries.json"
    log "Updated ${WEBSITE_RELEASE_DIR}/linux-binaries.json"
  elif [ "$LINUX_MANIFEST_STATE" = "missing" ]; then
    rm -f "${WEBSITE_RELEASE_DIR}/linux-binaries.json"
    log "Linux binaries manifest is not present in the latest release yet; Desktop mirror only."
  else
    # Transient failure. Keep whatever is already published: CLI self-update and
    # one-click Relay deploy both fall back to this file, so a single flaky run
    # must not take it offline for the next 10 minutes.
    log "WARN: Linux binaries manifest unreachable this run; keeping the published mirror."
  fi

  # 8. Clean up old versions — keep only the latest KEEP_VERSIONS dirs
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

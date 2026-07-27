#!/usr/bin/env bash
#
# Executes the source-ranking and download loop that
# `relay_deploy.rs::release_binary_deploy_bash` generates, against a stubbed
# curl. `bash -n` only proves the script parses; these scenarios prove it picks
# the fast source, survives a link that is slow rather than broken, and still
# reaches the source-build fallback when every source is dead.
#
# Usage: release-download-harness.sh <generated-script>
#
# Scenario plan lines are `url-substring:mode:speed-bytes-per-sec`, where mode is
# one of ok | stall | corrupt | dead.

set -uo pipefail

SCRIPT_UNDER_TEST="${1:?usage: release-download-harness.sh <generated-script>}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
STUB="$WORK/bin"
mkdir -p "$STUB" "$WORK/home"

cat >"$STUB/curl" <<'CURL'
#!/usr/bin/env bash
url=""; out=""; writeout=""; range=""
args=("$@")
for ((i=0;i<${#args[@]};i++)); do
  case "${args[$i]}" in
    -o) out="${args[$((i+1))]}" ;;
    -w) writeout="${args[$((i+1))]}" ;;
    -r) range="${args[$((i+1))]}" ;;
    http*) url="${args[$i]}" ;;
  esac
done
mode="dead"; speed="0"
while IFS=: read -r pat m s; do
  [ -n "$pat" ] || continue
  case "$url" in *"$pat"*) mode="$m"; speed="$s" ;; esac
done < "$PLAN"
echo "${mode}  ${url}" >>"$TRACE"
# Record only the resumable archive transfer (-C), not the ranged probe (-r):
# the probe is deliberately time-boxed, the transfer must not be.
case " $* " in
  *" -C "*) printf '%s\n' "$*" >>"$WORKDIR/archive-flags" ;;
esac
[ "$mode" = dead ] && exit 7

case "$url" in
  *linux-binaries.json) cat "$WORKDIR/manifest.json"; exit 0 ;;
esac

if [ -n "$range" ]; then                       # throughput probe
  [ -n "$writeout" ] && printf '%s' "$speed"
  exit 0
fi

case "$url" in
  *.sha256)
    # Only canonical github.com serves the true checksum. Every other origin
    # serves a wrong one, so a successful verify proves the canonical URL was
    # used rather than the download origin's own sidecar.
    case "$url" in
      https://github.com/*) printf '%s  %s\n' "$(cat "$WORKDIR/expected_sha")" "$(basename "${url%.sha256}")" >"$out" ;;
      *)                    printf '%s  %s\n' "$(printf 'f%.0s' $(seq 64))" "$(basename "${url%.sha256}")" >"$out" ;;
    esac
    exit 0 ;;
esac

case "$mode" in
  stall)   exit 28 ;;                          # curl's speed-limit abort
  corrupt) printf 'CORRUPT' >"$out"; exit 0 ;;
  ok)      cat "$WORKDIR/payload" >"$out"; exit 0 ;;
esac
exit 7
CURL

cat >"$STUB/tar" <<'TAR'
#!/usr/bin/env bash
echo "DOWNLOAD-OK-reached-tar" >>"$TRACE"
exit 1
TAR

cat >"$STUB/uname" <<'U'
#!/usr/bin/env bash
if [ "${1:-}" = "-m" ]; then echo x86_64; else echo Linux; fi
U

chmod +x "$STUB/curl" "$STUB/tar" "$STUB/uname"

RELAY_ASSET="bitfun-relay-server-x86_64-unknown-linux-gnu.tar.gz"
MIRROR_ASSET_URL="https://openbitfun.com/release/0.2.13/${RELAY_ASSET}"
cat >"$WORK/manifest.json" <<JSON
{"schemaVersion":1,"version":"0.2.13","tag":"v0.2.13","platforms":{
"linux-x86_64":{"target":"x86_64-unknown-linux-gnu",
"cli":{"filename":"bitfun-cli-0.2.13-x86_64-unknown-linux-gnu.tar.gz","url":"https://openbitfun.com/release/0.2.13/bitfun-cli-0.2.13-x86_64-unknown-linux-gnu.tar.gz","sha256Url":"https://openbitfun.com/release/0.2.13/bitfun-cli-0.2.13-x86_64-unknown-linux-gnu.tar.gz.sha256"},
"relay":{"filename":"${RELAY_ASSET}","url":"${MIRROR_ASSET_URL}","sha256Url":"${MIRROR_ASSET_URL}.sha256"}}}}
JSON

printf 'RELAY-ARCHIVE-CONTENT' >"$WORK/payload"
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$WORK/payload" | awk '{print $1}' >"$WORK/expected_sha"
else
  shasum -a 256 "$WORK/payload" | awk '{print $1}' >"$WORK/expected_sha"
fi

pass=0
fail=0

run_case() {
  local name="$1" expect="$2"
  shift 2
  printf '%s\n' "$@" >"$WORK/plan"
  : >"$WORK/trace"
  : >"$WORK/archive-flags"
  rm -rf "$WORK/home/.bitfun"

  local output got
  output="$(
    export PATH="$STUB:$PATH" HOME="$WORK/home" WORKDIR="$WORK" \
      PLAN="$WORK/plan" TRACE="$WORK/trace" RELAY_PORT=9700 \
      BITFUN_MIRROR_MODE=cn BITFUN_GITHUB_PROXY=https://ghfast.top/
    # Production calls this as an if-condition under `set -euo pipefail`.
    bash -c '
      set -euo pipefail
      source "$1"
      if bitfun_try_release_deploy; then echo RESULT=deployed; else echo RESULT=fallback; fi
    ' _ "$SCRIPT_UNDER_TEST" 2>&1
  )"

  if grep -q "DOWNLOAD-OK-reached-tar" "$WORK/trace"; then got=download-ok; else got=no-download; fi

  local problem=""
  if [ "$got" != "$expect" ]; then
    problem="expected $expect, got $got"
  elif [ -n "${EXPECT_SOURCE:-}" ] &&
    ! printf '%s\n' "$output" | grep -qF "Downloading published Relay binary: ${EXPECT_SOURCE}"; then
    # The chosen source must be the fastest one that actually works.
    problem="expected the download to come from ${EXPECT_SOURCE}"
  fi

  if [ -z "$problem" ]; then
    echo "PASS  $name"
    pass=$((pass + 1))
  else
    echo "FAIL  $name ($problem)"
    printf '%s\n' "$output" | sed 's/^/        /'
    echo "      curl trace:"
    sed 's/^/        /' "$WORK/trace"
    fail=$((fail + 1))
  fi
}

# Read the tag out of the script under test rather than hardcoding one: Desktop
# pins BITFUN_RELEASE_TAG to its own crate version, so a literal here silently
# rots at every release bump and every `EXPECT_SOURCE="$GITHUB_URL"` case fails.
RELEASE_TAG="$(sed -n 's/^export BITFUN_RELEASE_TAG="\(.*\)"$/\1/p' "$SCRIPT_UNDER_TEST" | head -n1)"
RELEASE_TAG="${RELEASE_TAG:-latest}"
if [ "$RELEASE_TAG" = "latest" ]; then
  GITHUB_URL="https://github.com/GCWing/BitFun/releases/latest/download/${RELAY_ASSET}"
else
  GITHUB_URL="https://github.com/GCWing/BitFun/releases/download/${RELEASE_TAG}/${RELAY_ASSET}"
fi

# The reported case: GitHub is reachable but crawling, the mirror is fast.
# Ranking must send the download to the mirror instead of crawling for an hour.
EXPECT_SOURCE="$MIRROR_ASSET_URL" \
  run_case "slow GitHub loses to fast mirror" download-ok \
  "ghfast.top:ok:20480" "//github.com:ok:20480" \
  "linux-binaries.json:ok:999999" "release/0.2.13:ok:2097152"

# Nothing clears the healthy bar: still download, because the alternative is a
# 20-minute source rebuild.
EXPECT_SOURCE="$MIRROR_ASSET_URL" \
  run_case "all sources under the healthy bar still download" download-ok \
  "ghfast.top:ok:20480" "//github.com:ok:30720" \
  "linux-binaries.json:ok:999999" "release/0.2.13:ok:40960"

# A source that dies mid-transfer must hand off rather than retry forever.
EXPECT_SOURCE="$GITHUB_URL" \
  run_case "stalled fastest source fails over" download-ok \
  "ghfast.top:stall:2097152" "//github.com:ok:100000" \
  "linux-binaries.json:ok:999999" "release/0.2.13:ok:50000"

# Bad bytes must not be resumed on top of from the next source.
EXPECT_SOURCE="$GITHUB_URL" \
  run_case "checksum mismatch discards the partial file" download-ok \
  "ghfast.top:corrupt:2097152" "//github.com:ok:100000" \
  "linux-binaries.json:ok:999999" "release/0.2.13:ok:50000"

# An unreachable mirror manifest must not abort the caller under `set -e`.
EXPECT_SOURCE="$GITHUB_URL" \
  run_case "unreachable mirror manifest leaves GitHub usable" download-ok \
  "ghfast.top:dead:0" "//github.com:ok:150000" "linux-binaries.json:dead:0"

EXPECT_SOURCE="" \
  run_case "every source dead reaches the source-build fallback" no-download \
  "ghfast.top:dead:0" "//github.com:dead:0" "linux-binaries.json:dead:0"

# Security property: bytes from a mirror must be checked against the checksum
# GitHub serves, not the one the mirror serves. The stub gives every non-GitHub
# origin a wrong checksum, so downloading from the mirror can only succeed if
# the canonical URL was used.
EXPECT_SOURCE="$MIRROR_ASSET_URL" \
  run_case "mirror download verifies against the canonical GitHub checksum" download-ok \
  "ghfast.top:ok:1024" "//github.com:ok:1024" \
  "linux-binaries.json:ok:999999" "release/0.2.13:ok:2097152"

# A wall-clock ceiling on the archive transfer is the original bug in disguise:
# it kills a link that is slow but progressing. The throughput floor must be the
# only give-up condition.
if grep -q -- '--max-time' "$WORK/archive-flags"; then
  echo "FAIL  archive download must not carry a wall-clock ceiling"
  grep -- '--max-time' "$WORK/archive-flags" | sed 's/^/        /'
  fail=$((fail + 1))
else
  echo "PASS  archive download has no wall-clock ceiling"
  pass=$((pass + 1))
fi
if grep -q -- '--speed-limit' "$WORK/archive-flags"; then
  echo "PASS  archive download gives up on a throughput floor"
  pass=$((pass + 1))
else
  echo "FAIL  archive download has no throughput floor"
  fail=$((fail + 1))
fi

echo "----"
echo "pass=$pass fail=$fail"
[ "$fail" -eq 0 ]

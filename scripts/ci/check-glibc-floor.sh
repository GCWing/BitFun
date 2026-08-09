#!/usr/bin/env bash
#
# Assert that released Linux binaries do not require a newer glibc than we
# promise to support.
#
# Usage: check-glibc-floor.sh <max-glibc> <binary> [binary...]
#
# Why this exists: the release matrix used to build x86_64 on ubuntu-22.04
# (glibc 2.35) but arm64 on ubuntu-24.04-arm (2.39). The arm64 relay therefore
# required GLIBC_2.38 and could not start at all in the deploy runtime image
# (debian:bookworm-slim, 2.36) — nor on a Debian 12 / Ubuntu 22.04 arm64 host
# for anyone running the tarball directly. Nothing anywhere declared the
# supported floor, so the mismatch was invisible until a container exited with a
# loader error on stderr that the deploy script discarded.
#
# The floor is a promise to users. Make it explicit and machine-checked, so
# raising it becomes a deliberate decision rather than a side effect of a runner
# image bump.
#
# `.gnu.version_r` is the authoritative list of versioned symbols a binary needs
# from its shared libraries; `readelf -V` prints it.

set -euo pipefail

MAX="${1:?usage: check-glibc-floor.sh <max-glibc> <binary> [binary...]}"
shift
if [ "$#" -eq 0 ]; then
  echo "ERROR: no binaries given" >&2
  exit 2
fi

# Without readelf every binary would look requirement-free and the check would
# pass everything — a silent false negative is worse than no check at all.
if ! command -v readelf >/dev/null 2>&1; then
  echo "ERROR: readelf not found (install binutils); refusing to skip the check" >&2
  exit 2
fi

# Highest GLIBC_x.y this binary asks for; empty when it needs none.
# The trailing `|| true` matters: `grep` exits non-zero when a binary has no
# versioned glibc symbols at all (a static build), and under `set -e` that
# would abort the whole run at the command substitution below.
max_required_glibc() {
  readelf -V "$1" 2>/dev/null |
    grep -oE 'GLIBC_[0-9]+\.[0-9]+' |
    sort -u -V |
    tail -n 1 || true
}

status=0
for bin in "$@"; do
  if [ ! -f "$bin" ]; then
    echo "ERROR: $bin does not exist" >&2
    status=1
    continue
  fi
  required="$(max_required_glibc "$bin")"
  if [ -z "$required" ]; then
    echo "ok    $(basename "$bin"): no versioned glibc requirement"
    continue
  fi
  required="${required#GLIBC_}"
  # sort -V puts the larger version last: if that is not MAX, MAX was exceeded.
  highest="$(printf '%s\n%s\n' "$MAX" "$required" | sort -V | tail -n 1)"
  if [ "$highest" != "$MAX" ]; then
    echo "FAIL  $(basename "$bin"): requires glibc $required, above the $MAX floor" >&2
    status=1
  else
    echo "ok    $(basename "$bin"): requires glibc $required (floor $MAX)"
  fi
done

if [ "$status" -ne 0 ]; then
  cat >&2 <<EOF

The binary needs a newer glibc than this release promises, so it cannot run on
older distributions or in the relay deploy runtime image.

Fix one of these, deliberately:
  - Build on an older runner image (this is what keeps the floor low), or
  - raise the declared floor here AND in the relay runtime base image
    (src/apps/relay-server/release-download.sh), remembering that already
    published archives keep whatever floor they were built with.
EOF
fi
exit "$status"

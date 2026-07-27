#!/usr/bin/env bash
# BitFun Relay Server — published-binary download and runtime deploy.
#
# Single implementation shared by both deployment paths, the same way mirror.sh
# is shared:
#   - src/apps/relay-server/deploy.sh          sources this file
#   - remote_ssh/relay_deploy.rs               embeds it with include_str!
#
# Defines:
#   bitfun_try_release_deploy   Download the published archive for this host's
#                               architecture, verify it, build a small runtime
#                               image around it and start the relay. Returns 1
#                               (without disturbing a running relay) whenever the
#                               caller should fall back to a source build.
#
# Configuration — all optional, defaults target the official release:
#   BITFUN_RELEASE_TAG              v0.2.13 | nightly | latest   (default latest)
#   BITFUN_GITHUB_RELEASE_BASE      https://github.com/GCWing/BitFun/releases
#   BITFUN_OPENBITFUN_RELEASE_BASE  https://openbitfun.com/release
#   BITFUN_GITHUB_PROXY             prefix-style proxy, set by mirror.sh in CN
#   BITFUN_MIRROR_MODE              cn | global, set by mirror.sh
#   RELAY_PORT                      published port (default 9700)
#   RELAY_HOST_BIND_IP              bind address (default 0.0.0.0, as compose)
#
# Throughput tuning. A wall-clock ceiling alone is the wrong give-up condition:
# it makes success depend on archive size over link speed, so a link that is
# merely slow can never finish and retries from zero forever. Rank sources by
# measured throughput instead, and treat only a sustained floor breach as death.
#   BITFUN_PROBE_SECONDS   probe window per candidate            (default 10)
#   BITFUN_PROBE_BYTES     ranged probe size                     (default 4MiB)
#   BITFUN_HEALTHY_BPS     a source at/above this is used freely (default 128KiB/s)
#   BITFUN_STALL_BPS       sustained below this counts as dead   (default 8KiB/s)
#   BITFUN_STALL_SECONDS   window for the floor above            (default 30)

BITFUN_RELEASE_TAG="${BITFUN_RELEASE_TAG:-latest}"
BITFUN_GITHUB_RELEASE_BASE="${BITFUN_GITHUB_RELEASE_BASE:-https://github.com/GCWing/BitFun/releases}"
BITFUN_OPENBITFUN_RELEASE_BASE="${BITFUN_OPENBITFUN_RELEASE_BASE:-https://openbitfun.com/release}"
BITFUN_PROBE_SECONDS="${BITFUN_PROBE_SECONDS:-10}"
BITFUN_PROBE_BYTES="${BITFUN_PROBE_BYTES:-4194304}"
BITFUN_HEALTHY_BPS="${BITFUN_HEALTHY_BPS:-131072}"
BITFUN_STALL_BPS="${BITFUN_STALL_BPS:-8192}"
BITFUN_STALL_SECONDS="${BITFUN_STALL_SECONDS:-30}"

# Docker invocation. relay_deploy.rs and common.sh each define their own
# privilege-aware wrapper before sourcing this file; fall back to a compatible
# one so the file also works standalone.
if ! declare -F bitfun_shell_join >/dev/null 2>&1; then
  # `sg -c` re-parses a single string, so an unquoted "$*" loses argument
  # boundaries. Single-quote each argument (POSIX-safe for any /bin/sh).
  bitfun_shell_join() {
    local out="" arg
    for arg in "$@"; do
      out="$out'$(printf '%s' "$arg" | sed "s/'/'\\\\''/g")' "
    done
    printf '%s' "$out"
  }
fi

if ! declare -F bitfun_docker >/dev/null 2>&1; then
  bitfun_docker() {
    case "${BITFUN_DOCKER_MODE:-direct}" in
      sg) sg docker -c "$(bitfun_shell_join docker "$@")" ;;
      sudo)
        if sudo -n true >/dev/null 2>&1; then sudo -n docker "$@"; else sudo docker "$@"; fi
        ;;
      *) docker "$@" ;;
    esac
  }
fi

# Build the release asset URL for a tag. `latest` uses GitHub's redirecting
# /releases/latest/download/ path, which is what the manual deploy wants; a
# pinned tag is what Desktop wants so the relay matches the app it ships with.
bitfun_release_asset_url() {
  local tag="$1" asset="$2"
  if [ "$tag" = "latest" ]; then
    printf '%s/latest/download/%s\n' "$BITFUN_GITHUB_RELEASE_BASE" "$asset"
  else
    printf '%s/download/%s/%s\n' "$BITFUN_GITHUB_RELEASE_BASE" "$tag" "$asset"
  fi
}

# Map any candidate download URL back to the checksum GitHub itself serves for
# those exact bytes.
#
# Verifying an archive against a `.sha256` fetched from the same host proves
# only that the transfer was not corrupted; a hostile or compromised mirror
# serves both and passes. Binding to a checksum from a different origin means
# one compromised mirror is not enough. This matters because the CN path
# deliberately prefers a third-party GitHub proxy.
#
# The mirror encodes its version in the path (release/<version>/<asset>), so the
# matching canonical tag is recoverable even when the mirror lags behind latest.
bitfun_canonical_checksum_url() {
  local url="$1" asset version
  asset="${url##*/}"
  case "$url" in
    "$BITFUN_OPENBITFUN_RELEASE_BASE"/*)
      version="${url#"$BITFUN_OPENBITFUN_RELEASE_BASE"/}"
      version="${version%%/*}"
      if [ -n "$version" ] && [ "$version" != "$asset" ]; then
        printf '%s/download/v%s/%s.sha256\n' "$BITFUN_GITHUB_RELEASE_BASE" "$version" "$asset"
        return 0
      fi
      ;;
    *"$BITFUN_GITHUB_RELEASE_BASE"/*)
      # Plain GitHub, or a prefix-style proxy in front of it. Strip the prefix.
      printf '%s.sha256\n' "${BITFUN_GITHUB_RELEASE_BASE}${url#*"$BITFUN_GITHUB_RELEASE_BASE"}"
      return 0
      ;;
  esac
  printf '%s.sha256\n' "$url"
}

# Build the runtime image around the published binary.
#
# Losing this build costs ~20 minutes: the caller falls back to compiling the
# relay from source. Two failure modes are recoverable and worth retrying rather
# than surrendering to that, both observed on real hosts:
#
#   - DOCKER_CONFIG holds a root-owned config.json from an earlier elevated run.
#     The CLI prints `WARNING: Error loading config file: ... permission denied`
#     and then mis-dispatches the build (`unknown shorthand flag: 't' in -t`).
#   - BuildKit is requested through inherited DOCKER_BUILDKIT=1 but buildx is
#     missing or broken. This image is `FROM debian` + `COPY`, so it needs none
#     of BuildKit's cache mounts and the classic builder does just as well.
#
# Each attempt runs in a subshell so its env override cannot leak into the
# source-build path that follows.
bitfun_build_runtime_image() {
  local image="$1" context="$2" rc=1

  # A config dir this user definitely owns. Empty if it cannot be created, in
  # which case the retries keep the inherited DOCKER_CONFIG.
  local clean_config="$context.docker-config"
  rm -rf "$clean_config"
  if ! mkdir -p "$clean_config" 2>/dev/null; then
    clean_config=""
  fi

  local attempt
  for attempt in inherited clean-config classic-builder; do
    case "$attempt" in
      clean-config)
        if [ -z "$clean_config" ]; then continue; fi
        echo ">>> Retrying the runtime image build with a clean Docker config..."
        ;;
      classic-builder)
        echo ">>> Retrying the runtime image build with the classic builder..."
        ;;
    esac
    # Subshell: the env overrides must not leak into the source-build path.
    if (
      case "$attempt" in
        clean-config) export DOCKER_CONFIG="$clean_config" ;;
        classic-builder)
          if [ -n "$clean_config" ]; then export DOCKER_CONFIG="$clean_config"; fi
          export DOCKER_BUILDKIT=0
          ;;
      esac
      bitfun_docker build -t "$image" "$context"
    ); then
      rc=0
      break
    fi
  done

  if [ -n "$clean_config" ]; then
    rm -rf "$clean_config"
  fi
  return "$rc"
}

bitfun_try_release_deploy() {
  local release_dir="$HOME/.bitfun/relay-release"
  local target archive upstream_url download_dir extracted context image expected_hash
  case "$(uname -m 2>/dev/null)" in
    x86_64 | amd64) target="x86_64-unknown-linux-gnu" ;;
    aarch64 | arm64) target="aarch64-unknown-linux-gnu" ;;
    *)
      echo ">>> No published Relay binary for architecture $(uname -m); using source build."
      return 1
      ;;
  esac

  archive="bitfun-relay-server-${target}.tar.gz"
  upstream_url="$(bitfun_release_asset_url "$BITFUN_RELEASE_TAG" "$archive")"
  case "$target" in
    x86_64-unknown-linux-gnu) expected_hash="${BITFUN_EXPECTED_SHA256_X86_64_UNKNOWN_LINUX_GNU:-}" ;;
    aarch64-unknown-linux-gnu) expected_hash="${BITFUN_EXPECTED_SHA256_AARCH64_UNKNOWN_LINUX_GNU:-}" ;;
    *) expected_hash="" ;;
  esac
  if [ -n "$expected_hash" ]; then
    echo ">>> Using a signature-verified checksum supplied by the client."
  else
    echo ">>> No signature-verified checksum available; falling back to the canonical GitHub checksum."
  fi
  mkdir -p "$release_dir"
  chmod 700 "$release_dir" 2>/dev/null || true
  download_dir="$(mktemp -d "$release_dir/download.XXXXXX")"

  bitfun_verify_release_archive() {
    (
      cd "$download_dir" || return 1
      if command -v sha256sum >/dev/null 2>&1; then
        sha256sum -c "${archive}.sha256"
      elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 -c "${archive}.sha256"
      else
        echo "ERROR: sha256sum or shasum is required to verify the Relay release." >&2
        return 1
      fi
    )
  }

  # Fetch the checksum, preferring the canonical GitHub copy over the one the
  # download origin offers. Falls back to same-origin only when GitHub cannot be
  # reached at all, and says so — that is a materially weaker guarantee.
  bitfun_fetch_release_checksum() {
    local url="$1" canonical
    # Strongest case: the caller already verified a signature over the checksum
    # and passed the hash down. Nothing fetched from any origin can override it,
    # so a hostile mirror is out of the picture — and this host needs no
    # minisign, only sha256sum.
    if [ -n "$expected_hash" ]; then
      printf '%s  %s\n' "$expected_hash" "$archive" >"$download_dir/${archive}.sha256"
      return 0
    fi
    canonical="$(bitfun_canonical_checksum_url "$url")"
    rm -f "$download_dir/${archive}.sha256"
    if curl -fsSL --retry 3 --connect-timeout 15 --max-time 60 \
      -o "$download_dir/${archive}.sha256" "$canonical"; then
      return 0
    fi
    if [ "$canonical" = "${url}.sha256" ]; then
      return 1
    fi
    echo ">>> WARNING: could not reach the canonical checksum at ${canonical}."
    echo ">>>          Falling back to the checksum served by the download origin,"
    echo ">>>          which only detects corruption, not a tampered mirror."
    curl -fsSL --retry 3 --connect-timeout 15 --max-time 60 \
      -o "$download_dir/${archive}.sha256" "${url}.sha256"
  }

  # Measure a candidate by how many bytes it delivers inside a fixed window.
  # Bytes-in-fixed-time is throughput, so one ranged request ranks a source
  # without downloading the whole archive from one we may not use.
  bitfun_probe_source() {
    local url="$1" speed
    speed="$(curl -sSL --connect-timeout 5 --max-time "$BITFUN_PROBE_SECONDS" \
      -r "0-$((BITFUN_PROBE_BYTES - 1))" -o /dev/null \
      -w '%{speed_download}' "$url" 2>/dev/null || true)"
    speed="${speed%%.*}"
    case "$speed" in
      '' | *[!0-9]*) echo 0 ;;
      *) echo "$speed" ;;
    esac
  }

  bitfun_download_release_pair() {
    local url="$1"
    local watcher="" status=0
    local done_marker="$download_dir/.download-active"
    echo ">>> Downloading published Relay binary: $url"
    # Callers poll this log; without a heartbeat a slow link is
    # indistinguishable from a hang. Poll a sentinel on a short interval rather
    # than sleeping long: `kill` on a shell blocked in a long `sleep` is not
    # delivered until that sleep returns, which would stall every download by a
    # full tick after curl already finished.
    : >"$done_marker"
    (
      local ticks=0
      while [ -f "$done_marker" ]; do
        sleep 2
        ticks=$((ticks + 1))
        [ "$((ticks % 10))" -eq 0 ] || continue
        [ -f "$download_dir/$archive" ] || continue
        echo ">>>   ... $(du -h "$download_dir/$archive" 2>/dev/null | cut -f1) downloaded"
      done
    ) &
    watcher=$!
    # No --max-time on purpose. Any wall-clock ceiling reintroduces the original
    # bug one order of magnitude out: at the 8 KB/s floor a 30 MB archive needs
    # ~3750 s, so a 3600 s cap would kill a transfer that was progressing fine,
    # and --retry-max-time would already have refused to retry it. The
    # throughput floor is the give-up condition — it aborts a dead or hung link
    # within --speed-time, and --connect-timeout covers setup — so a ceiling
    # adds nothing but a cliff for slow users.
    curl -fsSL -C - \
      --retry 3 --retry-delay 3 --retry-max-time 0 \
      --connect-timeout 15 \
      --speed-limit "$BITFUN_STALL_BPS" --speed-time "$BITFUN_STALL_SECONDS" \
      -o "$download_dir/$archive" "$url" || status=$?
    rm -f "$done_marker"
    wait "$watcher" >/dev/null 2>&1 || true
    if [ "$status" -ne 0 ]; then
      echo ">>> Source failed or stalled below $((BITFUN_STALL_BPS / 1024)) KB/s (curl $status); trying the next source."
      return 1
    fi
    bitfun_fetch_release_checksum "$url" || return 1
    if bitfun_verify_release_archive; then
      return 0
    fi
    # Bad bytes, not a bad link: mark the partial file poisoned so the caller
    # discards it instead of resuming on top of it from the next source.
    : >"$download_dir/${archive}.verify-failed"
    return 1
  }

  # Candidate sources, one per line. Files rather than arrays: this runs on
  # whatever bash the target server has, and `"${empty[@]}"` under `set -u`
  # aborts on bash 4.2 (CentOS 7).
  local sources="$download_dir/sources.tsv" mirror_url="" probe speed best_speed
  : >"$sources.in"
  if [ "${BITFUN_MIRROR_MODE:-global}" = "cn" ] && [ -n "${BITFUN_GITHUB_PROXY:-}" ]; then
    printf '%s\n' "${BITFUN_GITHUB_PROXY%/}/${upstream_url}" >>"$sources.in"
  fi
  printf '%s\n' "$upstream_url" >>"$sources.in"
  # Take the mirror URL from the mirror's own manifest rather than building a
  # /<version>/ path: openbitfun keeps only the most recent releases, so a
  # pinned version 404s for every Desktop build that is not one of them.
  # `|| true`: an unreachable mirror or a non-matching manifest must leave this
  # empty, never abort the caller under `set -e`.
  mirror_url="$(curl -fsSL --connect-timeout 10 --max-time 30 \
    "${BITFUN_OPENBITFUN_RELEASE_BASE}/linux-binaries.json" 2>/dev/null |
    tr ',' '\n' | grep -F '"url"' | grep -F "$archive" |
    head -n 1 | sed -e 's/.*"url"[[:space:]]*:[[:space:]]*"//' -e 's/".*//' || true)"
  if [ -n "$mirror_url" ]; then
    printf '%s\n' "$mirror_url" >>"$sources.in"
  fi

  : >"$sources"
  while IFS= read -r probe; do
    [ -n "$probe" ] || continue
    speed="$(bitfun_probe_source "$probe")"
    echo ">>> Source probe: $((speed / 1024)) KB/s — $probe"
    printf '%s\t%s\n' "$speed" "$probe" >>"$sources"
  done <"$sources.in"

  if [ ! -s "$sources" ]; then
    echo ">>> No Relay binary source responded; falling back to source build."
    rm -rf "$download_dir"
    return 1
  fi
  sort -rn -k1,1 -o "$sources" "$sources"
  best_speed="$(head -n 1 "$sources" | cut -f1)"
  if [ "${best_speed:-0}" -lt "$BITFUN_HEALTHY_BPS" ]; then
    echo ">>> Fastest source is $((${best_speed:-0} / 1024)) KB/s, under the $((BITFUN_HEALTHY_BPS / 1024)) KB/s bar; continuing anyway — a slow download still beats a source rebuild."
  fi

  # Try fastest first. Every source serves the identical artifact, so a partial
  # file is reused across sources too (`-C -`); only a checksum mismatch, which
  # means the bytes really are bad, wipes it and starts the next source clean.
  local ok=0
  while IFS=$'\t' read -r speed probe; do
    [ -n "$probe" ] || continue
    if bitfun_download_release_pair "$probe"; then
      ok=1
      break
    fi
    if [ -f "$download_dir/${archive}.verify-failed" ]; then
      rm -f "$download_dir/$archive" "$download_dir/${archive}.verify-failed"
    fi
  done <"$sources"
  if [ "$ok" -ne 1 ]; then
    echo ">>> Published Relay binary unavailable from every source; falling back to source build."
    rm -rf "$download_dir"
    return 1
  fi

  mkdir -p "$download_dir/extracted"
  if ! tar xzf "$download_dir/$archive" -C "$download_dir/extracted"; then
    echo ">>> Published Relay archive could not be extracted; falling back to source build."
    rm -rf "$download_dir"
    return 1
  fi
  extracted="$(find "$download_dir/extracted" -mindepth 1 -maxdepth 1 -type d \
    -name 'bitfun-relay-server-*' | head -n 1)"
  if [ -z "$extracted" ] ||
    [ ! -x "$extracted/bitfun-relay-server" ] ||
    [ ! -x "$extracted/relay-admin" ] ||
    [ ! -f "$extracted/static/index.html" ]; then
    echo ">>> Published Relay archive layout is invalid; falling back to source build."
    rm -rf "$download_dir"
    return 1
  fi

  context="$release_dir/runtime"
  rm -rf "$context.new"
  mkdir -p "$context.new"
  cp "$extracted/bitfun-relay-server" "$extracted/relay-admin" "$context.new/"
  cp -R "$extracted/static" "$context.new/static"
  # Base image glibc must be >= what the *archive being installed* was linked
  # against — which is not the same as what CI builds today.
  #
  # arm64 releases up to and including v0.2.14 were built on ubuntu-24.04-arm and
  # require GLIBC_2.38. On bookworm-slim (2.36) they could not load at all: the
  # container exited instantly, the loader error went to stderr, and the deploy
  # surfaced only as a failed health check followed by a 20-minute rebuild.
  #
  # The release matrix now pins both arches to ubuntu-22.04 (glibc 2.35, asserted
  # by scripts/ci/check-glibc-floor.sh), but that does NOT make bookworm safe
  # again: Desktop pins BITFUN_RELEASE_TAG to its own version, so a v0.2.14
  # client installs the v0.2.14 archive forever, and published archives keep the
  # floor they were built with. This base must satisfy the highest floor across
  # every release a client in the wild might still install. trixie-slim carries
  # glibc 2.41 and covers both 2.38 and 2.35.
  cat >"$context.new/Dockerfile" <<'DOCKERFILE'
FROM debian:trixie-slim
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY bitfun-relay-server relay-admin /app/
COPY static /app/static
RUN chmod 755 /app/bitfun-relay-server /app/relay-admin \
    && mkdir -p /app/data /app/room-web
# Fail the build, loudly and in seconds, if either binary cannot be loaded here.
# `ldd` runs the real dynamic loader and prints the exact
# `version 'GLIBC_x.yz' not found` line — but it still exits 0, so its *output*
# is the gate, not its status. The relay binary itself is unusable as a probe:
# it has no --version flag and simply starts serving. Without this check a
# future runner bump reappears as an opaque failed health check plus a
# 20-minute source rebuild.
RUN set -eu; \
    for bin in /app/bitfun-relay-server /app/relay-admin; do \
      out="$(ldd "$bin" 2>&1)"; \
      printf '%s\n' "$out"; \
      case "$out" in \
        *"not found"*) \
          echo "ERROR: $bin cannot be loaded on this base image (see above)." >&2; \
          echo "       The published binary needs a newer glibc than this base provides." >&2; \
          exit 1 ;; \
      esac; \
    done
HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=5 \
  CMD curl -fsS "http://127.0.0.1:${RELAY_PORT:-9700}/health" || exit 1
CMD ["/app/bitfun-relay-server"]
DOCKERFILE
  rm -rf "$context"
  mv "$context.new" "$context"
  rm -rf "$download_dir"

  image="bitfun-relay:release-${BITFUN_RELEASE_TAG}"
  echo ">>> Building lightweight Relay runtime image (no Rust/Cargo compilation)..."
  if ! bitfun_build_runtime_image "$image" "$context"; then
    echo ">>> Published binary image build failed; falling back to source build."
    return 1
  fi

  bitfun_docker volume create relay-server_relay-db >/dev/null
  bitfun_docker volume create relay-server_room-web >/dev/null

  local backup_container=""
  if bitfun_docker container inspect bitfun-relay >/dev/null 2>&1; then
    backup_container="bitfun-relay-before-release-$$"
    bitfun_docker stop bitfun-relay >/dev/null 2>&1 || true
    if ! bitfun_docker rename bitfun-relay "$backup_container"; then
      echo ">>> Could not stage the existing Relay container; falling back to source build."
      bitfun_docker start bitfun-relay >/dev/null 2>&1 || true
      return 1
    fi
  fi

  bitfun_restore_previous_relay() {
    bitfun_docker rm -f bitfun-relay >/dev/null 2>&1 || true
    if [ -n "$backup_container" ]; then
      bitfun_docker rename "$backup_container" bitfun-relay >/dev/null 2>&1 || true
      bitfun_docker start bitfun-relay >/dev/null 2>&1 || true
    fi
  }

  # A cancelled wizard sends TERM/INT. Without a trap the user's relay would
  # stay stopped under its backup name and disappear from the "already
  # deployed" probe, so always put the previous container back.
  trap 'bitfun_restore_previous_relay; trap - INT TERM; exit 1' INT TERM

  echo ">>> Starting published Relay binary on port ${RELAY_PORT:-9700}..."
  if ! bitfun_docker run -d \
    --name bitfun-relay \
    --restart unless-stopped \
    --label com.docker.compose.project=relay-server \
    --label com.docker.compose.service=relay-server \
    -p "${RELAY_HOST_BIND_IP:-0.0.0.0}:${RELAY_PORT:-9700}:${RELAY_PORT:-9700}" \
    -e "RELAY_PORT=${RELAY_PORT:-9700}" \
    -e RELAY_STATIC_DIR=/app/static \
    -e RELAY_ROOM_WEB_DIR=/app/room-web \
    -e RELAY_ROOM_TTL=300 \
    -e RELAY_ASSET_STORE_MAX_BYTES=1073741824 \
    -e RELAY_DB_PATH=/app/data/bitfun_relay.db \
    -v relay-server_room-web:/app/room-web \
    -v relay-server_relay-db:/app/data \
    "$image" >/dev/null; then
    echo ">>> Published Relay binary could not start; restoring previous container."
    bitfun_restore_previous_relay
    trap - INT TERM
    return 1
  fi

  # Probe the address the container is actually published on; a wildcard bind
  # is reachable through loopback.
  local attempt stale probe_host="${RELAY_HOST_BIND_IP:-0.0.0.0}"
  if [ "$probe_host" = "0.0.0.0" ] || [ "$probe_host" = "::" ]; then
    probe_host="127.0.0.1"
  fi
  for attempt in $(seq 1 20); do
    if curl -fsS --max-time 3 "http://${probe_host}:${RELAY_PORT:-9700}/health" >/dev/null 2>&1; then
      trap - INT TERM
      if [ -n "$backup_container" ]; then
        bitfun_docker rm "$backup_container" >/dev/null 2>&1 || true
      fi
      # Sweep backups orphaned by an earlier interrupted release deploy.
      for stale in $(bitfun_docker ps -aq \
        --filter 'name=^bitfun-relay-before-release-' 2>/dev/null); do
        bitfun_docker rm -f "$stale" >/dev/null 2>&1 || true
      done
      echo ">>> Published Relay binary is healthy."
      return 0
    fi
    if ! bitfun_docker inspect -f '{{.State.Running}}' bitfun-relay 2>/dev/null |
      grep -qx true; then
      break
    fi
    sleep 2
  done

  echo ">>> Published Relay binary failed its health check; restoring previous container."
  # `docker logs` relays the container's stderr on *its own* stderr, so the
  # `2>/dev/null` that used to be here discarded exactly the output we need:
  # the relay logs through tracing, i.e. to stderr. Keep both streams, and say
  # whether the container died or was up but not answering — the two have
  # completely different causes.
  echo ">>> Container state: $(bitfun_docker inspect \
    -f 'running={{.State.Running}} exit={{.State.ExitCode}} oom={{.State.OOMKilled}} err={{.State.Error}}' \
    bitfun-relay 2>&1 || true)"
  echo ">>> Probed http://${probe_host}:${RELAY_PORT:-9700}/health"
  echo ">>> Last 40 log lines from bitfun-relay:"
  bitfun_docker logs --tail 40 bitfun-relay 2>&1 | sed 's/^/    /' || true
  bitfun_restore_previous_relay
  trap - INT TERM
  return 1
}

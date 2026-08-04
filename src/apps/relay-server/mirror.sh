#!/usr/bin/env bash
# BitFun Relay deploy — region detection and China mirror configuration.
#
# Source this file, then call:
#   bitfun_mirror_init [--cn-mirror|--global-mirror]
#
# Or execute directly:
#   bash mirror.sh [--cn-mirror|--global-mirror]
#
# Environment:
#   BITFUN_MIRROR=auto|cn|global
#   BITFUN_APT_MIRROR=mirrors.aliyun.com
#   BITFUN_DOCKER_REGISTRY_MIRRORS="https://docker.1ms.run https://dockerproxy.net https://docker.m.daocloud.io"
#   BITFUN_CARGO_SPARSE_URL=sparse+https://rsproxy.cn/index/
#   BITFUN_RUSTUP_DIST_SERVER=https://rsproxy.cn
#   BITFUN_GITHUB_PROXY=https://ghfast.top/
#   BITFUN_DOCKER_INSTALL_URL=   # optional full URL override for get.docker.com script
#
# Sets / exports (when mode=cn):
#   BITFUN_MIRROR_MODE=cn|global
#   BITFUN_MIRROR_REQUESTED_MODE=auto|cn|global
#   BITFUN_MIRROR_REASON=<human-readable-resolution-reason>
#   BITFUN_USE_CN_MIRROR=0|1
#   BITFUN_GITHUB_GIT_URL / BITFUN_GITHUB_TARBALL_URL
#   BITFUN_DOCKER_GET_URL
#   BITFUN_APT_MIRROR / BITFUN_CARGO_SPARSE_URL / BITFUN_DOCKER_REGISTRY_MIRRORS
#   RUSTUP_DIST_SERVER / RUSTUP_UPDATE_ROOT (cn only)

# shellcheck disable=SC2034

bitfun_mirror_default_docker_mirrors() {
  # Order from Beijing CN re-probe (2026-07-25):
  # - 1ms: fastest digests for hello-world/debian/rust (~0.5s)
  # - dockerproxy.net: stable digests (~1.5-2.5s)
  # - daocloud: usable fallback (occasionally slower digest)
  # xuanyuan free tier dropped: TOOMANYREQUESTS on debian/rust
  echo "https://docker.1ms.run https://dockerproxy.net https://docker.m.daocloud.io"
}

bitfun_mirror_normalize_list() {
  # Portable: BSD/GNU sed differ on \n in character classes; use tr.
  echo "$1" | tr ',\t\n' '   ' | tr -s ' ' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//'
}

bitfun_mirror_priv() {
  if [ "$(id -u)" = "0" ]; then
    "$@"
  elif command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then
    sudo -n "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    "$@"
  fi
}

bitfun_mirror_parse_args() {
  local arg
  for arg in "$@"; do
    case "$arg" in
      --cn-mirror)
        export BITFUN_MIRROR=cn
        ;;
      --global-mirror|--no-cn-mirror)
        export BITFUN_MIRROR=global
        ;;
      --skip-mirror-apply)
        export BITFUN_MIRROR_SKIP_APPLY=1
        ;;
    esac
  done
}

bitfun_mirror_http_ok() {
  local url="$1"
  local timeout="${2:-3}"
  if command -v curl >/dev/null 2>&1; then
    curl -fsS -m "$timeout" -o /dev/null "$url" >/dev/null 2>&1
    return
  fi
  if command -v wget >/dev/null 2>&1; then
    wget -q -T "$timeout" -O /dev/null "$url" >/dev/null 2>&1
    return
  fi
  if command -v python3 >/dev/null 2>&1; then
    python3 - "$url" "$timeout" >/dev/null 2>&1 <<'PY'
import sys
import urllib.request

with urllib.request.urlopen(sys.argv[1], timeout=float(sys.argv[2])) as response:
    response.read(1)
PY
    return
  fi
  return 1
}

bitfun_mirror_http_body() {
  local url="$1"
  local timeout="${2:-3}"
  if command -v curl >/dev/null 2>&1; then
    curl -fsS -m "$timeout" "$url" 2>/dev/null
    return
  fi
  if command -v wget >/dev/null 2>&1; then
    wget -q -T "$timeout" -O - "$url" 2>/dev/null
    return
  fi
  if command -v python3 >/dev/null 2>&1; then
    python3 - "$url" "$timeout" 2>/dev/null <<'PY'
import sys
import urllib.request

with urllib.request.urlopen(sys.argv[1], timeout=float(sys.argv[2])) as response:
    sys.stdout.buffer.write(response.read(65536))
PY
    return
  fi
  return 1
}

# Last-resort country lookup for minimal hosts without curl/wget/python.
# Uses bash /dev/tcp against the same plain-HTTP ip-api endpoint already used
# above, bounded by coreutils/busybox `timeout`.
bitfun_mirror_country_via_bash_tcp() {
  if ! command -v timeout >/dev/null 2>&1; then
    return 1
  fi
  local response code
  response="$(
    timeout 4 bash -c '
      exec 3<>/dev/tcp/ip-api.com/80 || exit 1
      printf "GET /line/?fields=countryCode HTTP/1.1\r\nHost: ip-api.com\r\nConnection: close\r\n\r\n" >&3
      cat <&3
    ' 2>/dev/null || true
  )"
  code="$(
    printf '%s' "$response" \
      | tr -d '\r' \
      | awk '
          body && /^[[:alpha:]][[:alpha:]]$/ { print toupper($0); exit }
          /^$/ { body=1 }
        '
  )"
  if [ "${#code}" -eq 2 ]; then
    echo "$code"
    return 0
  fi
  return 1
}

# Country lookups, HTTPS first.
#
# The answer decides whether this host gets Chinese apt/Docker/GitHub mirrors,
# so an on-path attacker who can rewrite a plain-HTTP body can choose that for
# us. The HTTP endpoints are kept only as a last resort for hosts where TLS is
# unavailable, and the mode they produce is announced as unverified.
bitfun_mirror_detect_country() {
  local code="" endpoint
  for endpoint in \
    "https://ipinfo.io/country" \
    "https://ifconfig.co/country-iso" \
    "https://api.country.is/"; do
    code="$(bitfun_mirror_http_body "$endpoint" 3 \
      | tr -d '[:space:]' \
      | tr '[:lower:]' '[:upper:]' \
      | sed -n 's/.*"COUNTRY":"\([A-Z][A-Z]\)".*/\1/p;s/^\([A-Z][A-Z]\)$/\1/p' \
      | head -n 1)"
    if [ "${#code}" -eq 2 ]; then
      echo "$code"
      return 0
    fi
  done
  code="$(bitfun_mirror_http_body "http://ip-api.com/line/?fields=countryCode" 3 | tr -d '[:space:]' | tr '[:lower:]' '[:upper:]')"
  if [ "${#code}" -eq 2 ]; then
    echo ">>> Region detect: only the unauthenticated HTTP lookup answered" >&2
    echo "$code"
    return 0
  fi
  code="$(bitfun_mirror_country_via_bash_tcp 2>/dev/null || true)"
  if [ "${#code}" -eq 2 ]; then
    echo ">>> Region detect: only the unauthenticated HTTP lookup answered" >&2
    echo "$code"
    return 0
  fi
  return 1
}

bitfun_mirror_timezone_suggests_cn() {
  local tz=""
  if [ -n "${TZ:-}" ]; then
    tz="$TZ"
  elif [ -f /etc/timezone ]; then
    tz="$(tr -d '[:space:]' </etc/timezone)"
  elif command -v timedatectl >/dev/null 2>&1; then
    tz="$(timedatectl show -p Timezone --value 2>/dev/null || true)"
  fi
  case "$tz" in
    Asia/Shanghai|Asia/Chongqing|Asia/Urumqi|Asia/Harbin|PRC)
      return 0
      ;;
  esac
  return 1
}

bitfun_mirror_connectivity_suggests_cn() {
  # GitHub hard to reach, but a mainland mirror works → likely CN.
  if bitfun_mirror_http_ok "https://mirrors.aliyun.com/" 4; then
    if ! bitfun_mirror_http_ok "https://github.com/" 4; then
      return 0
    fi
  fi
  return 1
}

# Resolve BITFUN_MIRROR_MODE to cn|global. Returns 0 always.
bitfun_mirror_resolve_mode() {
  local forced="${BITFUN_MIRROR:-auto}"
  forced="$(echo "$forced" | tr '[:upper:]' '[:lower:]')"
  case "$forced" in
    cn|china|zh|zh-cn|zh_cn|1|true|yes)
      export BITFUN_MIRROR_REQUESTED_MODE=cn
      export BITFUN_MIRROR_MODE=cn
      export BITFUN_MIRROR_REASON=forced-cn
      export BITFUN_USE_CN_MIRROR=1
      return 0
      ;;
    global|intl|international|off|0|false|no|overseas)
      export BITFUN_MIRROR_REQUESTED_MODE=global
      export BITFUN_MIRROR_MODE=global
      export BITFUN_MIRROR_REASON=forced-global
      export BITFUN_USE_CN_MIRROR=0
      return 0
      ;;
  esac
  export BITFUN_MIRROR_REQUESTED_MODE=auto

  # Already resolved in this shell (or exported by the caller). Detection costs
  # up to four HTTP lookups plus two 4s probes, and mirror.sh is initialised at
  # several points of one deploy; re-running it there is both slow and unstable,
  # because a network blip mid-deploy could flip the mode between steps.
  case "${BITFUN_MIRROR_MODE:-}" in
    cn)
      export BITFUN_MIRROR_REASON="${BITFUN_MIRROR_REASON:-cached-cn}"
      export BITFUN_USE_CN_MIRROR=1
      return 0
      ;;
    global)
      export BITFUN_MIRROR_REASON="${BITFUN_MIRROR_REASON:-cached-global}"
      export BITFUN_USE_CN_MIRROR=0
      return 0
      ;;
  esac

  local country=""
  country="$(bitfun_mirror_detect_country || true)"
  if [ "$country" = "CN" ]; then
    echo ">>> Region detect: public IP country=CN → China mirrors"
    export BITFUN_MIRROR_MODE=cn
    export BITFUN_MIRROR_REASON=public-ip-cn
    export BITFUN_USE_CN_MIRROR=1
    return 0
  fi

  # A resolved country code is a direct answer, so stop here. The heuristics
  # below exist for the case where there is no answer at all; letting them
  # override one turns a transient GitHub outage on a Frankfurt VPS into
  # rewritten apt sources, a Chinese Docker registry and GitHub traffic through
  # a third-party proxy — because mirrors.aliyun.com answers from anywhere.
  if [ -n "$country" ]; then
    echo ">>> Region detect: public IP country=${country} → global mirrors"
    export BITFUN_MIRROR_MODE=global
    export BITFUN_MIRROR_REASON="public-ip-${country}"
    export BITFUN_USE_CN_MIRROR=0
    return 0
  fi

  if bitfun_mirror_timezone_suggests_cn; then
    echo ">>> Region detect: timezone suggests mainland China → China mirrors"
    export BITFUN_MIRROR_MODE=cn
    export BITFUN_MIRROR_REASON=timezone-cn
    export BITFUN_USE_CN_MIRROR=1
    return 0
  fi

  if bitfun_mirror_connectivity_suggests_cn; then
    echo ">>> Region detect: GitHub unreachable + Aliyun reachable → China mirrors"
    export BITFUN_MIRROR_MODE=cn
    export BITFUN_MIRROR_REASON=connectivity-cn
    export BITFUN_USE_CN_MIRROR=1
    return 0
  fi

  echo ">>> Region detect: inconclusive → global mirrors"
  export BITFUN_MIRROR_MODE=global
  export BITFUN_MIRROR_REASON=inconclusive-global
  export BITFUN_USE_CN_MIRROR=0
  return 0
}

bitfun_mirror_export_urls() {
  local git_upstream="${BITFUN_REPO_GIT_URL:-https://github.com/GCWing/BitFun.git}"
  local tarball_upstream="${BITFUN_REPO_TARBALL_URL:-https://github.com/GCWing/BitFun/archive/refs/heads/main.tar.gz}"
  local proxy="${BITFUN_GITHUB_PROXY:-https://ghfast.top/}"
  local docker_get_upstream="${BITFUN_DOCKER_INSTALL_URL:-https://get.docker.com}"

  export BITFUN_APT_MIRROR="${BITFUN_APT_MIRROR:-mirrors.aliyun.com}"
  export BITFUN_CARGO_SPARSE_URL="${BITFUN_CARGO_SPARSE_URL:-sparse+https://rsproxy.cn/index/}"
  export BITFUN_RUSTUP_DIST_SERVER="${BITFUN_RUSTUP_DIST_SERVER:-https://rsproxy.cn}"
  export BITFUN_DOCKER_REGISTRY_MIRRORS
  BITFUN_DOCKER_REGISTRY_MIRRORS="$(bitfun_mirror_normalize_list "${BITFUN_DOCKER_REGISTRY_MIRRORS:-$(bitfun_mirror_default_docker_mirrors)}")"

  if [ "${BITFUN_MIRROR_MODE:-global}" != "cn" ]; then
    export BITFUN_GITHUB_GIT_URL="$git_upstream"
    export BITFUN_GITHUB_TARBALL_URL="$tarball_upstream"
    export BITFUN_DOCKER_GET_URL="$docker_get_upstream"
    export BITFUN_USE_CN_MIRROR=0
    return 0
  fi

  case "$proxy" in
    */) ;;
    *) proxy="${proxy}/" ;;
  esac
  export BITFUN_GITHUB_PROXY="$proxy"

  # Prefix-style proxy: https://ghfast.top/https://github.com/...
  if [[ "$git_upstream" == https://github.com/* ]] || [[ "$git_upstream" == http://github.com/* ]]; then
    export BITFUN_GITHUB_GIT_URL="${proxy}${git_upstream}"
  else
    export BITFUN_GITHUB_GIT_URL="$git_upstream"
  fi
  if [[ "$tarball_upstream" == https://github.com/* ]] || [[ "$tarball_upstream" == http://github.com/* ]]; then
    export BITFUN_GITHUB_TARBALL_URL="${proxy}${tarball_upstream}"
  else
    export BITFUN_GITHUB_TARBALL_URL="$tarball_upstream"
  fi

  if [ -n "${BITFUN_DOCKER_INSTALL_URL:-}" ]; then
    export BITFUN_DOCKER_GET_URL="$BITFUN_DOCKER_INSTALL_URL"
  else
    # get.docker.com and most GitHub-prefix proxies return 403 from CN.
    # Prefer the upstream install script mirrored on jsDelivr (same docker/docker-install).
    export BITFUN_DOCKER_GET_URL="${BITFUN_DOCKER_GET_URL:-https://cdn.jsdelivr.net/gh/docker/docker-install@master/install.sh}"
  fi

  export RUSTUP_DIST_SERVER="$BITFUN_RUSTUP_DIST_SERVER"
  export RUSTUP_UPDATE_ROOT="${BITFUN_RUSTUP_UPDATE_ROOT:-${BITFUN_RUSTUP_DIST_SERVER}/rustup}"
  export BITFUN_USE_CN_MIRROR=1
}

bitfun_mirror_backup_file() {
  local src="$1"
  local stamp="${BITFUN_MIRROR_BACKUP_STAMP:-$(date +%Y%m%d%H%M%S)}"
  local dest_dir="${2:-/etc/bitfun}"
  if [ ! -e "$src" ]; then
    return 0
  fi
  bitfun_mirror_priv mkdir -p "$dest_dir" 2>/dev/null || mkdir -p "$HOME/.bitfun/mirror-backup" 2>/dev/null || true
  local base dest
  base="$(basename "$src")"
  if bitfun_mirror_priv test -d "$dest_dir" 2>/dev/null; then
    dest="${dest_dir}/mirror-backup-${stamp}-${base}"
    bitfun_mirror_priv cp -a "$src" "$dest" 2>/dev/null || true
  else
    dest="$HOME/.bitfun/mirror-backup/mirror-backup-${stamp}-${base}"
    mkdir -p "$(dirname "$dest")" 2>/dev/null || true
    cp -a "$src" "$dest" 2>/dev/null || true
  fi
}

bitfun_mirror_chown_to_home_owner() {
  if [ ! -d "$HOME" ] || ! command -v stat >/dev/null 2>&1; then
    return 0
  fi
  local owner path_owner path
  owner="$(stat -c '%u:%g' "$HOME" 2>/dev/null || true)"
  if [ -z "$owner" ]; then
    return 0
  fi
  for path in "$@"; do
    path_owner="$(stat -c '%u:%g' "$path" 2>/dev/null || true)"
    if [ -z "$path_owner" ] || [ "$path_owner" = "$owner" ]; then
      continue
    fi
    if [ "$(id -u)" = "0" ]; then
      chown "$owner" "$path" 2>/dev/null || true
    else
      bitfun_mirror_priv chown "$owner" "$path" 2>/dev/null || true
    fi
  done
}

bitfun_mirror_file_cksum() {
  local path="$1"
  if ! command -v cksum >/dev/null 2>&1; then
    return 1
  fi
  cksum "$path" 2>/dev/null | awk '{print $1 " " $2}' \
    || bitfun_mirror_priv cksum "$path" 2>/dev/null | awk '{print $1 " " $2}'
}

# Scheme for the apt mirror we are about to write.
#
# `bitfun_mirror_init` runs before anything is installed, and minimal cloud
# images ship without `ca-certificates` — that is precisely why their stock
# sources are `http://`. Writing `https://` there breaks the very `apt-get
# update` that would install the CA bundle, and it fails with a TLS error that
# says nothing about mirrors. Package signatures are what protect apt, so http
# costs integrity nothing; use https only when the host can actually verify it.
bitfun_mirror_apt_scheme() {
  if [ -n "${BITFUN_APT_SCHEME:-}" ]; then
    printf '%s' "$BITFUN_APT_SCHEME"
    return 0
  fi
  if [ -s /etc/ssl/certs/ca-certificates.crt ] \
    || [ -s /etc/pki/tls/certs/ca-bundle.crt ]; then
    if [ -f /usr/lib/apt/methods/https ] || [ -f /usr/libexec/apt/methods/https ]; then
      printf 'https'
      return 0
    fi
    # Modern apt has https built into the http method; only older releases ship
    # a separate binary, so treat its absence as conclusive only there.
    if ! command -v apt-get >/dev/null 2>&1 \
      || apt-get --version 2>/dev/null | grep -Eq 'apt 2\.|apt 1\.[6-9]'; then
      printf 'https'
      return 0
    fi
  fi
  printf 'http'
}

bitfun_mirror_apply_apt_debian_family() {
  local mirror="${BITFUN_APT_MIRROR:-mirrors.aliyun.com}"
  local scheme
  scheme="$(bitfun_mirror_apt_scheme)"
  local id="" version_codename="" id_like=""
  # shellcheck disable=SC1091
  . /etc/os-release 2>/dev/null || true
  id="${ID:-}"
  version_codename="${VERSION_CODENAME:-}"
  id_like="${ID_LIKE:-}"

  if [ -z "$version_codename" ]; then
    echo ">>> apt mirror: skip (missing VERSION_CODENAME)"
    return 0
  fi

  local suite_security=""
  case "$id" in
    ubuntu)
      suite_security="${version_codename}-security"
      ;;
    debian)
      suite_security="${version_codename}-security"
      ;;
    *)
      case "$id_like" in
        *ubuntu*)
          id=ubuntu
          suite_security="${version_codename}-security"
          ;;
        *debian*)
          id=debian
          suite_security="${version_codename}-security"
          ;;
        *)
          echo ">>> apt mirror: unsupported distro '${id}'; rewriting common hosts only"
          ;;
      esac
      ;;
  esac

  bitfun_mirror_priv mkdir -p /etc/apt/sources.list.d /etc/bitfun 2>/dev/null || true
  if [ -f /etc/apt/sources.list ]; then
    bitfun_mirror_backup_file /etc/apt/sources.list
  fi

  # Prefer a BitFun-owned list so cloud-init vendor files stay intact.
  local list_file="/etc/apt/sources.list.d/bitfun-cn-mirror.list"
  local tmp
  tmp="$(mktemp)"
  case "$id" in
    ubuntu)
      cat >"$tmp" <<EOF
# Managed by BitFun relay deploy (China mirrors). Safe to delete to revert.
deb ${scheme}://${mirror}/ubuntu/ ${version_codename} main restricted universe multiverse
deb ${scheme}://${mirror}/ubuntu/ ${version_codename}-updates main restricted universe multiverse
deb ${scheme}://${mirror}/ubuntu/ ${version_codename}-backports main restricted universe multiverse
deb ${scheme}://${mirror}/ubuntu/ ${suite_security} main restricted universe multiverse
EOF
      ;;
    debian)
      cat >"$tmp" <<EOF
# Managed by BitFun relay deploy (China mirrors). Safe to delete to revert.
deb ${scheme}://${mirror}/debian/ ${version_codename} main contrib non-free non-free-firmware
deb ${scheme}://${mirror}/debian/ ${version_codename}-updates main contrib non-free non-free-firmware
deb ${scheme}://${mirror}/debian-security ${suite_security} main contrib non-free non-free-firmware
EOF
      ;;
    *)
      # Unknown distro: the suites can only come from the file that is already
      # there. Write the rewrite into the BitFun-owned list and disable the
      # original by rename rather than overwriting it in place — an in-place
      # edit is invisible to `bitfun_mirror_restore_apt`, which leaves the host
      # permanently pinned to Chinese mirrors with only a timestamped backup the
      # operator has to find by hand.
      if [ ! -f /etc/apt/sources.list ]; then
        rm -f "$tmp"
        echo ">>> apt mirror: skip (no /etc/apt/sources.list to rewrite)"
        return 0
      fi
      {
        echo "# Managed by BitFun relay deploy (China mirrors). Safe to delete to revert."
        sed -e "s|deb.debian.org/debian|${mirror}/debian|g" \
          -e "s|security.debian.org/debian-security|${mirror}/debian-security|g" \
          -e "s|archive.ubuntu.com/ubuntu|${mirror}/ubuntu|g" \
          -e "s|security.ubuntu.com/ubuntu|${mirror}/ubuntu|g" \
          /etc/apt/sources.list
      } >"$tmp"
      bitfun_mirror_priv cp "$tmp" "$list_file"
      rm -f "$tmp"
      bitfun_mirror_backup_file /etc/apt/sources.list
      if [ -e /etc/apt/sources.list.bitfun-disabled ]; then
        # A previous deploy already saved the real upstream sources here.
        # Overwriting it would destroy the only copy `restore_apt` can put back,
        # so drop the current file (already mirrored, and preserved as a
        # timestamped backup above) instead of promoting it to "the original".
        bitfun_mirror_priv rm -f /etc/apt/sources.list 2>/dev/null || true
      elif ! bitfun_mirror_priv mv /etc/apt/sources.list /etc/apt/sources.list.bitfun-disabled; then
        # Could not disable the original, so both lists would be active and the
        # overseas hosts would still be tried. Undo instead of half-applying.
        bitfun_mirror_priv rm -f "$list_file" 2>/dev/null || true
        echo ">>> apt mirror: could not disable /etc/apt/sources.list; left untouched" >&2
        return 1
      fi
      echo ">>> apt mirror: rewrote common upstream hosts → ${mirror} (${list_file})"
      return 0
      ;;
  esac

  bitfun_mirror_priv cp "$tmp" "$list_file"
  rm -f "$tmp"

  # Disable conflicting default lists that still point overseas (keep backups).
  local f
  for f in /etc/apt/sources.list /etc/apt/sources.list.d/debian.sources \
    /etc/apt/sources.list.d/ubuntu.sources /etc/apt/sources.list.d/official-package-repositories.list; do
    if [ -f "$f" ] && grep -Eq 'deb\.debian\.org|security\.debian\.org|archive\.ubuntu\.com|security\.ubuntu\.com' "$f" 2>/dev/null; then
      bitfun_mirror_backup_file "$f"
      bitfun_mirror_priv mv "$f" "${f}.bitfun-disabled" 2>/dev/null || true
    fi
  done

  echo ">>> apt mirror: enabled ${list_file} → ${mirror}"
}

# Bounded `apt-get update` against whatever sources are currently active.
bitfun_mirror_probe_apt() {
  local timeout_prefix=""
  if command -v timeout >/dev/null 2>&1; then
    timeout_prefix="timeout 180"
  fi
  # shellcheck disable=SC2086
  bitfun_mirror_priv $timeout_prefix apt-get \
    -o Acquire::Retries=1 \
    -o Acquire::http::Timeout=15 \
    -o Acquire::https::Timeout=15 \
    update >/dev/null 2>&1
}

bitfun_mirror_apply_apt() {
  if ! command -v apt-get >/dev/null 2>&1; then
    return 0
  fi
  if [ ! -f /etc/os-release ]; then
    return 0
  fi
  if ! bitfun_mirror_apply_apt_debian_family; then
    echo ">>> apt mirror: apply failed (continuing)" >&2
    return 0
  fi

  # A write that succeeds still proves nothing about the mirror. Without this
  # probe the first symptom is an `apt-get update` failure several steps later,
  # with nothing tying it back to the sources BitFun swapped in.
  if bitfun_mirror_probe_apt; then
    return 0
  fi
  echo ">>> apt mirror: ${BITFUN_APT_MIRROR:-mirrors.aliyun.com} did not answer a test \
apt-get update; restoring the previous sources" >&2
  bitfun_mirror_restore_apt || true
  if ! bitfun_mirror_probe_apt; then
    echo ">>> apt mirror: apt-get update still fails after restoring the original \
sources, so the failure is not the mirror" >&2
  fi
  return 0
}

bitfun_mirror_write_docker_daemon_json() {
  local mirrors_csv="$1"
  local tmp py added_tmp state_dir state_file created_state prior_added daemon_json mirror checksum
  tmp="$(mktemp)"
  py="$(mktemp)"
  added_tmp="$(mktemp)"
  daemon_json="${BITFUN_DOCKER_DAEMON_JSON:-/etc/docker/daemon.json}"
  state_dir="$HOME/.bitfun/mirror-state"
  state_file="${state_dir}/docker-added-mirrors"
  created_state="${state_dir}/docker-daemon-created.cksum"
  mkdir -p "$state_dir" 2>/dev/null || true
  prior_added=""
  if [ -f "$state_file" ]; then
    prior_added="$(cat "$state_file" 2>/dev/null || bitfun_mirror_priv cat "$state_file" 2>/dev/null || true)"
  fi
  cat >"$py" <<'PY'
import json, os, sys
path = sys.argv[5]
mirrors = [m for m in sys.argv[1].split() if m]
prior_added = [m for m in sys.argv[3].split() if m]
data = {}
if os.path.exists(path):
    try:
        with open(path, "r", encoding="utf-8") as f:
            raw = f.read().strip()
        if raw:
            data = json.loads(raw)
            if not isinstance(data, dict):
                raise ValueError("daemon.json root must be an object")
    except Exception as exc:
        print(f"cannot safely parse {path}: {exc}", file=sys.stderr)
        sys.exit(2)
existing = data.get("registry-mirrors") or []
if not isinstance(existing, list):
    print("daemon.json registry-mirrors must be an array", file=sys.stderr)
    sys.exit(2)
legacy_managed = bool(data.pop("bitfun-cn-mirror", False))
merged = []
for item in list(existing) + mirrors:
    if item and item not in merged:
        merged.append(item)
data["registry-mirrors"] = merged
added = []
for item in prior_added:
    if item and item not in added:
        added.append(item)
for item in mirrors:
    if item not in existing or legacy_managed:
        if item not in added:
            added.append(item)
out = sys.argv[2]
with open(out, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
with open(sys.argv[4], "w", encoding="utf-8") as f:
    for item in added:
        f.write(item + "\n")
PY
  if command -v python3 >/dev/null 2>&1; then
    if [ -f "$daemon_json" ]; then
      bitfun_mirror_backup_file "$daemon_json"
    fi
    if ! bitfun_mirror_priv mkdir -p "$(dirname "$daemon_json")"; then
      echo ">>> docker mirror: cannot create $(dirname "$daemon_json")" >&2
      rm -f "$tmp" "$py" "$added_tmp"
      return 1
    fi
    if bitfun_mirror_priv python3 "$py" "$mirrors_csv" "$tmp" "$prior_added" "$added_tmp" "$daemon_json"; then
      if command -v dockerd >/dev/null 2>&1 \
        && ! bitfun_mirror_priv dockerd --validate --config-file "$tmp" >/dev/null; then
        echo ">>> docker mirror: generated daemon.json failed dockerd validation; not installing it" >&2
        rm -f "$tmp" "$py" "$added_tmp"
        return 1
      fi
      if ! bitfun_mirror_priv cp "$tmp" "$daemon_json"; then
        echo ">>> docker mirror: cannot install ${daemon_json}" >&2
        rm -f "$tmp" "$py" "$added_tmp"
        return 1
      fi
      if cp "$added_tmp" "$state_file" 2>/dev/null \
        || bitfun_mirror_priv cp "$added_tmp" "$state_file" 2>/dev/null; then
        :
      else
        echo ">>> docker mirror: could not persist rollback state" >&2
        rm -f "$state_dir/version" 2>/dev/null \
          || bitfun_mirror_priv rm -f "$state_dir/version" 2>/dev/null \
          || true
      fi
      chmod 644 "$state_file" 2>/dev/null || true
      bitfun_mirror_chown_to_home_owner "$state_dir" "$state_file"
      if [ -f "$created_state" ]; then
        checksum="$(bitfun_mirror_file_cksum "$daemon_json" || true)"
        if [ -n "$checksum" ]; then
          echo "$checksum" >"$created_state"
          chmod 644 "$created_state" 2>/dev/null || true
          bitfun_mirror_chown_to_home_owner "$created_state"
        fi
      fi
      echo ">>> docker mirror: merged registry-mirrors into ${daemon_json}"
    else
      echo ">>> docker mirror: safe JSON merge failed; leaving daemon.json untouched" >&2
      rm -f "$tmp" "$py" "$added_tmp"
      return 1
    fi
  else
    if [ -f "$daemon_json" ]; then
      echo ">>> docker mirror: python3 missing; leaving existing daemon.json untouched" >&2
      rm -f "$tmp" "$py" "$added_tmp"
      return 1
    else
      if ! bitfun_mirror_priv mkdir -p "$(dirname "$daemon_json")"; then
        echo ">>> docker mirror: cannot create $(dirname "$daemon_json")" >&2
        rm -f "$tmp" "$py" "$added_tmp"
        return 1
      fi
      {
        echo '{'
        echo '  "registry-mirrors": ['
        local first=1 m
        for m in $mirrors_csv; do
          if [ "$first" -eq 1 ]; then first=0; else echo ','; fi
          printf '    "%s"' "$m"
        done
        echo ''
        echo '  ]'
        echo '}'
      } >"$tmp"
      if ! bitfun_mirror_priv cp "$tmp" "$daemon_json"; then
        echo ">>> docker mirror: cannot install ${daemon_json}" >&2
        rm -f "$tmp" "$py" "$added_tmp"
        return 1
      fi
      : >"$added_tmp"
      for mirror in $mirrors_csv; do
        printf '%s\n' "$mirror" >>"$added_tmp"
      done
      if cp "$added_tmp" "$state_file" 2>/dev/null \
        || bitfun_mirror_priv cp "$added_tmp" "$state_file" 2>/dev/null; then
        :
      else
        echo ">>> docker mirror: could not persist rollback state" >&2
        rm -f "$state_dir/version" 2>/dev/null \
          || bitfun_mirror_priv rm -f "$state_dir/version" 2>/dev/null \
          || true
      fi
      chmod 644 "$state_file" 2>/dev/null || true
      bitfun_mirror_chown_to_home_owner "$state_dir" "$state_file"
      checksum="$(bitfun_mirror_file_cksum "$daemon_json" || true)"
      if [ -n "$checksum" ]; then
        if echo "$checksum" >"$created_state"; then
          chmod 644 "$created_state" 2>/dev/null || true
          bitfun_mirror_chown_to_home_owner "$created_state"
        else
          echo ">>> docker mirror: could not persist created-file checksum for rollback" >&2
          rm -f "$state_dir/version" 2>/dev/null \
            || bitfun_mirror_priv rm -f "$state_dir/version" 2>/dev/null \
            || true
        fi
      fi
      echo ">>> docker mirror: wrote ${daemon_json}"
    fi
  fi
  rm -f "$tmp" "$py" "$added_tmp"
}

bitfun_mirror_restart_docker_if_needed() {
  if ! command -v docker >/dev/null 2>&1; then
    return 0
  fi
  if docker info >/dev/null 2>&1 || bitfun_mirror_priv docker info >/dev/null 2>&1; then
    echo ">>> docker mirror: restarting docker to apply registry-mirrors..."
    bitfun_mirror_priv systemctl restart docker 2>/dev/null \
      || bitfun_mirror_priv service docker restart 2>/dev/null \
      || true
    sleep 1
  fi
}

bitfun_mirror_apply_docker_daemon() {
  local mirrors
  mirrors="$(bitfun_mirror_normalize_list "${BITFUN_DOCKER_REGISTRY_MIRRORS:-$(bitfun_mirror_default_docker_mirrors)}")"
  if [ -z "$mirrors" ]; then
    return 0
  fi
  if bitfun_mirror_write_docker_daemon_json "$mirrors"; then
    bitfun_mirror_restart_docker_if_needed || true
  else
    echo ">>> docker mirror: apply failed (continuing)" >&2
  fi
}

bitfun_mirror_restore_apt() {
  local list_file="/etc/apt/sources.list.d/bitfun-cn-mirror.list"
  local changed=0 restore_failed=0 original disabled
  for original in /etc/apt/sources.list /etc/apt/sources.list.d/debian.sources \
    /etc/apt/sources.list.d/ubuntu.sources /etc/apt/sources.list.d/official-package-repositories.list; do
    disabled="${original}.bitfun-disabled"
    if [ ! -f "$disabled" ]; then
      continue
    fi
    if [ -e "$original" ]; then
      echo ">>> apt mirror: not restoring ${original}; a replacement already exists" >&2
      restore_failed=1
      continue
    fi
    if bitfun_mirror_priv mv "$disabled" "$original" 2>/dev/null; then
      changed=1
    else
      echo ">>> apt mirror: failed to restore ${original}" >&2
      restore_failed=1
    fi
  done
  if [ -f "$list_file" ] && [ "$restore_failed" -eq 0 ]; then
    if bitfun_mirror_priv rm -f "$list_file" 2>/dev/null; then
      changed=1
    else
      echo ">>> apt mirror: failed to remove ${list_file}" >&2
    fi
  elif [ -f "$list_file" ]; then
    echo ">>> apt mirror: keeping ${list_file} because an upstream source could not be restored" >&2
  fi
  if [ "$changed" -eq 1 ]; then
    echo ">>> apt mirror: removed BitFun mirror list and restored disabled upstream sources"
  fi
}

bitfun_mirror_remove_docker_daemon() {
  local daemon_json="${BITFUN_DOCKER_DAEMON_JSON:-/etc/docker/daemon.json}"
  local state_file="$HOME/.bitfun/mirror-state/docker-added-mirrors"
  local created_state="$HOME/.bitfun/mirror-state/docker-daemon-created.cksum"
  local version_file="$HOME/.bitfun/mirror-state/version"
  local mode_file="$HOME/.bitfun/mirror-mode"
  local managed=0 mirrors_to_remove="" tmp py status expected_checksum actual_checksum

  if [ -f "$state_file" ]; then
    managed=1
    mirrors_to_remove="$(cat "$state_file" 2>/dev/null || bitfun_mirror_priv cat "$state_file" 2>/dev/null || true)"
  elif [ ! -f "$version_file" ] && [ "$(cat "$mode_file" 2>/dev/null || true)" = "cn" ]; then
    # Compatibility cleanup for hosts touched by early versions that did not
    # record exactly which registry mirrors they added.
    managed=1
    mirrors_to_remove="$(bitfun_mirror_default_docker_mirrors)"
  elif [ -r "$daemon_json" ] && grep -q '"bitfun-cn-mirror"' "$daemon_json" 2>/dev/null; then
    managed=1
    mirrors_to_remove="$(bitfun_mirror_default_docker_mirrors)"
  fi
  if [ "$managed" -ne 1 ]; then
    if [ -f "$version_file" ] && [ ! -f "$state_file" ]; then
      rm -f "$version_file" "$created_state" 2>/dev/null \
        || bitfun_mirror_priv rm -f "$version_file" "$created_state" 2>/dev/null \
        || true
    fi
    return 0
  fi
  if [ ! -f "$daemon_json" ]; then
    rm -f "$state_file" "$version_file" "$created_state" 2>/dev/null \
      || bitfun_mirror_priv rm -f "$state_file" "$version_file" "$created_state" 2>/dev/null \
      || true
    return 0
  fi
  if ! command -v python3 >/dev/null 2>&1; then
    if [ -f "$created_state" ]; then
      expected_checksum="$(cat "$created_state" 2>/dev/null || true)"
      actual_checksum="$(bitfun_mirror_file_cksum "$daemon_json" || true)"
      if [ -n "$expected_checksum" ] && [ "$actual_checksum" = "$expected_checksum" ]; then
        bitfun_mirror_backup_file "$daemon_json"
        if bitfun_mirror_priv rm -f "$daemon_json"; then
          rm -f "$state_file" "$version_file" "$created_state" 2>/dev/null \
            || bitfun_mirror_priv rm -f "$state_file" "$version_file" "$created_state" 2>/dev/null \
            || true
          echo ">>> docker mirror: removed BitFun-created ${daemon_json}"
          if command -v docker >/dev/null 2>&1; then
            bitfun_mirror_priv systemctl restart docker 2>/dev/null \
              || bitfun_mirror_priv service docker restart 2>/dev/null \
              || true
          fi
          return 0
        fi
      fi
    fi
    echo ">>> docker mirror: python3 missing; cannot safely remove managed daemon.json entries" >&2
    return 1
  fi

  tmp="$(mktemp)"
  py="$(mktemp)"
  cat >"$py" <<'PY'
import json, os, sys

path = sys.argv[3]
remove = {m for m in sys.argv[1].split() if m}
try:
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)
except Exception as exc:
    print(f"cannot safely parse {path}: {exc}", file=sys.stderr)
    sys.exit(2)
if not isinstance(data, dict):
    print("daemon.json root must be an object", file=sys.stderr)
    sys.exit(2)
before = json.dumps(data, sort_keys=True)
data.pop("bitfun-cn-mirror", None)
existing = data.get("registry-mirrors")
if isinstance(existing, list):
    kept = [item for item in existing if item not in remove]
    if kept:
        data["registry-mirrors"] = kept
    else:
        data.pop("registry-mirrors", None)
after = json.dumps(data, sort_keys=True)
if before == after:
    sys.exit(3)
with open(sys.argv[2], "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY

  status=0
  bitfun_mirror_priv python3 "$py" "$mirrors_to_remove" "$tmp" "$daemon_json" || status=$?
  if [ "$status" -eq 3 ]; then
    rm -f "$tmp" "$py"
    rm -f "$state_file" 2>/dev/null || bitfun_mirror_priv rm -f "$state_file" 2>/dev/null || true
    rm -f "$version_file" 2>/dev/null || bitfun_mirror_priv rm -f "$version_file" 2>/dev/null || true
    rm -f "$created_state" 2>/dev/null || bitfun_mirror_priv rm -f "$created_state" 2>/dev/null || true
    return 0
  fi
  if [ "$status" -ne 0 ]; then
    echo ">>> docker mirror: failed to remove managed daemon.json entries" >&2
    rm -f "$tmp" "$py"
    return 1
  fi
  if command -v dockerd >/dev/null 2>&1 \
    && ! bitfun_mirror_priv dockerd --validate --config-file "$tmp" >/dev/null; then
    echo ">>> docker mirror: restored daemon.json failed dockerd validation; leaving current file untouched" >&2
    rm -f "$tmp" "$py"
    return 1
  fi
  bitfun_mirror_backup_file "$daemon_json"
  if ! bitfun_mirror_priv cp "$tmp" "$daemon_json"; then
    echo ">>> docker mirror: failed to restore ${daemon_json}" >&2
    rm -f "$tmp" "$py"
    return 1
  fi
  rm -f "$tmp" "$py"
  rm -f "$state_file" 2>/dev/null || bitfun_mirror_priv rm -f "$state_file" 2>/dev/null || true
  rm -f "$version_file" 2>/dev/null || bitfun_mirror_priv rm -f "$version_file" 2>/dev/null || true
  rm -f "$created_state" 2>/dev/null || bitfun_mirror_priv rm -f "$created_state" 2>/dev/null || true
  echo ">>> docker mirror: removed BitFun-managed registry mirrors from ${daemon_json}"
  if command -v docker >/dev/null 2>&1; then
    bitfun_mirror_priv systemctl restart docker 2>/dev/null \
      || bitfun_mirror_priv service docker restart 2>/dev/null \
      || true
  fi
}

bitfun_mirror_remove_cargo_managed_block() {
  local cargo_home="${CARGO_HOME:-$HOME/.cargo}"
  local cfg="${cargo_home}/config.toml"
  local tmp backup
  if [ ! -f "$cfg" ]; then
    return 0
  fi
  if ! grep -q '^# >>> BITFUN-CN-MIRROR$' "$cfg" 2>/dev/null \
    && ! bitfun_mirror_priv grep -q '^# >>> BITFUN-CN-MIRROR$' "$cfg" 2>/dev/null; then
    return 0
  fi
  mkdir -p "$HOME/.bitfun/mirror-backup" 2>/dev/null || true
  backup="$HOME/.bitfun/mirror-backup/cargo-config.toml.$(date +%Y%m%d%H%M%S)"
  if cp -a "$cfg" "$backup" 2>/dev/null || bitfun_mirror_priv cp -a "$cfg" "$backup" 2>/dev/null; then
    :
  else
    echo ">>> cargo mirror: cannot back up ${cfg}; leaving it untouched" >&2
    return 1
  fi
  bitfun_mirror_chown_to_home_owner "$HOME/.bitfun/mirror-backup" "$backup"
  tmp="$(mktemp)"
  # shellcheck disable=SC2016
  if ! awk '
      BEGIN {skip=0}
      /^# >>> BITFUN-CN-MIRROR$/ {skip=1; next}
      /^# <<< BITFUN-CN-MIRROR$/ {skip=0; next}
      skip==0 {print}
    ' "$cfg" >"$tmp" 2>/dev/null; then
    bitfun_mirror_priv awk '
        BEGIN {skip=0}
        /^# >>> BITFUN-CN-MIRROR$/ {skip=1; next}
        /^# <<< BITFUN-CN-MIRROR$/ {skip=0; next}
        skip==0 {print}
      ' "$cfg" >"$tmp"
  fi
  if cp "$tmp" "$cfg" 2>/dev/null || bitfun_mirror_priv cp "$tmp" "$cfg" 2>/dev/null; then
    :
  else
    echo ">>> cargo mirror: failed to remove legacy managed block from ${cfg}" >&2
    rm -f "$tmp"
    return 1
  fi
  bitfun_mirror_chown_to_home_owner "$cargo_home" "$cfg"
  rm -f "$tmp"
  echo ">>> cargo mirror: removed legacy BitFun block from ${cfg}; relay Cargo mirroring is build-local"
}

bitfun_mirror_restore_host() {
  echo ">>> Restoring global host sources managed by BitFun..."
  bitfun_mirror_restore_apt || true
  bitfun_mirror_remove_docker_daemon || true
  bitfun_mirror_remove_cargo_managed_block || true
}

bitfun_mirror_apply_host() {
  if [ "${BITFUN_MIRROR_SKIP_APPLY:-0}" = "1" ]; then
    echo ">>> mirror apply skipped (BITFUN_MIRROR_SKIP_APPLY=1)"
    return 0
  fi
  if [ "${BITFUN_MIRROR_MODE:-global}" != "cn" ]; then
    return 0
  fi
  mkdir -p "$HOME/.bitfun/mirror-state" 2>/dev/null || true
  echo "2" >"$HOME/.bitfun/mirror-state/version" 2>/dev/null || true
  bitfun_mirror_chown_to_home_owner \
    "$HOME/.bitfun" "$HOME/.bitfun/mirror-state" "$HOME/.bitfun/mirror-state/version"
  echo ">>> Applying China host mirrors (apt / docker; Cargo stays build-local)..."
  bitfun_mirror_apply_apt || true
  bitfun_mirror_apply_docker_daemon || true
  # Relay compilation happens inside Docker. Do not mutate the SSH user's
  # global Cargo config; older versions did and could create duplicate TOML
  # tables or root-owned ~/.cargo directories.
  bitfun_mirror_remove_cargo_managed_block || true
  mkdir -p "$HOME/.bitfun" 2>/dev/null || true
  echo "cn" >"$HOME/.bitfun/mirror-mode" 2>/dev/null || true
  bitfun_mirror_chown_to_home_owner "$HOME/.bitfun" "$HOME/.bitfun/mirror-mode"
}

# Undo a partially applied Aliyun docker-ce repository.
#
# The caller falls back to get.docker.com when this install fails, and that path
# runs its own `apt-get update` — which would pick up a half-written docker.list
# or an unusable docker.asc and fail on those instead, hiding the real error.
bitfun_mirror_cleanup_docker_aliyun_apt() {
  bitfun_mirror_priv rm -f /etc/apt/sources.list.d/docker.list /etc/apt/keyrings/docker.asc \
    2>/dev/null || true
}

# Install Docker Engine from Aliyun docker-ce (CN). Returns 0 on success.
bitfun_mirror_install_docker_aliyun() {
  if ! command -v apt-get >/dev/null 2>&1 && ! command -v dnf >/dev/null 2>&1 && ! command -v yum >/dev/null 2>&1; then
    return 1
  fi
  # shellcheck disable=SC1091
  . /etc/os-release 2>/dev/null || true
  local id="${ID:-}" version_codename="${VERSION_CODENAME:-}" arch
  arch="$(dpkg --print-architecture 2>/dev/null || uname -m)"
  case "$arch" in
    x86_64) arch=amd64 ;;
    aarch64) arch=arm64 ;;
  esac

  echo ">>> Installing Docker from Aliyun docker-ce mirror..."
  if command -v apt-get >/dev/null 2>&1; then
    local docker_ce_distro=""
    case "$id" in
      ubuntu|linuxmint|pop) docker_ce_distro=ubuntu ;;
      debian|raspbian) docker_ce_distro=debian ;;
      *)
        case "${ID_LIKE:-}" in
          *ubuntu*) docker_ce_distro=ubuntu ;;
          *debian*) docker_ce_distro=debian ;;
          *) return 1 ;;
        esac
        ;;
    esac
    [ -n "$version_codename" ] || return 1

    bitfun_mirror_priv apt-get update -y || true
    bitfun_mirror_priv apt-get install -y ca-certificates curl || {
      echo ">>> Aliyun docker-ce: could not install ca-certificates/curl" >&2
      return 1
    }
    bitfun_mirror_priv install -m 0755 -d /etc/apt/keyrings || return 1

    local key_tmp
    key_tmp="$(mktemp)"
    if ! curl -fsSL --retry 3 "https://mirrors.aliyun.com/docker-ce/linux/${docker_ce_distro}/gpg" \
      -o "$key_tmp" || [ ! -s "$key_tmp" ]; then
      # An empty or missing key silently produces a repository apt can never
      # verify, so stop before it is written anywhere.
      rm -f "$key_tmp"
      echo ">>> Aliyun docker-ce: GPG key download failed or was empty" >&2
      return 1
    fi
    bitfun_mirror_priv cp "$key_tmp" /etc/apt/keyrings/docker.asc || {
      rm -f "$key_tmp"
      bitfun_mirror_cleanup_docker_aliyun_apt
      return 1
    }
    rm -f "$key_tmp"
    bitfun_mirror_priv chmod a+r /etc/apt/keyrings/docker.asc || true
    echo "deb [arch=${arch} signed-by=/etc/apt/keyrings/docker.asc] https://mirrors.aliyun.com/docker-ce/linux/${docker_ce_distro} ${version_codename} stable" \
      | bitfun_mirror_priv tee /etc/apt/sources.list.d/docker.list >/dev/null || {
        bitfun_mirror_cleanup_docker_aliyun_apt
        return 1
      }
    if ! bitfun_mirror_priv apt-get update -y; then
      bitfun_mirror_cleanup_docker_aliyun_apt
      echo ">>> Aliyun docker-ce: apt-get update failed for the docker-ce repository" >&2
      return 1
    fi
    # `return 0` unconditionally here would report success on a failed install,
    # and the caller would skip its fallback and fail later at `systemctl enable
    # --now docker` with an error that says nothing about apt.
    if ! bitfun_mirror_priv apt-get install -y docker-ce docker-ce-cli containerd.io \
      docker-buildx-plugin docker-compose-plugin; then
      bitfun_mirror_cleanup_docker_aliyun_apt
      echo ">>> Aliyun docker-ce: package installation failed" >&2
      return 1
    fi
    return 0
  fi

  if command -v dnf >/dev/null 2>&1 || command -v yum >/dev/null 2>&1; then
    local pkg=yum
    command -v dnf >/dev/null 2>&1 && pkg=dnf
    bitfun_mirror_priv tee /etc/yum.repos.d/docker-ce.repo >/dev/null <<EOF
[docker-ce-stable]
name=Docker CE Stable - \$basearch
baseurl=https://mirrors.aliyun.com/docker-ce/linux/centos/\$releasever/\$basearch/stable
enabled=1
gpgcheck=1
gpgkey=https://mirrors.aliyun.com/docker-ce/linux/centos/gpg
EOF
    if ! bitfun_mirror_priv "$pkg" install -y docker-ce docker-ce-cli containerd.io \
      docker-buildx-plugin docker-compose-plugin; then
      bitfun_mirror_priv rm -f /etc/yum.repos.d/docker-ce.repo 2>/dev/null || true
      echo ">>> Aliyun docker-ce: package installation failed" >&2
      return 1
    fi
    return 0
  fi
  return 1
}

bitfun_mirror_sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" 2>/dev/null | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" 2>/dev/null | awk '{print $1}'
  else
    return 1
  fi
}

# Download the Docker install script to $1 using the CN-aware URL, and prove it
# is the upstream script before anyone runs it.
#
# The CN default is a jsDelivr copy of `docker/docker-install@master`: a floating
# ref, edge-cached for hours, fetched over a third-party CDN — and the result is
# executed as root. Nothing about that path is authenticated, so a compromise
# anywhere along it is a root shell on every host deployed while it lasts.
#
# Two ways out, in order:
#   * `BITFUN_DOCKER_INSTALL_SHA256` — an operator-pinned digest, checked exactly.
#   * cross-origin agreement — the same script fetched from a second, independent
#     origin must hash identically. One compromised mirror is then not enough.
#
# Neither is available on a host that can only reach the CN CDN, so
# `BITFUN_DOCKER_INSTALL_ALLOW_UNVERIFIED=1` exists as a deliberate, logged
# opt-out. It is not the default: the Aliyun docker-ce path above is GPG-verified
# and covers Debian/Ubuntu/RHEL, so failing closed here costs little.
bitfun_mirror_fetch_docker_install_script() {
  local dest="$1"
  local url="${BITFUN_DOCKER_GET_URL:-https://get.docker.com}"
  local expected="${BITFUN_DOCKER_INSTALL_SHA256:-}"
  local actual="" reference="" reference_url="" verified=0

  echo ">>> Fetching Docker install script: ${url}"
  curl -fsSL --retry 3 "$url" -o "$dest" || return 1

  actual="$(bitfun_mirror_sha256_of "$dest" || true)"
  if [ -z "$actual" ]; then
    echo ">>> Docker install script: no sha256 tool available; cannot verify" >&2
  elif [ -n "$expected" ]; then
    if [ "$actual" = "$expected" ]; then
      verified=1
    else
      echo ">>> Docker install script: sha256 ${actual} does not match the pinned \
${expected}; refusing to run it" >&2
      rm -f "$dest"
      return 1
    fi
  else
    local candidate
    for candidate in \
      "https://get.docker.com" \
      "https://raw.githubusercontent.com/docker/docker-install/master/install.sh"; do
      [ "$candidate" = "$url" ] && continue
      if curl -fsSL --retry 1 --max-time 30 "$candidate" -o "${dest}.crosscheck" 2>/dev/null; then
        reference="$(bitfun_mirror_sha256_of "${dest}.crosscheck" || true)"
        rm -f "${dest}.crosscheck"
        if [ -n "$reference" ] && [ "$reference" = "$actual" ]; then
          verified=1
          reference_url="$candidate"
          break
        fi
        if [ -n "$reference" ]; then
          echo ">>> Docker install script: ${candidate} serves different bytes \
(${reference} vs ${actual}); refusing to run it" >&2
          rm -f "$dest"
          return 1
        fi
      fi
      rm -f "${dest}.crosscheck"
    done
  fi

  if [ "$verified" -eq 1 ]; then
    if [ -n "$reference_url" ]; then
      echo ">>> Docker install script verified against ${reference_url} (sha256 ${actual})"
    else
      echo ">>> Docker install script matches the pinned sha256"
    fi
    return 0
  fi

  if [ "${BITFUN_DOCKER_INSTALL_ALLOW_UNVERIFIED:-0}" = "1" ]; then
    echo ">>> WARNING: running an unverified Docker install script from ${url} \
because BITFUN_DOCKER_INSTALL_ALLOW_UNVERIFIED=1 (sha256 ${actual:-unknown})" >&2
    return 0
  fi

  echo ">>> Docker install script from ${url} could not be verified against an \
independent origin. Install Docker with the distribution's own packages, set \
BITFUN_DOCKER_INSTALL_SHA256=${actual:-<sha256>} to pin this exact script, or set \
BITFUN_DOCKER_INSTALL_ALLOW_UNVERIFIED=1 to accept the risk." >&2
  rm -f "$dest"
  return 1
}

bitfun_mirror_init() {
  bitfun_mirror_parse_args "$@"
  bitfun_mirror_resolve_mode
  bitfun_mirror_export_urls
  echo ">>> Mirror mode: ${BITFUN_MIRROR_MODE} (requested=${BITFUN_MIRROR_REQUESTED_MODE:-auto}, reason=${BITFUN_MIRROR_REASON:-unknown}, BITFUN_USE_CN_MIRROR=${BITFUN_USE_CN_MIRROR})"
  if [ "${BITFUN_MIRROR_MODE}" = "cn" ]; then
    echo ">>> GitHub git URL:     ${BITFUN_GITHUB_GIT_URL}"
    echo ">>> GitHub tarball URL: ${BITFUN_GITHUB_TARBALL_URL}"
    echo ">>> Docker get URL:     ${BITFUN_DOCKER_GET_URL}"
    echo ">>> apt mirror:         ${BITFUN_APT_MIRROR}"
    echo ">>> cargo sparse:       ${BITFUN_CARGO_SPARSE_URL}"
    echo ">>> docker registries:  ${BITFUN_DOCKER_REGISTRY_MIRRORS}"
    bitfun_mirror_apply_host
  else
    bitfun_mirror_restore_host
    mkdir -p "$HOME/.bitfun" 2>/dev/null || true
    echo "global" >"$HOME/.bitfun/mirror-mode" 2>/dev/null || true
    bitfun_mirror_chown_to_home_owner "$HOME/.bitfun" "$HOME/.bitfun/mirror-mode"
  fi
}

# When executed directly as mirror.sh (not sourced, not text-embedded), run init.
# Basename guard prevents auto-run when this file is concatenated into Desktop
# driver scripts where BASH_SOURCE[0] == $0.
if [[ "${BASH_SOURCE[0]:-}" == "${0}" ]] \
  && [[ "$(basename "${BASH_SOURCE[0]}")" == "mirror.sh" ]]; then
  set -euo pipefail
  bitfun_mirror_init "$@"
fi

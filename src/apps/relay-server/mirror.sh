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
  if ! command -v curl >/dev/null 2>&1; then
    return 1
  fi
  curl -fsS -m "$timeout" -o /dev/null "$url" >/dev/null 2>&1
}

bitfun_mirror_http_body() {
  local url="$1"
  local timeout="${2:-3}"
  if ! command -v curl >/dev/null 2>&1; then
    return 1
  fi
  curl -fsS -m "$timeout" "$url" 2>/dev/null
}

bitfun_mirror_detect_country() {
  local code=""
  code="$(bitfun_mirror_http_body "https://ipinfo.io/country" 3 | tr -d '[:space:]' | tr '[:lower:]' '[:upper:]')"
  if [ "${#code}" -eq 2 ]; then
    echo "$code"
    return 0
  fi
  code="$(bitfun_mirror_http_body "http://ip-api.com/line/?fields=countryCode" 3 | tr -d '[:space:]' | tr '[:lower:]' '[:upper:]')"
  if [ "${#code}" -eq 2 ]; then
    echo "$code"
    return 0
  fi
  code="$(bitfun_mirror_http_body "https://ifconfig.co/country-iso" 3 | tr -d '[:space:]' | tr '[:lower:]' '[:upper:]')"
  if [ "${#code}" -eq 2 ]; then
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
      export BITFUN_MIRROR_MODE=cn
      export BITFUN_USE_CN_MIRROR=1
      return 0
      ;;
    global|intl|international|off|0|false|no|overseas)
      export BITFUN_MIRROR_MODE=global
      export BITFUN_USE_CN_MIRROR=0
      return 0
      ;;
  esac

  local country=""
  country="$(bitfun_mirror_detect_country || true)"
  if [ "$country" = "CN" ]; then
    echo ">>> Region detect: public IP country=CN → China mirrors"
    export BITFUN_MIRROR_MODE=cn
    export BITFUN_USE_CN_MIRROR=1
    return 0
  fi

  if bitfun_mirror_timezone_suggests_cn; then
    echo ">>> Region detect: timezone suggests mainland China → China mirrors"
    export BITFUN_MIRROR_MODE=cn
    export BITFUN_USE_CN_MIRROR=1
    return 0
  fi

  if bitfun_mirror_connectivity_suggests_cn; then
    echo ">>> Region detect: GitHub unreachable + Aliyun reachable → China mirrors"
    export BITFUN_MIRROR_MODE=cn
    export BITFUN_USE_CN_MIRROR=1
    return 0
  fi

  if [ -n "$country" ]; then
    echo ">>> Region detect: public IP country=${country} → global mirrors"
  else
    echo ">>> Region detect: inconclusive → global mirrors"
  fi
  export BITFUN_MIRROR_MODE=global
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

bitfun_mirror_apply_apt_debian_family() {
  local mirror="${BITFUN_APT_MIRROR:-mirrors.aliyun.com}"
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
deb https://${mirror}/ubuntu/ ${version_codename} main restricted universe multiverse
deb https://${mirror}/ubuntu/ ${version_codename}-updates main restricted universe multiverse
deb https://${mirror}/ubuntu/ ${version_codename}-backports main restricted universe multiverse
deb https://${mirror}/ubuntu/ ${suite_security} main restricted universe multiverse
EOF
      ;;
    debian)
      cat >"$tmp" <<EOF
# Managed by BitFun relay deploy (China mirrors). Safe to delete to revert.
deb https://${mirror}/debian/ ${version_codename} main contrib non-free non-free-firmware
deb https://${mirror}/debian/ ${version_codename}-updates main contrib non-free non-free-firmware
deb https://${mirror}/debian-security ${suite_security} main contrib non-free non-free-firmware
EOF
      ;;
    *)
      rm -f "$tmp"
      # Best-effort host rewrite for existing lists.
      if [ -f /etc/apt/sources.list ]; then
        local rewritten
        rewritten="$(mktemp)"
        sed -e "s|deb.debian.org/debian|${mirror}/debian|g" \
          -e "s|security.debian.org/debian-security|${mirror}/debian-security|g" \
          -e "s|archive.ubuntu.com/ubuntu|${mirror}/ubuntu|g" \
          -e "s|security.ubuntu.com/ubuntu|${mirror}/ubuntu|g" \
          /etc/apt/sources.list >"$rewritten"
        bitfun_mirror_priv cp "$rewritten" /etc/apt/sources.list
        rm -f "$rewritten"
      fi
      echo ">>> apt mirror: rewrote common upstream hosts → ${mirror}"
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

bitfun_mirror_apply_apt() {
  if ! command -v apt-get >/dev/null 2>&1; then
    return 0
  fi
  if [ ! -f /etc/os-release ]; then
    return 0
  fi
  bitfun_mirror_apply_apt_debian_family || echo ">>> apt mirror: apply failed (continuing)" >&2
}

bitfun_mirror_write_docker_daemon_json() {
  local mirrors_csv="$1"
  local tmp py
  tmp="$(mktemp)"
  py="$(mktemp)"
  cat >"$py" <<'PY'
import json, os, sys
path = "/etc/docker/daemon.json"
mirrors = [m for m in sys.argv[1].split() if m]
data = {}
if os.path.exists(path):
    try:
        with open(path, "r", encoding="utf-8") as f:
            raw = f.read().strip()
        if raw:
            data = json.loads(raw)
            if not isinstance(data, dict):
                data = {}
    except Exception:
        data = {}
existing = data.get("registry-mirrors") or []
if not isinstance(existing, list):
    existing = []
merged = []
for item in list(existing) + mirrors:
    if item and item not in merged:
        merged.append(item)
data["registry-mirrors"] = merged
data["bitfun-cn-mirror"] = True
out = sys.argv[2]
with open(out, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
PY
  if command -v python3 >/dev/null 2>&1; then
    if [ -f /etc/docker/daemon.json ]; then
      bitfun_mirror_backup_file /etc/docker/daemon.json
    fi
    bitfun_mirror_priv mkdir -p /etc/docker
    if python3 "$py" "$mirrors_csv" "$tmp"; then
      bitfun_mirror_priv cp "$tmp" /etc/docker/daemon.json
      echo ">>> docker mirror: merged registry-mirrors into /etc/docker/daemon.json"
    else
      echo ">>> docker mirror: python merge failed (continuing)" >&2
    fi
  else
    if [ -f /etc/docker/daemon.json ]; then
      echo ">>> docker mirror: python3 missing; leaving existing daemon.json untouched" >&2
    else
      bitfun_mirror_priv mkdir -p /etc/docker
      {
        echo '{'
        echo '  "registry-mirrors": ['
        local first=1 m
        for m in $mirrors_csv; do
          if [ "$first" -eq 1 ]; then first=0; else echo ','; fi
          printf '    "%s"' "$m"
        done
        echo ''
        echo '  ],'
        echo '  "bitfun-cn-mirror": true'
        echo '}'
      } >"$tmp"
      bitfun_mirror_priv cp "$tmp" /etc/docker/daemon.json
      echo ">>> docker mirror: wrote /etc/docker/daemon.json"
    fi
  fi
  rm -f "$tmp" "$py"
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
  bitfun_mirror_write_docker_daemon_json "$mirrors" || echo ">>> docker mirror: apply failed (continuing)" >&2
  bitfun_mirror_restart_docker_if_needed || true
}

bitfun_mirror_apply_cargo_config() {
  local cargo_home="${CARGO_HOME:-$HOME/.cargo}"
  local cfg="${cargo_home}/config.toml"
  local sparse="${BITFUN_CARGO_SPARSE_URL:-sparse+https://rsproxy.cn/index/}"
  mkdir -p "$cargo_home" 2>/dev/null || true
  if [ -f "$cfg" ]; then
    mkdir -p "$HOME/.bitfun/mirror-backup" 2>/dev/null || true
    cp -a "$cfg" "$HOME/.bitfun/mirror-backup/cargo-config.toml.$(date +%Y%m%d%H%M%S)" 2>/dev/null || true
  fi
  # Replace BitFun-managed block; keep other user content when possible.
  local tmp
  tmp="$(mktemp)"
  if [ -f "$cfg" ]; then
    # shellcheck disable=SC2016
    awk '
      BEGIN {skip=0}
      /^# >>> BITFUN-CN-MIRROR$/ {skip=1; next}
      /^# <<< BITFUN-CN-MIRROR$/ {skip=0; next}
      skip==0 {print}
    ' "$cfg" >"$tmp"
  else
    : >"$tmp"
  fi
  cat >>"$tmp" <<EOF

# >>> BITFUN-CN-MIRROR
[source.crates-io]
replace-with = "bitfun-rsproxy-sparse"

[source.bitfun-rsproxy-sparse]
registry = "${sparse}"

[registries.bitfun-rsproxy-sparse]
index = "${sparse}"

[net]
git-fetch-with-cli = true
# <<< BITFUN-CN-MIRROR
EOF
  mv "$tmp" "$cfg"
  echo ">>> cargo mirror: configured ${cfg} → ${sparse}"
}

bitfun_mirror_apply_host() {
  if [ "${BITFUN_MIRROR_SKIP_APPLY:-0}" = "1" ]; then
    echo ">>> mirror apply skipped (BITFUN_MIRROR_SKIP_APPLY=1)"
    return 0
  fi
  if [ "${BITFUN_MIRROR_MODE:-global}" != "cn" ]; then
    return 0
  fi
  echo ">>> Applying China host mirrors (apt / docker / cargo)..."
  bitfun_mirror_apply_apt || true
  bitfun_mirror_apply_docker_daemon || true
  bitfun_mirror_apply_cargo_config || true
  mkdir -p "$HOME/.bitfun" 2>/dev/null || true
  echo "cn" >"$HOME/.bitfun/mirror-mode" 2>/dev/null || true
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
    bitfun_mirror_priv apt-get update -y
    bitfun_mirror_priv apt-get install -y ca-certificates curl
    bitfun_mirror_priv install -m 0755 -d /etc/apt/keyrings
    curl -fsSL --retry 3 "https://mirrors.aliyun.com/docker-ce/linux/${docker_ce_distro}/gpg" \
      | bitfun_mirror_priv tee /etc/apt/keyrings/docker.asc >/dev/null
    bitfun_mirror_priv chmod a+r /etc/apt/keyrings/docker.asc
    echo "deb [arch=${arch} signed-by=/etc/apt/keyrings/docker.asc] https://mirrors.aliyun.com/docker-ce/linux/${docker_ce_distro} ${version_codename} stable" \
      | bitfun_mirror_priv tee /etc/apt/sources.list.d/docker.list >/dev/null
    bitfun_mirror_priv apt-get update -y
    bitfun_mirror_priv apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
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
    bitfun_mirror_priv "$pkg" install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
    return 0
  fi
  return 1
}

# Download get.docker.com script to $1 using CN-aware URL.
bitfun_mirror_fetch_docker_install_script() {
  local dest="$1"
  local url="${BITFUN_DOCKER_GET_URL:-https://get.docker.com}"
  echo ">>> Fetching Docker install script: ${url}"
  curl -fsSL --retry 3 "$url" -o "$dest"
}

bitfun_mirror_init() {
  bitfun_mirror_parse_args "$@"
  bitfun_mirror_resolve_mode
  bitfun_mirror_export_urls
  echo ">>> Mirror mode: ${BITFUN_MIRROR_MODE} (BITFUN_USE_CN_MIRROR=${BITFUN_USE_CN_MIRROR})"
  if [ "${BITFUN_MIRROR_MODE}" = "cn" ]; then
    echo ">>> GitHub git URL:     ${BITFUN_GITHUB_GIT_URL}"
    echo ">>> GitHub tarball URL: ${BITFUN_GITHUB_TARBALL_URL}"
    echo ">>> Docker get URL:     ${BITFUN_DOCKER_GET_URL}"
    echo ">>> apt mirror:         ${BITFUN_APT_MIRROR}"
    echo ">>> cargo sparse:       ${BITFUN_CARGO_SPARSE_URL}"
    echo ">>> docker registries:  ${BITFUN_DOCKER_REGISTRY_MIRRORS}"
    bitfun_mirror_apply_host
  else
    mkdir -p "$HOME/.bitfun" 2>/dev/null || true
    echo "global" >"$HOME/.bitfun/mirror-mode" 2>/dev/null || true
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

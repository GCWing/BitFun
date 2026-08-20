# Relay Deploy: China Mirror Acceleration

**Date:** 2026-07-24
**Status:** Approved for implementation (user: design + implement end-to-end)

## Problem

One-click / `deploy.sh` relay deployment pulls Docker Hub images, Debian apt
packages, crates.io crates, GitHub source, and `get.docker.com`. On mainland
China hosts these endpoints are slow or unreliable, so deploy often stalls.

## Goals

1. Auto-detect mainland China at deploy start; allow force override.
2. Cover the full path: Desktop SSH (Docker install, GitHub sync) + `deploy.sh`
   + Dockerfile (apt + cargo) + Docker Hub pulls.
3. Persist host-level apt and Docker mirror config; keep Cargo mirroring scoped
   to the relay Docker build so deployment never rewrites the SSH user's Cargo
   configuration.
4. Ship built-in default CN mirrors; allow env overrides.
5. Keep non-CN hosts unchanged.

## Non-goals

- Changing relay runtime / account / port behavior
- Building a private mirror service
- Guaranteeing third-party public mirror uptime (defaults + overrides only)

## Detection

Priority:

1. `BITFUN_MIRROR=cn|global` or flags `--cn-mirror` / `--global-mirror`
2. Auto (`BITFUN_MIRROR=auto` default):
   - Public IP country lookup (short timeout)
   - Timezone `Asia/Shanghai` / `Asia/Chongqing` / `Asia/Urumqi`
   - Connectivity heuristic: GitHub slow/fail + Aliyun mirror reachable → CN
3. On ambiguity → `global` (safe default)

## Default CN mirrors (overridable)

| Surface | Default | Override env |
|---|---|---|
| Docker Hub registry-mirrors | `https://docker.1ms.run`, `https://dockerproxy.net`, `https://docker.m.daocloud.io` | `BITFUN_DOCKER_REGISTRY_MIRRORS` (space/comma separated) |
| Debian/Ubuntu apt | `mirrors.aliyun.com` | `BITFUN_APT_MIRROR` |
| RHEL/CentOS yum/dnf docker-ce | Aliyun docker-ce | same family |
| Relay Docker build Cargo / crates.io | `sparse+https://rsproxy.cn/index/` | `BITFUN_CARGO_SPARSE_URL` |
| Rustup (host, if used) | `https://rsproxy.cn` | `BITFUN_RUSTUP_DIST_SERVER` |
| GitHub git / tarball | `https://ghfast.top/` prefix | `BITFUN_GITHUB_PROXY` |
| Docker Engine install | Aliyun docker-ce packages; fallback proxied `get.docker.com` | `BITFUN_DOCKER_INSTALL_URL` |

## Architecture

Canonical script: `src/apps/relay-server/mirror.sh`

- Sourced by `deploy.sh`
- Embedded into Desktop orchestration via `include_str!` from
  `relay_deploy.rs` (single source of truth; no Cargo crate dependency on apps)
- Idempotent apply with backups under `/etc/bitfun/mirror-backup-*` and
  `$HOME/.bitfun/mirror-backup-*`

Flow:

```
detect mode → if cn: apply host mirrors → export build/env vars
            → Docker install / GitHub sync use CN URLs
            → deploy.sh passes BITFUN_USE_CN_MIRROR=1 build-args
            → Dockerfile rewrites apt + writes build-local cargo config
```

## Persistence / merge rules

- **apt:** backup `sources.list` (+ `sources.list.d` bitfun file); write BitFun-owned
  `sources.list.d/bitfun-cn-mirror.list` when possible; otherwise rewrite
  `deb.debian.org` / `archive.ubuntu.com` hosts in place.
- **Docker daemon.json:** JSON-merge `registry-mirrors` (python3 when available);
  never drop unrelated keys; `systemctl restart docker` only if daemon was
  already manageable.
- **Cargo:** leave `$HOME/.cargo/config.toml` untouched. The builder writes
  `/usr/local/cargo/config.toml` inside Docker for rsproxy sparse.
- Marker file: `$HOME/.bitfun/mirror-mode` = `cn|global` for logs/idempotency.
- **Rollback:** `BITFUN_MIRROR=global` removes the BitFun apt list, restores
  source files renamed with `.bitfun-disabled`, removes only Docker mirrors
  recorded as BitFun additions, and cleans the legacy managed Cargo block from
  early deployments.

## Failure behavior

- Mirror apply failures log warnings and continue with best effort; do not abort
  deploy solely because a public mirror endpoint is down.
- GitHub proxy failure keeps existing tarball fallback chain (try CN URL then
  upstream if override allows).
- `BITFUN_MIRROR=global` skips CN writes and rolls back BitFun-owned host
  mirror changes even on a China IP.

## Verification

- `bash -n` on `mirror.sh` / `deploy.sh`
- `cargo test -p bitfun-services-integrations --features remote-ssh` (include
  embedded mirror script + existing deploy tests)
- `cargo check -p bitfun-desktop` when orchestration wiring changes

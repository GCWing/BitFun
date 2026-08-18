# Common commands

> Companion to the root `AGENTS.md` entry. This is a **command dictionary**, not
> the “what to run after a change” selector. After edits, use
> [`verification.md`](verification.md).
>
> [中文](common-commands.zh-CN.md)

```bash
# Install
pnpm install

# Dev
pnpm run desktop:dev               # full hot-reload: Vite HMR + Rust auto-rebuild & restart
pnpm run desktop:preview:debug     # reuse pre-built binary + Vite HMR; no Rust auto-rebuild
pnpm run dev:web                   # browser-only frontend
pnpm run cli:dev                   # CLI runtime
pnpm run cli:install               # build release + install bitfun (Windows/macOS/Linux; deprecated bitfun-cli included)

# Check
pnpm run fmt:rs                     # format only changed / staged Rust files
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/mobile-web run type-check
pnpm run i18n:generate
pnpm run i18n:contract:test          # i18n contract / resources only
pnpm run i18n:audit                  # i18n contract / resources only
pnpm run theme:color-audit:all
pnpm run product:check               # default product definition
pnpm run check:repo-hygiene
pnpm run check:github-config
pnpm run check:core-boundaries
pnpm run check:build-prereqs
cargo check --workspace

# Test (prefer focused paths locally; broad suites are CI-backed)
pnpm run product:test
pnpm --dir src/web-ui run test:run
cargo test --workspace

# Build (build-impacting changes or CI reproduction)
cargo build -p bitfun-desktop
pnpm run build:web
pnpm run build:mobile-web

# Fast builds (manual build/debug)
pnpm run desktop:build:fast
pnpm run desktop:build:release-fast
pnpm run desktop:build:nsis:fast
```

### Build escape hatches

| Variable / flag | Use when |
| --- | --- |
| `CARGO_PROFILE_DEV_DEBUG=2` | Need full debug info for breakpoints. Dev profile ships `line-tables-only`. |
| `BITFUN_MOBILE_WEB_FORCE_BUILD=1` or `node scripts/mobile-web-build.cjs --force` | Force mobile-web rebuild even when `src/mobile-web/dist` looks up to date. |
| `VITE_USE_POLLING=1` | Vite watcher misses changes (network drive / WSL mount). |

`pnpm run build:web` runs type-check and Vite build concurrently; errors are prefixed `[type-check]` / `[vite-build]`.

This repository does not currently ship `docs:links:check` / `docs:architecture:check`.
For documentation changes, use `git diff --check` plus manual link review against
[`docs/README.md`](../README.md) and [`docs-governance.md`](docs-governance.md).

Full script list: [`package.json`](../../package.json).

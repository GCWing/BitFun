# Verification matrix

> Companion to the root `AGENTS.md` entry (STD-08). This table is the
> **authoritative “what to run after this change”** selector. For the command
> dictionary, see [`common-commands.md`](common-commands.md).
>
> [中文](verification.zh-CN.md)

## Principles (from repository AGENTS)

Choose verification at the owner, not from a repository-wide test matrix:

1. Read the nearest local `AGENTS.md` and run its narrowest command that covers
   the changed behavior.
2. Prefer one package, one test target or module filter, and the minimum feature
   set. Do not use `product-full`, `all-features`, or a workspace-wide suite as a
   shortcut.
3. Run a repository check only when its contract changed: repository hygiene for
   layout/content rules, GitHub config for workflow changes, and core boundaries
   for Cargo features, dependency direction, or test-target layout.
4. Leave broad builds, workspace suites, packaging, and platform matrices to
   existing CI unless the change affects those paths or reproduces a CI failure.

If a module lacks a useful focused command, add it to that module's guide rather
than expanding root `AGENTS.md`. Do not pre-emptively align every module's test
list; document a command only when a real workflow needs it.

## Matrix

Run the smallest local precheck that matches the touched files. CI is expected to
cover full builds and broad test suites; run heavier local commands only when the
change directly affects build, packaging, or CI cannot protect the path.

| Change type | Minimum verification |
|---|---|
| Frontend UI, state, or adapters without i18n resource/contract changes | `pnpm run type-check:web`, plus nearest focused test when behavior changed |
| Locale resource-only changes | `pnpm run i18n:audit` |
| Locale contract or shared terms | `pnpm run i18n:generate && pnpm run i18n:contract:test && pnpm run i18n:audit` |
| Web UI i18n runtime, namespace loading, or direct `i18nService.t(...)` usage | `pnpm run i18n:contract:test && pnpm run type-check:web && pnpm --dir src/web-ui run test:run src/infrastructure/i18n/core/I18nService.test.ts` |
| Theme, CSS variable, widget payload, mobile, installer, or CLI/TUI color changes | `pnpm run theme:color-audit:all` |
| Mobile web UI, state, pairing, disconnect, or reconnect behavior | `pnpm --dir src/mobile-web run type-check`; include manual pairing/reconnect notes when behavior changed |
| Product definition, schema, resolver, or Desktop/CLI product build adapter | `pnpm run product:test`, plus `pnpm run product:check` for the default definition |
| Shared Rust logic in `core`, `transport`, adapters, or services | Nearest module `AGENTS.md`; otherwise `cargo check -p <owning-package>` with the minimum feature set, plus the nearest focused `cargo test` when behavior changed |
| Desktop integration, Tauri APIs, browser/computer-use, or desktop-only behavior | `cargo check -p bitfun-desktop`, plus focused desktop tests when behavior changed |
| Behavior covered by desktop smoke/functional flows | Nearest focused E2E/smoke check; rely on CI for broad build/test unless build behavior changed |
| `src/crates/adapters/ai-adapters` | Relevant Rust checks above; add `cargo test -p bitfun-agent-stream` only when stream contracts changed |
| Installer frontend or i18n runtime without packaging changes | `pnpm --dir BitFun-Installer run type-check` |
| Installer Tauri/Rust changes | `cargo check --manifest-path BitFun-Installer/src-tauri/Cargo.toml` |
| Installer packaging, payload, install/uninstall flow, or native bundling | `pnpm run installer:build` |
| Build scripts or prerequisite changes | `pnpm run check:build-prereqs`, plus `node --test scripts/check-build-prereqs.test.mjs` when the check logic changed |
| Cargo features, dependency direction, or test-target layout | `pnpm run check:core-boundaries` |
| Documentation structure, indexes, local links, anchors, or naming | `git diff --check`, plus manual review against [`docs/README.md`](../README.md) and [`docs-governance.md`](docs-governance.md) (this repo does not ship `docs:links:check` / `docs:architecture:check` yet) |

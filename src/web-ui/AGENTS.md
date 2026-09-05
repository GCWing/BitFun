[中文](AGENTS-CN.md) | **English**

# AGENTS.md

## Scope

This file applies to `src/web-ui`. Use the top-level `AGENTS.md` for repository-wide rules.

## What matters here

`src/web-ui` is the shared frontend for:

- Tauri desktop
- server/web via WebSocket / Fetch adapters

Most changes start in:

- `src/infrastructure/`: adapters, i18n, theme, providers, config
- `src/infrastructure/peer-device/`: Peer Device Mode transport switch + host-invoke bridge
- `src/app/`: shell layout and top-level composition
- `src/flow_chat/`: chat flow UI and state
- `src/tools/`: editor, terminal, git, workspace, file explorer
- `src/shared/`: shared services, stores, helpers, types
- `src/locales/`: localized strings

Peer Device Mode (same-account remote full client) is documented in
`docs/architecture/peer-device-mode.md`. Frontend invariants:
`src/infrastructure/peer-device/README.md`. Do not reintroduce nested
sessions/chat shells; enter peer mode from the device list (Remote Connect →
My OpenBitFun) instead.

One-click relay deploy wizard: `src/features/relay-deploy/` (see its README).
The Remote Connect account group (My OpenBitFun) login form and the Remote Connect
Self-Hosted entries must open `RelayDeployWizard`, not an external README.

## Local rules

- Do not call Tauri APIs directly from UI components; go through the adapter / infrastructure layer
- Reuse `@openbitfun/ui`, design tokens, theme, i18n, and Zustand stores before adding new frontend primitives
- Theme and color-token changes must follow
  `docs/architecture/theme-token-optimization.md`: failing audits should be
  fixed by reusing tokens, merging redundant values, or adding a scoped owner
  contract. Do not raise baseline or test expectation counts just to make a
  theme audit pass. Use `pnpm run theme:color-audit:all` for changes that touch
  theme tokens, CSS variables, color literals, widget payloads, mobile,
  installer, or CLI/TUI color projection.
- Keep locale metadata in the generated i18n contract files. Edit
  `src/shared/i18n/contract/locales.json`, run `pnpm run i18n:generate`, and
  keep Web UI strings under `src/web-ui/src/locales`.
- Use `useI18n(namespace)` for route or feature copy so non-bootstrap
  namespaces stay lazy. Direct `i18nService.t(...)` calls require bootstrap
  namespace coverage.
- Follow `src/web-ui/LOGGING.md`: English only, no emojis, structured logs

## Commands

Keep development/build entry points here. Verification commands are maintained
only in the section below.

```bash
pnpm --dir src/web-ui dev
pnpm run build:web                     # build-impacting changes / CI reproduction
```

`pnpm run build:web` runs type-check and Vite concurrently; either error may
appear first and their output uses `[type-check]` / `[vite-build]` prefixes.
Set `VITE_USE_POLLING=1` only when native file events miss changes, typically on
a network drive or WSL mount.

## Verification

Choose the smallest matching check:

```bash
pnpm run i18n:audit
pnpm run i18n:generate && pnpm run i18n:contract:test && pnpm run i18n:audit
pnpm run type-check:web && pnpm --dir src/web-ui run test:run src/infrastructure/i18n/core/I18nService.test.ts
pnpm run motion:audit
pnpm run check:web
```

Use the first line for resource-only locale changes, the second for
contract/shared-term changes, the third for i18n runtime/namespace-loading
changes, the fourth for presentation or interaction-motion changes, and the
fifth for ordinary Web UI code. `check:web` runs type-check plus the same
Appearance contract, theme color, and theme visual governance gates used by CI,
so rendered DOM or styling regressions are caught locally. The motion audit is
an intent-review inventory, not a pass/fail gate; do not mechanically replace
deliberate layout transitions or animate virtualized content. Rely on CI for
full lint, build, and broad test coverage unless the local change specifically
needs it.

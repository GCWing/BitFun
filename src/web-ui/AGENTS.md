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
- `src/app/`: shell layout and top-level composition
- `src/flow_chat/`: chat flow UI and state
- `src/tools/`: editor, terminal, git, workspace, file explorer
- `src/shared/`: shared services, stores, helpers, types
- `src/locales/`: localized strings

## Local rules

- Do not call Tauri APIs directly from UI components; go through the adapter / infrastructure layer
- Reuse existing theme, i18n, component-library, and Zustand stores before adding new frontend primitives
- Keep locale metadata in the generated i18n contract files. Edit
  `src/shared/i18n/contract/locales.json`, run `pnpm run i18n:generate`, and
  keep Web UI strings under `src/web-ui/src/locales`.
- Use `useI18n(namespace)` for route or feature copy so non-bootstrap
  namespaces stay lazy. Direct `i18nService.t(...)` calls require bootstrap
  namespace coverage.
- Follow `src/web-ui/LOGGING.md`: English only, no emojis, structured logs

## Commands

These are command references, not the default precheck list. Use Verification
below for PR scope.

```bash
pnpm --dir src/web-ui dev
pnpm --dir src/web-ui run lint
pnpm --dir src/web-ui run type-check
pnpm --dir src/web-ui run test:run     # broad suite; prefer focused paths locally
pnpm run i18n:contract:test
pnpm run i18n:audit
pnpm run build:web                     # build-impacting changes / CI reproduction
```

## Verification

Choose the smallest matching check:

```bash
pnpm run i18n:audit
pnpm run i18n:generate && pnpm run i18n:contract:test && pnpm run i18n:audit
pnpm run type-check:web && pnpm --dir src/web-ui run test:run src/infrastructure/i18n/core/I18nService.test.ts
pnpm run type-check:web
```

Use the first line for resource-only locale changes, the second for
contract/shared-term changes, the third for i18n runtime/namespace-loading
changes, and the fourth for ordinary Web UI code. Rely on CI for full lint,
build, and broad test coverage unless the local change specifically needs it.

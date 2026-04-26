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
- Follow `src/web-ui/LOGGING.md`: English only, no emojis, structured logs
- For quick manual desktop verification of shared frontend changes, prefer `pnpm run desktop:preview:debug` over `pnpm run desktop:dev`; switch back to the full desktop flow only when the Tauri startup/dev pipeline itself is part of what you are validating

## Commands

```bash
pnpm --dir src/web-ui dev
pnpm run desktop:preview:debug
pnpm --dir src/web-ui run lint
pnpm --dir src/web-ui run type-check
pnpm --dir src/web-ui run test:run
pnpm run build:web
```

## Verification

```bash
pnpm run lint:web && pnpm run type-check:web && pnpm --dir src/web-ui run test:run
```

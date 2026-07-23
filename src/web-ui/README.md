# BitFun Web UI

[中文](./README.zh-CN.md) | English

## Overview

This directory contains BitFun’s **Web UI** (React + TypeScript). The same frontend codebase is reused by:

- **Desktop**: loaded via **Tauri**
- **Server/Web**: built into static assets and served by the backend

## Tech stack

- React 18.3
- TypeScript 5.8
- Vite 7
- SCSS
- Zustand (state management)
- Monaco Editor

## Directory structure

```
src/web-ui/
├── README.md                     # This file
├── README.zh-CN.md               # Chinese version
├── LOGGING.md                    # Logging & debugging notes
├── index.html                    # Entry HTML
├── preview.html                  # Preview page (optional)
├── package.json                  # Dependencies & scripts
├── package-lock.json             # Locked dependency versions
├── public/                       # Static assets
├── src/                          # Frontend source
│   ├── app/                      # Main app UI
│   ├── component-library/        # Component library
│   ├── features/                 # Feature modules
│   ├── flow_chat/                # Flow / chat UI
│   ├── generated/                # Generated content (placeholder/artifacts)
│   ├── hooks/                    # Shared hooks
│   ├── infrastructure/           # Infra (API/i18n/theme/etc.)
│   ├── locales/                  # Translations
│   ├── shared/                   # Shared utils & types
│   ├── tools/                    # Tool UIs (editor/terminal/git/etc.)
│   ├── main.tsx                  # App entry
│   └── vite-env.d.ts             # Vite type declarations
├── tsconfig.json                 # TS config
├── tsconfig.node.json            # Node/Vite TS config
├── vite.config.ts                # Vite config
├── vite.config.preview.ts        # Preview build config
└── vite.config.version-plugin.ts # Version plugin
```

## Frontend communication layer

### Core idea

One UI, two runtimes:

- **Desktop**: Tauri API (`invoke`, `listen`)
- **Server/Web**: WebSocket / Fetch API

### Adapter pattern (conceptual example)

```ts
const adapter = IS_TAURI ? TauriAdapter : WebSocketAdapter;

await adapter.request("execute_agent_task", params);
adapter.listen("agentic://text-chunk", callback);
```

## Development

### Start the dev server

```bash
# Desktop
pnpm --dir src/web-ui run dev

# Server/Web
VITE_BUILD_TARGET=web pnpm --dir src/web-ui run dev
```

### Build

```bash
# Desktop
pnpm --dir src/web-ui run build

# Server/Web
VITE_BUILD_TARGET=web pnpm --dir src/web-ui run build
# output: dist/
```

## Related docs (within this package)

- [Logging guide](LOGGING.md)
- [Component library README](src/component-library/README.md)
- [i18n README](src/infrastructure/i18n/README.md)

## Notes

1. **Don’t call Tauri APIs directly** in UI components; use the adapter layer.
2. **Keep Web compatibility** in mind (some capabilities may not exist in browsers).
3. **Prefer CSS variables** over hard-coded colors/sizes.

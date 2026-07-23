# LSP (Language Server Protocol)

BitFun’s plugin-based LSP implementation: completions, hover, definition, references, formatting, diagnostics, and related editor features.

## Scope

This frontend directory is responsible for:

- Monaco-facing integration hooks and services
- plugin management UI
- shared frontend types and exports

Backend lifecycle management, plugin loading, and JSON-RPC transport live outside this directory.

## Architecture (high level)

- **Backend (Rust)**: owns server lifecycle, plugin loading, JSON-RPC transport, and workspace-scoped state.
- **Frontend (TypeScript/React)**: integrates LSP features into Monaco and exposes a small API surface.

Frontend layout:

```text
src/tools/lsp/
├── services/            # backend calls + Monaco integration
├── hooks/               # React-facing hooks
├── components/          # UI (plugin list, references panel)
├── types/               # shared type definitions
└── index.ts             # exports + initialize helper
```

## Usage

### Enable Monaco LSP integration

```ts
import { useMonacoLsp } from '@/tools/lsp';

useMonacoLsp(editor, languageId, filePath, true, workspacePath);
```

Notes:

- `workspacePath` is required to enable non-builtin Monaco languages.
- Builtin Monaco languages such as `typescript`, `javascript`, `typescriptreact`, and `javascriptreact` use a different integration path.

### Plugin management (UI or hook)

```ts
import { useLspPlugins } from '@/tools/lsp';

const { plugins, loading, error, installPlugin, uninstallPlugin, reload } = useLspPlugins();
```

Or use the ready-made component:

```tsx
import { LspPluginList } from '@/tools/lsp';

<LspPluginList />
```

## Plugin package format

Plugin packages are `.vcpkg` files (ZIP under the hood):

```text
my-language-lsp-1.0.0.vcpkg
├── manifest.json
├── bin/
│   ├── win-x64/...
│   ├── darwin-x64/...
│   └── linux-x64/...
└── config/ (optional)
```

`manifest.json` example:

```json
{
  "id": "typescript-lsp",
  "name": "TypeScript Language Server",
  "version": "1.0.0",
  "author": "Microsoft",
  "description": "TypeScript and JavaScript language support",
  "server": {
    "command": "bin/${platform}-${arch}/typescript-language-server",
    "args": ["--stdio"],
    "env": {}
  },
  "languages": ["typescript", "javascript"],
  "file_extensions": [".ts", ".tsx", ".js", ".jsx", ".mjs"],
  "capabilities": {
    "completion": true,
    "hover": true,
    "definition": true,
    "references": true,
    "rename": true,
    "formatting": true,
    "diagnostics": true
  },
  "min_bitfun_version": "1.0.0"
}
```

## Debugging

- `initializeLsp()` sets up the extension registry and workspace initializer.
- `window.LspDiag` is installed as a lightweight debugging helper (see [index.ts](index.ts)).

## Constraints and Notes

- Keep package format and manifest fields aligned with the backend plugin loader.
- When adding new frontend LSP capabilities, verify whether the backend protocol already supports them.
- Keep this README focused on the frontend surface; backend implementation details should be documented closer to the Rust side.

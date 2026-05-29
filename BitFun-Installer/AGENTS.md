[中文](AGENTS-CN.md) | **English**

# AGENTS.md

## Scope

This file applies to `BitFun-Installer`. Use the top-level `AGENTS.md` for repository-wide rules.

## What matters here

`BitFun-Installer` is a separate Tauri + React app, not part of the main Cargo workspace.

Important areas called out by the module README:

- `src-tauri/src/installer/commands.rs`: Tauri IPC and uninstall execution
- `src-tauri/src/installer/registry.rs`: Windows registry integration
- `src-tauri/src/installer/shortcut.rs`: shortcut creation
- `src-tauri/src/installer/extract.rs`: archive extraction
- `src/hooks/useInstaller.ts`: frontend installer state flow
- `src/i18n/`: installer-only strings; locale metadata is generated from
  `src/shared/i18n/contract/locales.json`

Install flow:

```text
Language Select → Options → Progress → Model Setup → Theme Setup
```

## Commands

```bash
pnpm --dir BitFun-Installer run installer:dev
pnpm --dir BitFun-Installer run tauri:dev
pnpm --dir BitFun-Installer run type-check
pnpm --dir BitFun-Installer run build
pnpm --dir BitFun-Installer run installer:build
```

## Verification

For frontend, i18n, language-contract, or non-packaging installer changes, prefer
the lighter focused checks:

```bash
pnpm --dir BitFun-Installer run type-check
cargo check --manifest-path BitFun-Installer/src-tauri/Cargo.toml
pnpm --dir BitFun-Installer run build
```

Run the full installer build only for packaging, payload, native bundling,
install/uninstall flow, registry, shortcut, or extraction changes:

```bash
pnpm --dir BitFun-Installer run type-check && pnpm --dir BitFun-Installer run installer:build
```

If you modify uninstall flow, also validate the uninstall mode entry points described in `BitFun-Installer/README.md`.

# Managed Runtime Delivery Plan (No-Dev Environment)

## Scope

This plan ensures Cowork mode can execute built-in Skills and local MCP servers on user machines without preinstalled development tools.

Constraints confirmed:
- No remote execution fallback.
- First-run large runtime/component download is acceptable.
- Dual-platform support in parallel (macOS + Windows).
- Dual package strategy is accepted.

## Packaging Strategy

### Package A: `BitFun-Lite`
- Smaller installer.
- Includes runtime bootstrapper only.
- On first use, downloads required managed components (Node/Python/Office/Poppler/Pandoc) by demand.

### Package B: `BitFun-Full`
- Larger installer.
- Bundles core managed runtime components in installer payload.
- Works offline for common Skill/MCP scenarios immediately after install.

## Runtime Layout

Managed runtime root:
- `~/.config/bitfun/runtimes/` (via PathManager)

Component layout:
- `runtimes/<component>/current/...`
- Optional versioned dirs for future upgrades:
  - `runtimes/<component>/<version>/...`
  - `current` symlink or pointer switch.

## Runtime Resolution Policy

Command resolution order:
1. Explicit command path (if command is absolute/relative path)
2. System PATH
3. BitFun managed runtimes

This policy is implemented in `RuntimeManager` and currently used by:
- Local MCP process launch.
- Terminal PATH injection (so Bash/Skill commands can find managed binaries).

## UX and Observability

- MCP config UI shows local command readiness and runtime source:
  - `system`
  - `managed`
  - `missing`
- Runtime capability API is exposed for diagnostics/settings UI.
- Start failure message explicitly reports managed runtime root path for troubleshooting.

## Security and Integrity

Downloader requirements (next phase):
- HTTPS only.
- SHA256 verification against signed manifest.
- Optional signature verification for manifest and artifacts.
- Atomic install (download -> verify -> extract -> switch `current`).
- Rollback to previous version if install fails.

## Next Implementation Milestones

1. Runtime installer service
- Add component manifest model.
- Add download/verify/extract pipeline.
- Add install state tracking and progress events.

2. Preflight dependency analyzer
- Parse built-in Skill runtime requirements.
- Parse local MCP commands and map to required components.
- Produce missing-component list for one-click install.

3. UI install workflow
- Add "Install required runtimes" action in Skills/MCP settings.
- Progress + retry + failure reason details.

4. Build pipeline for dual packages
- `Lite`: bootstrap only.
- `Full`: include runtime payload in bundle resources.
- Platform-specific artifact matrix for macOS and Windows.

## Acceptance Criteria

- On clean machine without Node/Python/Office installed:
  - Built-in Skills requiring these runtimes can run after managed install.
  - Local MCP servers using `npx/node/python` can start without system-level runtime.
- No cloud fallback is required for runtime execution.
- Both macOS and Windows pass same E2E runtime readiness scenarios.

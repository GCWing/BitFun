# Contributing

[中文版](./CONTRIBUTING_CN.md)

Thanks for your interest in BitFun! BitFun is a multi-platform AI programming environment powered by Rust and TypeScript, with shared core logic across Desktop/CLI/Server. This guide explains how to contribute effectively.

## Code of Conduct

Be respectful, kind, and constructive. We welcome contributors of all backgrounds and experience levels.

## Quick Start

### Prerequisites

- Node.js (LTS recommended)
- pnpm
- Rust toolchain (install via [rustup](https://rustup.rs/))
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for desktop development

#### Windows: OpenSSL Setup

The desktop app includes SSH remote support, which pulls in OpenSSL. On Windows the workspace **does not use vendored OpenSSL**; link against **pre-built** binaries (no Perl/NASM/OpenSSL source build).

- **Default**: `pnpm run desktop:dev` calls `ensure-openssl-windows.mjs` on Windows. `pnpm run desktop:preview:debug` does the same whenever it needs to fast-rebuild `bitfun-desktop` before preview. Every `desktop:build*` script runs via `scripts/desktop-tauri-build.mjs`, which does the same before `tauri build` (first run downloads FireDaemon OpenSSL 3.5.5 into `.bitfun/cache/`; later runs reuse the cache). Extra args: `pnpm run desktop:build -- <tauri-build-args>`.
- **Manual / CI**: Download the [FireDaemon OpenSSL 3.5.5 LTS ZIP](https://download.firedaemon.com/FireDaemon-OpenSSL/openssl-3.5.5.zip), extract, set `OPENSSL_DIR` to the `x64` folder, `OPENSSL_STATIC=1`, or run `scripts/ci/setup-openssl-windows.ps1`.
- **Opt out of auto-download**: `BITFUN_SKIP_OPENSSL_BOOTSTRAP=1` and configure `OPENSSL_DIR` yourself.
- **`desktop:dev:raw`** skips the dev script (no OpenSSL bootstrap); set `OPENSSL_DIR` yourself, run `scripts/ci/setup-openssl-windows.ps1`, or `node scripts/ensure-openssl-windows.mjs` (warms `.bitfun/cache/` and prints PowerShell `OPENSSL_*` lines to paste).

### Install dependencies

```bash
pnpm install
```

### Common commands

```bash
# Desktop
pnpm run desktop:dev
pnpm run desktop:preview:debug
pnpm run desktop:build

# E2E
pnpm run e2e:test
```

For local iteration:

- `desktop:preview:debug` avoids `tauri dev`, reuses the existing debug desktop binary when it is still current, and automatically does a fast rebuild with reduced debug info before preview when Rust / Tauri inputs are newer or the binary is missing.
- Add `-- --force-rebuild` only when you explicitly want to rebuild before preview even if the timestamp check says the binary is current.
- Keep `desktop:dev` for the full Tauri watcher/startup flow, and keep the project verification commands aligned with the actual files you changed.

For packaging and release work:

- Confirm the intended output format if the request only says "package" or "release" without naming the artifact type.
- For Windows end-user installer delivery, prefer `pnpm run desktop:build:nsis`.
- For a standalone Windows executable, use `pnpm run desktop:build:exe` only when that is the explicit ask.
- Do not confuse local fast/debug artifacts with real release deliverables.

> Note: More granular scripts are available (e.g. `dev:web`, `cli:dev`, `website:dev`). See `package.json` for details.

## Code Standards and Architecture Constraints

### Logging

- English only, avoid verbose logs
- Frontend: `createLogger('ModuleName')`
- Backend: `log::{info, debug, warn, error}` macros

### Platform-agnostic core

Do not use platform-specific dependencies in `core`:

- ❌ `tauri::AppHandle`
- ✅ `bitfun_events::EventEmitter`

### Tauri command conventions

- Command names use `snake_case`
- Keep Rust and TypeScript naming aligned
- Always use structured request format:

```rust
#[tauri::command]
pub async fn your_command(
  state: State<'_, AppState>,
  request: YourRequest,
) -> Result<YourResponse, String>
```

```ts
await api.invoke("your_command", { request: { /* ... */ } });
```

## Key Contribution Focus Areas

1. Contribute good ideas/creativity (features, interactions, visuals, etc.) by opening issues
   > Product managers and UI designers are welcome to submit ideas quickly via PI. We will help refine them for development.
2. Improve the Agent system and overall quality
3. Improve system stability and strengthen foundational capabilities
4. Expand the ecosystem (Skills, MCP, LSP plugins, or better support for domain-specific development scenarios)

## Contribution Workflow and PR Expectations

### What to Contribute (Beyond Features and Fixes)

We welcome contributions beyond standard feature or bug-fix PRs. Examples include:

| Contribution area | Location / files | Example |
| --- | --- | --- |
| Prompts | `src/crates/core/src/agentic/agents/prompts/` | Add or refine prompts, and update related logic as needed |
| Tools | `src/crates/core/src/agentic/tools/implementations/`, `src/crates/core/src/agentic/tools/registry.rs` | Add tool implementations and register them in the tool registry |
| Subagents | `src/crates/core/src/agentic/agents/custom_subagents/`, `src/crates/core/src/agentic/agents/registry.rs` | Add subagent implementations and register them in the subagent registry |
| Mode contributions | `src/crates/core/src/agentic/agents/*_mode.rs`, `src/crates/core/src/agentic/agents/prompts/*_mode.md`, `src/web-ui/src/locales/*/settings/modes.json` | Add/improve agent modes (e.g. Plan/Debug/Agentic or custom modes) and keep prompts + UI copy in sync |
| Scenario guides for Code Agent and AIIde | `website/src/docs/` | Add workflows, playbooks, and real-world scenario docs (or link them from `README.md`) |

### Before you start

- Open an issue to describe the problem or proposal, especially for larger changes, to avoid duplication and design conflicts
- For new features or UI changes, discuss the design direction early to ensure it fits the product experience

### PR title and description

We recommend using Conventional Commits for clearer history and better automation:

- `feat:` new feature
- `fix:` bug fix
- `docs:` documentation
- `chore:` maintenance/deps
- `refactor:` refactor without behavior change
- `test:` tests

UI changes should include before/after screenshots or a short recording for fast review.

If your work is AI-assisted, please note it in the PR and indicate testing level (untested/lightly tested/fully tested) to help reviewers assess risk.

### Branch management

**The `main` branch is the default collaboration branch and accepts feature PRs.** Since this repo encourages product managers and developers to use AI-generated code for rapid validation or idea submission, **please open all PRs targeting the `main` branch**.

### Scope

Keep PRs small and focused. Avoid bundling unrelated changes.

## Testing and Verification

Run relevant tests for your change:

```bash
# Rust
cargo test --workspace

# E2E
pnpm run e2e:test
```

If you cannot run tests, explain why in the PR and provide manual verification steps.



## Security and Compliance

- Do not commit secrets, tokens, certificates, or any sensitive data
- When adding dependencies, ensure license compatibility and explain the purpose

## Thanks

Every contribution matters. Issues, PRs, and suggestions are all welcome!

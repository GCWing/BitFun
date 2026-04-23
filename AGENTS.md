[中文](AGENTS-CN.md) | **English**

# AGENTS.md

## Overview

BitFun is a Rust workspace plus a shared React frontend.

Repository rule: **keep product logic platform-agnostic, then expose it through platform adapters**.

- `src/crates/core`: shared product logic center
- `src/crates/transport`: Tauri / WebSocket / CLI adapters
- `src/crates/api-layer`: shared handlers and DTOs
- `src/apps/desktop`: Tauri host app
- `src/apps/server`: web backend runtime
- `src/apps/cli`: CLI runtime
- `src/web-ui`: shared frontend for desktop and server/web
- `BitFun-Installer`: separate installer app
- `tests/e2e`: desktop E2E tests

## 3-step onboarding

1. Read `README.md`, `CONTRIBUTING.md`, and this file before architecture-sensitive changes.
2. Prefer `pnpm run desktop:preview:debug` for fast manual desktop checks after shared frontend changes when a recent debug desktop binary already exists. Prefer `pnpm run desktop:preview:debug:rebuild` after Rust / Tauri changes when you want the quickest local rebuild-and-preview loop. Keep `pnpm run desktop:dev` for the full Tauri dev flow, first-time setup, or startup/build-pipeline debugging, and use `pnpm run dev:web` for browser-only frontend work.
3. After changes, run the smallest matching verification set below.

## Core commands

```bash
# Install
pnpm install
pnpm run e2e:install

# Main dev flows
pnpm run desktop:dev
pnpm run desktop:preview:debug
pnpm run desktop:preview:debug:rebuild
pnpm run dev:web
pnpm run cli:dev
pnpm run installer:dev

# Frontend
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/web-ui run test:run
pnpm run build:web

# Rust
cargo check --workspace
cargo test --workspace
cargo test -p bitfun-core <test_name> -- --nocapture

# Desktop / E2E
cargo build -p bitfun-desktop
pnpm run e2e:test:l0
pnpm --dir tests/e2e exec wdio run ./config/wdio.conf.ts --spec "./specs/<file>.spec.ts"
```

## Fast Local Desktop Loops

- `pnpm run desktop:preview:debug` starts or reuses the web dev server and launches the existing `target/debug/bitfun-desktop(.exe)` without `tauri dev`; prefer it for frontend-only manual desktop checks.
- `pnpm run desktop:preview:debug:rebuild` first rebuilds `bitfun-desktop` in dev mode with `CARGO_PROFILE_DEV_DEBUG=0` and high codegen parallelism, then launches the same preview flow; prefer it for temporary local Rust / Tauri debugging when you care most about turnaround time.
- These preview commands are for local iteration speed only. They do not replace the minimum verification set below before you finish the task.
- If the user intent is to "quickly check the effect", "run locally for a quick look", or similar manual inspection, prefer the preview commands above even when the request also mentions "build" or "debug version".
- Reserve `pnpm run desktop:build:fast` for cases where the user explicitly wants a debug build artifact and does not need the app launched for preview.
- Intent examples:
  - "build a local debug version and quickly inspect it" -> `pnpm run desktop:preview:debug:rebuild`
  - "build me a debug artifact only, no need to launch it" -> `pnpm run desktop:build:fast`

## Packaging Requests

- When the user asks to package, release, or build a distributable desktop artifact without naming the exact output form, confirm the intended package type before running the build.
- Distinguish local temporary artifacts from real release deliverables. Do not treat `desktop:preview:*`, debug builds, or `--no-bundle` fast outputs as the final user-facing release unless the user explicitly asks for that form.
- If the user clearly wants a Windows installer for end users, prefer `pnpm run desktop:build:nsis`.
- If the user clearly wants a standalone Windows executable instead of an installer, prefer `pnpm run desktop:build:exe`.
- If the user already names the exact target format, do not ask again; just use the requested packaging flow.

## Architecture

### Backend flow

Trace most features in this order:

1. `src/web-ui` or app entrypoint
2. `src/apps/desktop/src/api/*` or server routes
3. `src/crates/api-layer`
4. `src/crates/transport`
5. `src/crates/core`

### `bitfun-core`

`src/crates/core` is the center of the codebase.

Important areas:

- `agentic/`: agents, prompts, tools, sessions, execution, persistence
- `service/`: config, filesystem, terminal, git, LSP, MCP, remote connect, project context, AI memory
- `infrastructure/`: AI clients, app paths, event system, storage, debug log server

Agent runtime mental model:

```text
SessionManager → Session → DialogTurn → ModelRound
```

Session data is stored under `.bitfun/sessions/{session_id}/`.

### Frontend and desktop boundaries

- `src/web-ui` serves both Tauri desktop and server/web
- Do not call Tauri APIs directly from UI components; go through the adapter/infrastructure layer
- Desktop-only integrations belong in `src/apps/desktop`, then flow back through transport/API layers
- In shared core, avoid host-specific APIs such as `tauri::AppHandle`; use shared abstractions such as `bitfun_events::EventEmitter`

## Repository-specific rules

### Logging

Logs must be English-only, with no emojis.

- Frontend: `src/web-ui/LOGGING.md`
- Backend: `src/crates/LOGGING.md`

Patterns:

```ts
const log = createLogger('ModuleName');
log.info('Loaded items', { count });
```

```rust
use log::{debug, error, info, trace, warn};
info!("Registered adapter for session {}", session_id);
```

### Tauri commands

- command names: `snake_case`
- Rust side: `snake_case`
- TypeScript may wrap with `camelCase`, but invoke Rust with a structured `request`

```rust
#[tauri::command]
pub async fn your_command(
    state: State<'_, AppState>,
    request: YourRequest,
) -> Result<YourResponse, String>
```

```ts
await api.invoke('your_command', { request: { ... } });
```

### Extra narrow rules

- If you modify `src/crates/ai-adapters`, run the stream integration tests in `src/crates/core/tests`
- If you modify `src/crates/core/src/agentic/execution/stream_processor.rs`, run the stream integration tests before finishing

## Where to look first

- Agent modes: `src/crates/core/src/agentic/agents/`, `src/crates/core/src/agentic/agents/prompts/`, `src/web-ui/src/locales/*/scenes/agents.json`
- Tools: `src/crates/core/src/agentic/tools/implementations/`, `src/crates/core/src/agentic/tools/registry.rs`
- MCP / LSP / remote: `src/crates/core/src/service/mcp/`, `src/crates/core/src/service/lsp/`, `src/crates/core/src/service/remote_connect/`, `src/crates/core/src/service/remote_ssh/`
- Desktop APIs: `src/apps/desktop/src/api/`, `src/crates/api-layer/src/`, `src/crates/transport/src/adapters/tauri.rs`
- Web/server communication: `src/web-ui/src/infrastructure/api/`, `src/crates/transport/src/adapters/websocket.rs`, `src/apps/server/src/routes/`, `src/apps/server/src/main.rs`

## Verification

| Change type | Minimum verification |
| --- | --- |
| Frontend UI, state, adapters, or locales | `pnpm run lint:web && pnpm run type-check:web && pnpm --dir src/web-ui run test:run` |
| Shared Rust logic in `core`, `transport`, `api-layer`, or services | `cargo check --workspace && cargo test --workspace` |
| Desktop integration, Tauri APIs, browser/computer-use, or desktop-only behavior | `cargo check -p bitfun-desktop && cargo test -p bitfun-desktop` |
| Behavior covered by desktop smoke/functional flows | `cargo build -p bitfun-desktop` then the nearest E2E spec or `pnpm run e2e:test:l0` |
| `src/crates/ai-adapters` | Relevant Rust checks above **and** stream integration tests in `src/crates/core/tests` |
| Installer app | `pnpm run installer:build` |

## Agent-doc coverage

This is the repository-wide guide.

Rule priority:

- prefer the nearest matching `AGENTS.md` / `AGENTS-CN.md` for the directory you are changing
- if local guidance conflicts with this file, follow the more specific, nearer document

Prefer the nearest matching agent doc when present:

- `src/web-ui/AGENTS.md`
- `src/crates/core/AGENTS.md`
- `src/apps/desktop/AGENTS.md`
- `tests/e2e/AGENTS.md`
- `BitFun-Installer/AGENTS.md`
- `src/crates/ai-adapters/AGENTS.md`
- `src/crates/core/src/agentic/execution/AGENTS.md`


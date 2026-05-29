[中文](AGENTS-CN.md) | **English**

# AGENTS.md

BitFun is a Rust workspace plus React frontends.

Repository rule: **keep product logic platform-agnostic, then expose it through platform adapters**.

## Quick start

1. Read `README.md` and `CONTRIBUTING.md` before architecture-sensitive changes.
2. For desktop development, prefer `pnpm run desktop:dev` — it provides full hot-reload (Vite HMR + Rust auto-rebuild & restart). Use `pnpm run desktop:preview:debug` only when you need a faster cold-start for frontend-only iteration (Rust changes are not auto-rebuilt).
3. After Rust file changes, prefer `pnpm run fmt:rs` to format only changed or staged `.rs` files. Use `cargo fmt` only when you intentionally want broader formatting coverage.
4. After changes, run the smallest matching verification from the table below.

## Module index

| Module | Path | Agent doc |
|---|---|---|
| Core (product logic) | `src/crates/core` | [AGENTS.md](src/crates/core/AGENTS.md) |
| Extracted core support | `src/crates/{core-types,agent-stream,runtime-ports,terminal,tool-runtime}` | (use core guide) |
| Service core owner crate | `src/crates/services-core` | [AGENTS.md](src/crates/services-core/AGENTS.md) |
| Service integrations owner crate | `src/crates/services-integrations` | [AGENTS.md](src/crates/services-integrations/AGENTS.md) |
| Agent tool contracts | `src/crates/agent-tools` | [AGENTS.md](src/crates/agent-tools/AGENTS.md) |
| Tool pack provider plan | `src/crates/tool-packs` | [AGENTS.md](src/crates/tool-packs/AGENTS.md) |
| Product domains | `src/crates/product-domains` | [AGENTS.md](src/crates/product-domains/AGENTS.md) |
| Transport adapters | `src/crates/transport` | (use core guide) |
| API layer | `src/crates/api-layer` | (use core guide) |
| ACP integration | `src/crates/acp` | [AGENTS.md](src/crates/acp/AGENTS.md) |
| AI adapters | `src/crates/ai-adapters` | [AGENTS.md](src/crates/ai-adapters/AGENTS.md) |
| Desktop app | `src/apps/desktop` | [AGENTS.md](src/apps/desktop/AGENTS.md) |
| Server | `src/apps/server` | (use core guide) |
| CLI | `src/apps/cli` | (use core guide) |
| Relay server | `src/apps/relay-server` | (use core guide) |
| Shared frontend | `src/web-ui` | [AGENTS.md](src/web-ui/AGENTS.md) |
| Mobile web | `src/mobile-web` | [AGENTS.md](src/mobile-web/AGENTS.md) |
| Installer | `BitFun-Installer` | [AGENTS.md](BitFun-Installer/AGENTS.md) |
| E2E tests | `tests/e2e` | [AGENTS.md](tests/e2e/AGENTS.md) |

## Most-used commands

```bash
# Install
pnpm install

# Dev
pnpm run desktop:dev               # full hot-reload: Vite HMR + Rust auto-rebuild & restart
pnpm run desktop:preview:debug     # reuse pre-built binary + Vite HMR; no Rust auto-rebuild
pnpm run dev:web                   # browser-only frontend
pnpm run cli:dev                   # CLI runtime

# Check
pnpm run fmt:rs                     # format only changed / staged Rust files
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/mobile-web run type-check
pnpm run check:repo-hygiene
pnpm run check:github-config
cargo check --workspace

# Test
pnpm --dir src/web-ui run test:run
cargo test --workspace

# Build
cargo build -p bitfun-desktop
pnpm run build:web
pnpm run build:mobile-web

# Fast builds (for development / CI speed)
pnpm run desktop:build:fast           # debug build, no bundling
pnpm run desktop:build:release-fast   # release with reduced LTO
pnpm run desktop:build:nsis:fast      # Windows installer, release-fast profile
pnpm run installer:build:fast         # installer app, fast mode
```

For the full script list, see [`package.json`](package.json).

## Global rules

### Logging

Logs must be English-only, with no emojis.

- Frontend: [`src/web-ui/LOGGING.md`](src/web-ui/LOGGING.md)
- Backend: [`src/crates/LOGGING.md`](src/crates/LOGGING.md)

### Tauri commands

- Command names: `snake_case`
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

### Platform boundaries

- Do not call Tauri APIs directly from UI components; go through the adapter/infrastructure layer.
- Desktop-only integrations belong in `src/apps/desktop`, then flow back through transport/API layers.
- In shared core, avoid host-specific APIs such as `tauri::AppHandle`; use shared abstractions such as `bitfun_events::EventEmitter`.

### Remote compatibility

- When adding features, consider remote workspace and remote control synchronization support from the start. Local-only behavior can silently leave remote scenarios incomplete.
- If a feature cannot reasonably support remote workspaces, gate it or show a clear unsupported-state message instead of letting it fail with a generic error.

### Agent loop behavior

- Do not add hard-coded limits or pattern checks to the agent loop as a first response to looping behavior, such as blocking repeated tool calls by string or count alone.
- Excessive hard-coding turns the agent loop into a brittle workflow engine. Investigate the root cause first: tool behavior, model interaction, session context packaging, prompt/tool schema design, or state synchronization issues.

## Architecture

### Core decomposition guardrails

For any `bitfun-core` decomposition, feature-boundary, dependency-boundary, or
Rust build-speed refactor, read
[`docs/architecture/core-decomposition.md`](docs/architecture/core-decomposition.md)
before editing. Keep this file as an entry point; put module-specific ownership
details in the nearest module `AGENTS.md`.

Repository-level decomposition rules:

- Do not confuse DTO/contract extraction with runtime owner migration.
- Product surfaces may diverge; share stable facts or ports, not UI, protocol,
  lifecycle, or platform implementation.
- Moving runtime ownership requires a reviewed port/provider design, old-path
  compatibility, behavior equivalence tests, and explicit confirmation when a
  behavior boundary could change.

### DeepReview guardrails

Deep Review / Code Review Team work spans the core runtime and web UI. Keep
target resolution and manifest construction on the frontend; keep policy
validation, queue/retry state, and report enrichment in shared core.

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

## Verification

| Change type | Minimum verification |
|---|---|
| Frontend UI, state, adapters, or locales | `pnpm run lint:web && pnpm run type-check:web && pnpm --dir src/web-ui run test:run` |
| Mobile web UI, state, pairing, disconnect, or reconnect behavior | `pnpm --dir src/mobile-web run type-check && pnpm run build:mobile-web`; include manual pairing / reconnect verification when behavior changes |
| Deep Review / Code Review Team behavior | Web UI verification above, plus `cargo test -p bitfun-core deep_review -- --nocapture`; also run the Rust / desktop rows below when backend or Tauri APIs are touched |
| Shared Rust logic in `core`, `transport`, `api-layer`, or services | `cargo check --workspace && cargo test --workspace` |
| Desktop integration, Tauri APIs, browser/computer-use, or desktop-only behavior | `cargo check -p bitfun-desktop && cargo test -p bitfun-desktop` |
| Behavior covered by desktop smoke/functional flows | `cargo build -p bitfun-desktop` then the nearest E2E spec or `pnpm run e2e:test:l0` |
| `src/crates/ai-adapters` | Relevant Rust checks above **and** `cargo test -p bitfun-agent-stream` for stream contracts |
| Installer app | `pnpm run installer:build` |

## Where to look first

| Feature | Key paths |
|---|---|
| Agent modes | `src/crates/core/src/agentic/agents/`, `src/crates/core/src/agentic/agents/prompts/`, `src/web-ui/src/locales/*/scenes/agents.json` |
| Deep Review / Code Review Team | `src/crates/core/src/agentic/deep_review/`, `src/crates/core/src/agentic/deep_review_policy.rs`, `src/crates/core/src/agentic/agents/definitions/hidden/deep_review.rs`, `src/crates/core/src/agentic/tools/implementations/{task_tool.rs,code_review_tool.rs}`, `src/web-ui/src/shared/services/review-team/`, `src/web-ui/src/flow_chat/deep-review/`, `src/web-ui/src/app/scenes/agents/components/ReviewTeamPage.tsx` |
| Mobile web pairing / remote control | `src/mobile-web/src/pages/PairingPage.tsx`, `src/mobile-web/src/pages/SessionListPage.tsx`, `src/mobile-web/src/pages/ChatPage.tsx`, `src/mobile-web/src/services/RemoteSessionManager.ts`, `src/mobile-web/src/services/RelayHttpClient.ts`, `src/mobile-web/src/services/store.ts` |
| Session usage report (`/usage`) | `src/crates/core/src/service/session_usage/`, `src/web-ui/src/flow_chat/components/usage/`, `src/web-ui/src/locales/*/flow-chat.json` |
| Tools | `src/crates/core/src/agentic/tools/implementations/`, `src/crates/core/src/agentic/tools/registry.rs` |
| MCP / LSP / remote | `src/crates/core/src/service/mcp/`, `src/crates/core/src/service/lsp/`, `src/crates/core/src/service/remote_connect/`, `src/crates/core/src/service/remote_ssh/` |
| Desktop APIs | `src/apps/desktop/src/api/`, `src/crates/api-layer/src/`, `src/crates/transport/src/adapters/tauri.rs` |
| Relay server | `src/apps/relay-server/` |
| Web/server communication | `src/web-ui/src/infrastructure/api/`, `src/crates/transport/src/adapters/websocket.rs`, `src/apps/server/src/routes/`, `src/apps/server/src/main.rs` |

## Agent-doc priority

Prefer the nearest matching `AGENTS.md` / `AGENTS-CN.md` for the directory you are changing. If local guidance conflicts with this file, follow the more specific, nearer document.

# AGENTS.md

## Project Overview

BitFun is an AI agent-driven programming environment built with Rust and TypeScript, using multi-platform architecture (Desktop/CLI/Server) sharing a common core library.

### Architecture

- **src/crates/events** - Event definitions (platform-agnostic)
- **src/crates/core** - Core business logic (95%+ code reuse)
  - `agentic/` - Agent system (session, tools, execution)
  - `service/` - Workspace, Config, FileSystem, Terminal, Git
  - `infrastructure/` - AI client, storage, logging, events
- **src/crates/transport** - Transport adapters (CLI, Tauri, WebSocket)
- **src/crates/api-layer** - Platform-agnostic handlers
- **src/apps/desktop** - Tauri 2.0 desktop app
- **src/apps/cli** - Terminal UI（WIP）
- **src/apps/server** - Web server (Axum + WebSocket)（WIP）
- **src/web-ui** - React frontend
  - `infrastructure/` - Theme, I18n, Config, State management, API adapters
  - `component-library/` - Shared UI components
  - `tools/` - Feature modules (editor, git, terminal, mermaid...)
  - `flow_chat/` - Chat UI
  - `locales/` - Translation files (en-US, zh-CN)

### Key Design Principles

1. **Dependency Injection** - Services receive dependencies via constructors
2. **EventEmitter Pattern** - Use `Arc<dyn EventEmitter>` not `AppHandle`
3. **TransportAdapter Pattern** - Abstract communication across platforms
4. **Platform Agnostic Core** - No platform-specific dependencies in core

### Tech Stack

- **Backend**: Rust 2021, Tokio, Tauri 2.0, Axum
- **Frontend**: React 18, TypeScript, Vite, Zustand

## Development Commands

```bash
# Desktop
pnpm run desktop:dev             # Dev mode

# E2E
pnpm run e2e:test
```

## Critical Rules

### Logging

**Rules:** English only, no emojis, structured data, avoid verbose logging

- **Frontend**: `src/web-ui/LOGGING.md` - Use `createLogger('ModuleName')`
- **Backend**: `src/crates/LOGGING.md` - Use `log::{info, debug, ...}` macros

### Transport Layer

**Never use platform-specific APIs in core code:**
- ❌ `use tauri::AppHandle`
- ✅ `use bitfun_events::EventEmitter`

### Tauri Commands

**Naming:** Commands `snake_case`, Rust `snake_case`, TypeScript `camelCase`

**Always use structured request format:**

```rust
#[tauri::command]
pub async fn your_command(
    state: State<'_, AppState>,
    request: YourRequest,
) -> Result<YourResponse, String>
```

```typescript
await api.invoke('your_command', { request: { ... } });
```

### Frontend Reuse

When developing frontend features, reuse existing infrastructure:
- **Theme**: `infrastructure/theme/` - useTheme, useThemeToggle
- **I18n**: `infrastructure/i18n/` + `locales/` - useI18n, t()
- **Components**: `component-library/` - shared UI components
- **State**: Zustand stores in each module

## Key Components

### Agentic System

```
SessionManager → Session → DialogTurn → ModelRound
```

- `ConversationCoordinator` - Orchestrates turns
- `ExecutionEngine` - Multi-round loop
- `ToolPipeline` - Tool execution with concurrency

### Session Persistence

Location: `.bitfun/sessions/{session_id}/`

### Tool Development

Register in `agentic/tools/registry.rs`:
1. Implement `Tool` trait
2. Define input/output types
3. Handle streaming if applicable

### Adding Agents

In `agentic/agents/`:
1. Create agent file
2. Define prompt in `prompts/`
3. Register in `registry.rs`

## Frontend Debugging

A local log receiver server is available at `scripts/debug-log-server.mjs`.

**Start the server:**
```bash
node scripts/debug-log-server.mjs
# Listens on http://127.0.0.1:7469, writes logs to debug-agent.log
```

**Instrument code (one-liner fetch):**
```typescript
fetch('http://127.0.0.1:7469/log',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({location:'file.ts:LINE',message:'desc',data:{k:v},timestamp:Date.now()})}).catch(()=>{});
```

**Clear logs between runs:**
```bash
# Via HTTP
curl -X POST http://127.0.0.1:7469/clear
```

Logs are written to `debug-agent.log` in project root as NDJSON. The agent reads this file directly — no copy-paste needed.

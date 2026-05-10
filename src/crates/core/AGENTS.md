[中文](AGENTS-CN.md) | **English**

# AGENTS.md

## Scope

This file applies to `src/crates/core`. Use the top-level `AGENTS.md` for repository-wide rules.

## What matters here

`bitfun-core` is the shared product-logic center.

Main areas:

- `src/agentic/`: agents, prompts, tools, sessions, execution, persistence
- `src/service/`: config, filesystem, terminal, git, LSP, MCP, remote connect, project context, AI memory
- `src/infrastructure/`: AI clients, app paths, event system, storage, debug log server

Agent runtime mental model:

```text
SessionManager → Session → DialogTurn → ModelRound
```

## Local rules

- Keep shared core platform-agnostic
- Avoid host-specific APIs such as `tauri::AppHandle`
- Use shared abstractions such as `bitfun_events::EventEmitter`
- Desktop-only integrations belong in `src/apps/desktop`, then flow through transport/API layers
- During core decomposition, `bitfun-core` is a compatibility facade and full
  product runtime assembly point. New modules should prefer the extracted owner
  crate listed in `docs/architecture/core-decomposition.md`.
- Do not add new cross-layer references from `service` to `agentic` without a
  small port/interface boundary.
- Do not move platform-specific logic, build-script behavior, or product
  capability selection into shared core as part of decomposition.

Narrower rules already exist:

- `src/crates/ai-adapters/AGENTS.md`
- `src/agentic/execution/AGENTS.md`

## Commands

```bash
cargo check --workspace
cargo test --workspace
cargo test -p bitfun-core <test_name> -- --nocapture
```

## Verification

```bash
cargo check --workspace && cargo test --workspace
```

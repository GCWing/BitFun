[中文](AGENTS-CN.md) | **English**

# Core Agent Guide

## Scope

This file applies to `src/crates/core`. Use the top-level `AGENTS.md` for
repository-wide rules and the nearest narrower guide when one exists.

## Role

`bitfun-core` is the shared product runtime facade. It still owns compatibility
paths and the `product-full` assembly boundary, but new decomposition work should
prefer the owner crates described in `docs/architecture/core-decomposition.md`
and `docs/architecture/agent-runtime-services-design.md`.

Main areas:

- `src/agentic/`: agents, prompts, tools, sessions, execution, persistence
- `src/service/`: config, filesystem, terminal, git, LSP, MCP, remote connect, project context, AI memory
- `src/infrastructure/`: AI clients, app paths, event system, storage, debug log server

Agent runtime mental model:

```text
SessionManager -> Session -> DialogTurn -> ModelRound
```

## Boundary Rules

- Keep shared core platform-agnostic. Avoid host-specific APIs such as
  `tauri::AppHandle`; use shared abstractions such as
  `bitfun_events::EventEmitter`.
- Desktop-only integrations belong in `src/apps/desktop`, then flow through
  transport/API layers.
- Do not add new cross-layer references from `service` to `agentic` without a
  narrow port/interface boundary.
- Do not move platform-specific logic, build-script behavior, product capability
  selection, or provider-specific AI serialization into shared core.
- When moving ownership out of core, preserve old import paths with facade or
  re-export code until downstream call sites are intentionally migrated.

## Decomposition Rules

- Treat `bitfun-core` as a compatibility facade plus full product assembly point,
  not as the preferred home for new stable contracts.
- Put stable DTOs, facts, ports, and pure decisions in the matching owner crate
  where a clear owner exists. Keep concrete managers, IO, platform adapters, and
  product execution in core until a reviewed port/provider design and behavior
  equivalence tests exist.
- Tool changes must preserve expanded/collapsed exposure, prompt-visible
  manifests, `GetToolSpec`, permission behavior, `ToolUseContext` semantics, and
  desktop/MCP/ACP catalog behavior.
- Runtime-owner migrations must keep concrete lifecycle, IO, event delivery,
  permission orchestration, and remote/platform providers in core until the
  target owner has a reviewed port/provider design plus behavior-equivalence
  tests.
- Product-domain changes may move pure product-domain plans with equivalence
  coverage, but filesystem writes, worker/host side effects, Git/AI concrete
  calls, marker IO, and path-manager integration stay in core unless a reviewed
  owner design says otherwise.
- Remote/service changes must keep external protocol lifecycle, workspace
  projection, scheduler/session restore, terminal pre-warm, and product
  execution boundaries explicit.
- Feature work must keep `product-full` as the compatibility product assembly
  boundary unless a separate product matrix review changes default capability
  selection.

## Owner References

Use these files for ownership details instead of expanding this guide:

- `docs/architecture/core-decomposition.md`
- `docs/architecture/agent-runtime-services-design.md`
- `src/crates/agent-runtime/AGENTS.md`
- `src/crates/agent-tools/AGENTS.md`
- `src/crates/harness/AGENTS.md`
- `src/crates/product-domains/AGENTS.md`
- `src/crates/runtime-ports/` and `src/crates/runtime-services/` source docs
- `src/crates/services-core/AGENTS.md`
- `src/crates/services-integrations/AGENTS.md`
- `src/crates/tool-packs/AGENTS.md`

Narrower local guides already exist for some subtrees:

- `src/crates/ai-adapters/AGENTS.md`
- `src/agentic/execution/AGENTS.md`
- `src/agentic/deep_review/AGENTS.md`

## Verification

Use the smallest check that matches the touched behavior:

```bash
cargo check --workspace
cargo test -p bitfun-core <test_name> -- --nocapture
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

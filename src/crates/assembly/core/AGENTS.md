[中文](AGENTS-CN.md) | **English**

# Core Agent Guide

## Scope

This file applies to `src/crates/assembly/core`. Use the top-level `AGENTS.md` for
repository-wide rules and the nearest narrower guide when one exists.

## Role

`bitfun-core` is the shared product runtime facade. It still owns compatibility
paths and the `product-full` assembly boundary, but new decomposition work should
prefer the owner crates described in `docs/architecture/product-architecture.md`
and `docs/architecture/agent-runtime-services-design.md`.

Main areas:

- `src/agentic/`: agents, prompts, tools, sessions, execution, persistence
- `src/service/`: config, filesystem, terminal, git, LSP, MCP, remote connect, AI memory
- `src/infrastructure/`: AI clients, app paths, event system, storage, debug log server
- `src/product_runtime/`: Core Agent Runtime compatibility adapters and runtime service provider wiring

Agent runtime mental model:

```text
SessionManager -> Session -> DialogTurn -> ModelRound
```

## Boundary Rules

- Keep shared core platform-agnostic. Avoid host-specific APIs such as
  `tauri::AppHandle`; use shared abstractions such as
  `bitfun_events::EventEmitter`.
- Desktop-only host adapters belong in `src/apps/desktop`, then flow through
  typed capability interfaces; use the production transport adapter when event
  delivery is needed.
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
  product execution in core until a reviewed port/adapter/service design and
  behavior equivalence tests exist.
- Tool changes must preserve expanded/collapsed exposure, prompt-visible
  manifests, `GetToolSpec`, permission behavior, `ToolUseContext` semantics, and
  desktop/MCP/ACP catalog behavior.
- Runtime-owner migrations must keep concrete lifecycle, IO, event delivery,
  permission orchestration, and remote/platform implementations in core until
  the target owner has a reviewed port/adapter/service design plus
  behavior-equivalence tests.
- Product-domain changes may move pure product-domain plans with equivalence
  coverage, but filesystem writes, worker/host side effects, Git/AI concrete
  calls, marker IO, and path-manager integration stay in core unless a reviewed
  owner design says otherwise.
- `plugin_source` may inject product-owned paths and keep compatibility exports;
  concrete managed-package discovery and trust persistence stay in
  `services-integrations`, while ecosystem parsing and PluginRuntimeClient
  behavior remain in their adapter and execution owners.
- `plugin_runtime`, `external_sources`, and `instruction_sources` are the
  reviewed owner-feature composition files allowed to select ecosystem adapters
  for their respective capability contracts. Product surfaces consume
  product-level views and must not import adapter or raw plugin runtime client
  types.
- External-source Desktop, TUI, Peer, and Server surfaces share the versioned
  product-domain control DTO and closed generic actions. Capability-specific
  approvals and conflict choices remain typed owner operations; do not add a
  second surface-specific lifecycle model or arbitrary control payload.
- Remote/service changes must keep external protocol lifecycle, workspace
  projection, scheduler/session restore, terminal pre-warm, and product
  execution boundaries explicit.
- Feature work must keep `product-full` as the compatibility product assembly
  boundary unless a separate product matrix review changes default capability
  selection.
- `agent-runtime` owns the existing Core Agent Runtime compatibility facade,
  including its MCP, Remote Connect, workspace-search, and native Hook runtime
  services. `external-sources` adds third-party discovery/import adapters,
  `plugin-runtime` adds executable plugin-client wiring, and `debug-log` keeps
  the debug ingest server separate. None may enable `product-full`.
- CLI/ACP closure checks keep Cargo resolver-v2 normal and host
  (build/proc-macro) feature contexts separate, while treating all
  target-specific declarations within each context as one reviewed architecture
  boundary. Split a package/module owner when platforms genuinely differ; do
  not hide an unreviewed Core capability behind mutually exclusive Cargo `cfg`
  branches.
- Keep the light compatibility features independently compilable. Local service
  profiles are `dispatch-store`, `lsp`, `terminal`, `workspace-runtime`, and
  `workspace-watch`; `remote-workspace` adds only the remote workspace facade,
  while `ssh-remote` adds concrete SSH transport. Integration facades
  `announcement`, `file-watch`, `git`, and `review-platform` remain independent,
  with `service-integrations` only their compatibility aggregate. None of these
  narrow features may enable `product-full` directly or transitively.
- `product-full` must explicitly compose every capability it consumes, including
  product-only `services-core` features such as `permission`, `session-git`, and
  `runtime-ownership`. Do not put those features on the dependency declaration,
  because Cargo feature union would force them into every core consumer.
- Keep `cargo check -p bitfun-core --no-default-features` viable. Gate
  product-only modules at their owner feature; if a light facade operation
  cannot safely complete without a product owner, fail closed and preserve any
  durable recovery state instead of enabling `product-full` implicitly.

## Owner References

Use these files for ownership details instead of expanding this guide:

- `docs/architecture/product-architecture.md`
- `docs/architecture/agent-runtime-services-design.md`
- `src/crates/execution/agent-runtime/AGENTS.md`
- `src/crates/execution/tool-contracts/AGENTS.md`
- `src/crates/execution/harness/AGENTS.md`
- `src/crates/contracts/product-domains/AGENTS.md`
- `src/crates/contracts/runtime-ports/` and `src/crates/execution/runtime-services/` source docs
- `src/crates/services/services-core/AGENTS.md`
- `src/crates/services/services-integrations/AGENTS.md`
- `src/crates/execution/tool-provider-groups/AGENTS.md`

Narrower local guides already exist for some subtrees:

- `src/crates/adapters/ai-adapters/AGENTS.md`
- `src/crates/assembly/core/src/agentic/execution/AGENTS.md`
- `src/crates/assembly/core/src/agentic/deep_review/AGENTS.md`

## Verification

Use the smallest check that matches the touched behavior:

```bash
cargo check --workspace
cargo check -p bitfun-core --no-default-features
cargo check -p bitfun-core --no-default-features --features workspace-runtime
cargo check -p bitfun-core --no-default-features --features remote-workspace
cargo check -p bitfun-core --no-default-features --features ssh-remote
cargo test -p bitfun-core --lib <test_name> -- --nocapture
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

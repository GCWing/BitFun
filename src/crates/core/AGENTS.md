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
- For tools, keep lightweight contracts, pure manifest/exposure contracts,
  generic contextual prompt-manifest resolver contracts, generic catalog
  snapshot provider contracts, generic GetToolSpec catalog provider/detail/
  summary/static tool surface/execution-plan/provider-backed runtime facade / execution-result/
  result-vector adapter / result-assembly helpers, and portable tool context facts/provider plus generic registry / static-provider / dynamic-provider container
  contracts in `bitfun-agent-tools`. Provider-backed visible-tools / prompt-visible manifest / readonly catalog runtime facades,
  generic decorator references, snapshot decorator adapters, static-provider runtime assembly, and readonly/enabled
  registry-snapshot filtering belong in
  `bitfun-agent-tools`; core tool runtime should materialize concrete tools from the `bitfun-tool-packs`
  provider group plan and keep product snapshot wrapper adapter injection in
  `runtime_assembly.rs` + `static_providers.rs`, adapt core `Tool` into
  provider-neutral contracts through `tool_adapter.rs`, keep product catalog
  access and product manifest / GetToolSpec facade wiring in
  `catalog_provider.rs`, apply snapshot decoration, and own on-demand spec
  discovery Tool impl and unlock-state source for now.
  `bitfun-tool-packs` may expose planned
  feature-group scaffold metadata, but it must not own concrete tools yet.
- Keep `ToolUseContext` and concrete tool implementations in core unless a
  reviewed port/provider plan and equivalence tests exist. `ToolContextFacts`
  / `PortableToolContextProvider` are only portable projections; they must not
  carry runtime handles, workspace services, or cancellation tokens.
- Any tool migration must preserve expanded/collapsed exposure, prompt-visible
  manifests, `ToolUseContext.unlocked_collapsed_tools`, and desktop/MCP/ACP
  tool catalog behavior.
- Do not encode provider-specific OpenAI Responses / Codex ChatGPT flat tool
  schema behavior in core tool contracts; AI adapters own provider
  serialization while core keeps provider-neutral manifests.
- When touching session/token usage paths, keep `cached_content_token_count`
  as cache reads/hits and `cache_creation_token_count` as a separate provider
  fact.
- Function-agent commit-message and Startchat work-state orchestration may
  route through `bitfun-product-domains`; keep Git/AI service adapters, prompt
  templates, JSON extraction, and error mapping core-owned. JSON-string parsing
  helpers may live in `bitfun-product-domains` after extraction has happened.
- MiniApp built-in bundle/hash/marker seed-plan and marker wire helpers may
  live in `bitfun-product-domains`; keep bundled asset includes, filesystem
  writes, marker IO, customization metadata IO, recompile orchestration, worker
  process runtime, and host dispatch execution core-owned until a reviewed
  migration proves equivalence.
- Do not add new cross-layer references from `service` to `agentic` without a
  small port/interface boundary.
- Do not move platform-specific logic, build-script behavior, or product
  capability selection into shared core as part of decomposition.

Narrower rules already exist:

- `src/crates/ai-adapters/AGENTS.md`
- `src/agentic/execution/AGENTS.md`
- `src/agentic/deep_review/AGENTS.md`

## DeepReview notes

- Keep policy, manifest gate, queue state, Task adapter, and report enrichment
  aligned when changing `src/agentic/deep_review*` or review agents.
- Keep reviewer subagents read-only; user-approved remediation is outside the
  reviewer pass.

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

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
  provider group plan through `product_runtime.rs`, adapt core `Tool` into
  provider-neutral contracts through `tool_adapter.rs`, and keep product
  registry snapshot access, product manifest / GetToolSpec facade wiring,
  product snapshot wrapper adapter injection, on-demand spec discovery Tool
  impl, and unlock-state source in that product runtime owner for now.
  `bitfun-tool-packs` may expose planned
  feature-group scaffold metadata, but it must not own concrete tools yet.
- Keep `ToolUseContext` and concrete tool implementations in core unless a
  reviewed port/provider plan and equivalence tests exist. `ToolContextFacts`
  / `PortableToolContextProvider` are only portable projections; they must not
  carry runtime handles, workspace services, or cancellation tokens.
- Keep `ToolUseContext` runtime/service bindings centralized in
  `src/agentic/tools/tool_context_runtime.rs`. `framework.rs` should define the
  context shape, portable facts projection, and tool trait, not own workspace
  runtime lookup, path enforcement, pipeline/description/preflight context
  materialization, cancellation wrapping, post-call hooks, or checkpoint
  collection.
- Host path normalization, runtime artifact URI parsing/building, and remote
  POSIX path containment are portable `bitfun-agent-tools` contracts. Core
  keeps compatibility wrappers for `BitFunError`, workspace runtime-root
  lookup, and `ToolUseContext` integration.
- Tool allowed-list and collapsed-tool direct execution gating delegate to
  `bitfun-agent-tools`; core still owns the unlock-state source and maps gate
  results into pipeline failure state.
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
  route through `bitfun-product-domains`. Keep Git/AI service adapters,
  provider acquisition, AI client calls, and transport error mapping core-owned;
  prompt templates, JSON extraction/repair, domain error mapping, and domain
  JSON parsing policy may live in `bitfun-product-domains`.
- MiniApp built-in bundle/hash/marker seed-plan and marker wire helpers may
  live in `bitfun-product-domains`. MiniApp create/update/draft/apply pure state
  transitions and built-in seed meta timestamp policy may also live there; keep
  bundled asset includes, filesystem writes, marker IO, customization metadata
  IO, recompile orchestration, worker process runtime, and host dispatch
  execution core-owned until a reviewed migration proves equivalence.
- Remote-connect wire/tracker/dialog and cancel orchestration plus portable
  file/image contracts may live in `bitfun-services-integrations`; keep
  workspace-root source selection, response wrapping, concrete scheduler/session
  restore, terminal pre-warm adapters, and product execution core-owned until a
  reviewed migration proves equivalence. Core service/agent runtime bindings
  are centralized in `src/crates/core/src/service_agent_runtime.rs`.
- Keep concrete remote SSH runtime code behind `ssh-remote`. No-default builds
  may keep workspace identity helpers and explicit unsupported stubs, but must
  not compile russh-backed SSH/SFTP/terminal/search runtime modules.
- Keep no-default `bitfun-core` as a runtime-surface-light facade, not a
  claimed dependency-light build. Full product runtime modules such as agentic,
  MiniApp/function-agent, Git/MCP, remote-connect, review-platform, snapshot,
  token usage, and mode canonicalization stay behind `product-full` or their
  owner feature group.
- Provider-neutral tool path resolution, effective absolute-path checks,
  runtime artifact reference assembly, path policy root matching, and denial
  text may live in `bitfun-agent-tools`; file guidance markers, file-read
  freshness comparison policy, and oversized tool-result preview/rendering
  policy may also live there as pure contracts. Keep workspace/runtime root
  lookup, allowed-root resolution, local canonicalization, remote POSIX
  containment callbacks, session file-read state storage, tool-result
  filesystem writes, `BitFunError` mapping, and `ToolUseContext`
  runtime/service bindings in core unless a separate migration proves
  equivalence.
- Product/runtime dependencies that are only used behind those feature gates
  should stay optional in `bitfun-core` and be enabled by `product-full`,
  `service-integrations`, or `ssh-remote`; do not treat that as permission to
  lighten defaults or change product crate feature sets. Keep
  `scripts/check-core-boundaries.mjs` updated so each optional runtime
  dependency has an explicit feature owner.
- Product entry crates that depend on `bitfun-core` must keep
  `default-features = false` and explicitly enable `product-full`; keep this
  wired through product manifests rather than relying on core defaults. The
  boundary script scans product entry manifests for new direct `bitfun-core`
  dependencies and requires matching assembly rules.
- Keep `default = ["product-full"]` until a separate product matrix review
  explicitly changes default capability selection.
- Keep `bitfun-core/product-full` explicitly wired to the current owner feature
  groups: `ssh-remote`, `product-domains`, `service-integrations`, and
  `tool-packs`.
- Owner crate feature graph guards keep `tool-packs`, `services-integrations`,
  and `product-domains` default-light while allowing `product-full` to
  explicitly aggregate current owner feature groups. When adding an owner
  feature group, update `scripts/check-core-boundaries.mjs`; `product-full`
  must not include undeclared feature groups or dependency shortcuts. Optional
  runtime/domain dependencies in owner crates must stay owned by explicit
  feature groups.
- `service-integrations` is not a standalone product shape in core yet; MCP,
  remote-connect, and review-platform still depend on agentic/product runtime
  owners through `product-full`.
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

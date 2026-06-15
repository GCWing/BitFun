# tool-contracts Agent Guide

Scope: this guide applies to `src/crates/execution/tool-contracts`.

`bitfun-agent-tools` owns portable tool contracts. It must stay independent of
the product tool runtime.

## Guardrails

- Do not depend on `bitfun-core`, concrete service crates,
  `tool-provider-groups`, app crates, Tauri, Git, MCP, network clients, or CLI
  UI dependencies.
- This crate may own provider-neutral tool DTOs, validation/restriction facts,
  path and artifact contracts, pure manifest/catalog/exposure helpers, result
  presentation policy, deterministic admission policy, and portable tool context
  facts.
- This crate may own generic provider contracts, containers, materialization,
  and registry assembly. Concrete tool construction and product runtime
  registration stay outside this crate until a reviewed owner move proves
  behavior equivalence.
- Do not move `ToolUseContext`, concrete tools, workspace services, cancellation
  tokens, session file-read state storage, tool-result filesystem writes,
  state update side effects, snapshot decoration, collapsed unlock state,
  product registry snapshot access, or concrete `GetToolSpecTool` execution
  here without an owner design and equivalence tests.
- Provider-specific wire serialization belongs in AI adapters, not in these
  provider-neutral contracts.

## Verification

```bash
cargo test -p bitfun-agent-tools
cargo test -p bitfun-agent-tools --test tool_contracts
node scripts/check-core-boundaries.mjs
```

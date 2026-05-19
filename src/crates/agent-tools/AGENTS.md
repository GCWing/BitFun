# agent-tools Agent Guide

Scope: this guide applies to `src/crates/agent-tools`.

`bitfun-agent-tools` owns portable tool contracts. It must stay independent of
the product tool runtime.

## Guardrails

- Do not depend on `bitfun-core`, concrete service crates, `tool-packs`, app
  crates, Tauri, Git, MCP, network clients, or CLI UI dependencies.
- This crate may own `ToolResult`, validation DTOs, runtime restriction DTOs,
  path-resolution DTOs, generic/static/dynamic provider contracts, pure
  manifest/exposure helpers, GetToolSpec presentation/schema helpers, and
  `ToolContextFacts` / `PortableToolContextProvider`.
- Do not move `ToolUseContext`, concrete tools, workspace services, cancellation
  tokens, snapshot decoration, collapsed unlock state, runtime manifest
  assembly, or `GetToolSpec` execution here without H1 approval and equivalence
  tests.
- Provider-specific wire serialization belongs in AI adapters, not in these
  provider-neutral contracts.

## Verification

```bash
cargo test -p bitfun-agent-tools
cargo test -p bitfun-agent-tools --test tool_contracts
node scripts/check-core-boundaries.mjs
```

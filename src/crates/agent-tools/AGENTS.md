# agent-tools Agent Guide

Scope: this guide applies to `src/crates/agent-tools`.

`bitfun-agent-tools` owns portable tool contracts. It must stay independent of
the product tool runtime.

## Guardrails

- Do not depend on `bitfun-core`, concrete service crates, `tool-packs`, app
  crates, Tauri, Git, MCP, network clients, or CLI UI dependencies.
- This crate may own `ToolResult`, validation DTOs, runtime restriction DTOs,
  path-resolution DTOs, host path normalization, runtime artifact URI,
  remote POSIX path pure contracts, provider-neutral path resolution /
  absolute-path checks, runtime artifact reference assembly, file guidance
  markers, file-read freshness comparison policy, oversized tool-result
  preview/rendering policy, tool execution result/error/invalid-call presentation policy, deterministic
  tool execution admission policy including loop detection, allowed-list,
  runtime-restriction and collapsed-tool gates, generic/static/dynamic provider contracts, pure
  manifest/exposure helpers, generic contextual prompt-manifest resolver
  contracts, generic catalog snapshot provider contracts, generic GetToolSpec
  catalog provider/detail/summary helpers, provider-backed GetToolSpec runtime
  facades, and `ToolContextFacts` / `PortableToolContextProvider`.
- This crate may own generic provider containers such as
  `StaticToolProviderGroup`, but concrete tool construction and product runtime
  registration stay outside this crate until H1 explicitly moves an owner.
- Do not move `ToolUseContext`, concrete tools, workspace services, cancellation
  tokens, session file-read state storage, tool-result filesystem writes,
  state update side effects, snapshot decoration, collapsed unlock state, product registry
  snapshot access, or concrete `GetToolSpecTool` execution here without H1
  approval and equivalence tests.
- Provider-specific wire serialization belongs in AI adapters, not in these
  provider-neutral contracts.

## Verification

```bash
cargo test -p bitfun-agent-tools
cargo test -p bitfun-agent-tools --test tool_contracts
node scripts/check-core-boundaries.mjs
```

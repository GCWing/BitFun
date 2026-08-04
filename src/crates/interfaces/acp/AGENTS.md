[中文](AGENTS-CN.md) | **English**

# ACP Protocol Surface Guide

Scope: this guide applies to `src/crates/interfaces/acp`.

`bitfun-acp` owns the Agent Client Protocol surface over the assembled product
runtime. Keep ACP protocol/client details here or in app-surface adapters;
share only stable capability facts through contract crates.

The CLI-hosted ACP server consumes `DeliveryProfile::Acp` through
`ProductAssembler` and uses the Agent Runtime SDK for session creation/listing,
active session model/mode updates, dialog submission/cancellation, interaction
responses, and agent event subscription. `bitfun-acp` still depends directly on
the `bitfun-core` `agent-runtime`, `canvas-runtime`, `external-sources`, and
`ssh-remote` owner features for single-pass full persisted-history restore,
model/mode catalog, Canvas tool materialization, provider and compatible
instruction-source reads, MCP provisioning, remote workspace support, and the
ACP client half of this crate. Do not describe the crate as Core-independent
until those production paths have separately proven portable replacements.

## Guardrails

- Remote ACP workspaces reuse local ACP client configuration. Preserve the
  manager, remote shell probing, remote capability store, and workspace menu
  availability semantics when changing ACP client behavior.
- ACP config persistence, remote probing, timeout policy, and workspace surface
  selection are ACP/app-surface behavior. Do not move them into `core-types`,
  `runtime-ports`, or `agent-tools`.
- ACP external-agent tool naming, schema, validation, presentation, and result
  shape are portable contracts owned by `bitfun-agent-tools`; ACP should call
  those helpers instead of redefining them locally.
- Keep ACP stdio/connection ownership and protocol notification projection in
  this crate. Shared runtime facts may cross the SDK boundary; ACP protocol
  requests, client choices, and lifecycle state may not.
- If a future contract is needed, make it observational: environment identity,
  capability facts, and request/response DTOs only.

## Verification

```bash
cargo check -p bitfun-acp
cargo test -p bitfun-acp
```

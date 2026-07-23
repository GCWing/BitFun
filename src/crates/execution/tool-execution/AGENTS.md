# tool-execution Agent Guide

Scope: this guide applies to `src/crates/execution/tool-execution`.

`tool-runtime` owns low-level reusable tool execution helpers such as filesystem
and search utilities, provider-neutral pipeline planning/retry/token policy,
ExecCommand presentation/control facts, Computer Use loop/retry policies,
prompt-safe tool context facts/custom-data materialization and extension merge,
background exec-output capture state, and provider-neutral Web tool result
processing. It is not the product tool registry, permission model, or
agent-facing tool surface.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, product-domain crates,
  transport adapters, or AI providers.
- Keep this crate focused on reusable execution primitives and pure utilities.
  Product-specific tool exposure, prompt-visible manifests, `GetToolSpec`,
  collapsed unlock state, concrete runtime handles, and the `ToolUseContext`
  owner type stay outside this crate.
- Preserve existing filesystem/search/Web tool behavior when moving helpers
  here. Do not change path containment, encoding, cancellation, extraction, or
  result presentation semantics as a side effect of refactoring.
- Background exec-output and ExecCommand presentation helpers may own retained
  output buffers, cursors, lifecycle metadata, assistant response text, and
  provider-neutral completion shapes; concrete local/remote process managers
  stay in services or core adapters.
- Computer Use helpers here may own provider-neutral loop detection, screenshot
  hash, verification, and retry policy. Host APIs, permissions, captures, OCR,
  accessibility, and OS input remain in host adapters.
- Provider-neutral contracts belong in `tool-contracts` (`bitfun-agent-tools`);
  product provider grouping belongs in `tool-provider-groups`
  (`bitfun-tool-packs`).

## Verification

```bash
cargo test -p tool-runtime
cargo test -p tool-runtime --features web-readable web
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

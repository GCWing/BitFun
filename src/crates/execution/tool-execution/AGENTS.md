# tool-execution Agent Guide

Scope: this guide applies to `src/crates/execution/tool-execution`.

`tool-runtime` owns low-level reusable tool execution helpers such as filesystem
and search utilities. It is not the product tool registry, permission model, or
agent-facing tool surface.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, product-domain crates,
  transport adapters, or AI providers.
- Keep this crate focused on reusable execution primitives and pure utilities.
  Product-specific tool exposure, prompt-visible manifests, `GetToolSpec`,
  collapsed unlock state, and `ToolUseContext` stay outside this crate.
- Preserve existing filesystem/search behavior when moving helpers here. Do not
  change path containment, encoding, cancellation, or result presentation
  semantics as a side effect of refactoring.
- Provider-neutral contracts belong in `tool-contracts` (`bitfun-agent-tools`);
  product provider grouping belongs in `tool-provider-groups`
  (`bitfun-tool-packs`).

## Verification

```bash
cargo test -p tool-runtime
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

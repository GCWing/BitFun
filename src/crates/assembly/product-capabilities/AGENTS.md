# product-capabilities Agent Guide

Scope: this guide applies to `src/crates/assembly/product-capabilities`.

`bitfun-product-capabilities` owns product capability pack assembly facts: which
runtime services, feature groups, tool provider group ids, harness provider
descriptors, profile-scoped harness registries, and runtime service
availability wrappers a product capability selects. It does not own concrete
runtime execution.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, product-domain
  implementations, concrete service crates, AI adapters, transport adapters,
  terminal, tool-runtime, or concrete tool implementations.
- Keep this crate limited to stable capability ids, feature group facts, service
  capability facts, runtime service availability checks, tool provider group id
  selection, and harness provider descriptor selection.
- Do not encode product UI behavior, permission decisions, session lifecycle,
  filesystem/process IO, Git/AI provider acquisition, or feature defaults here.
- Preserve default product tool provider order and legacy harness provider ids
  when changing capability packs.

## Verification

```bash
cargo test -p bitfun-product-capabilities
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

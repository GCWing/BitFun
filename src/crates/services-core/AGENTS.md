# services-core Agent Guide

Scope: this guide applies to `src/crates/services-core`.

`bitfun-services-core` owns platform-neutral service DTOs and helpers that can
compile without the full product runtime.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, tool runtime, or product
  runtime crates.
- Prefer `bitfun-core-types` for shared DTOs and `bitfun-runtime-ports` for
  cross-layer traits.
- Keep the default feature lightweight; feature groups such as search, LSP,
  cron, or snapshot should not become new crates until measured compile cost
  proves the split is needed.
- Runtime call sites that touch agent execution, scheduler state, workspace
  managers, filesystem orchestration, or product behavior stay in core until a
  reviewed port/provider design and equivalence tests exist.
- Preserve legacy core imports with facade/re-export code when ownership moves.

## Verification

```bash
cargo test -p bitfun-services-core
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```

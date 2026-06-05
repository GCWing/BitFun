# api-layer Agent Guide

Scope: this guide applies to `src/crates/adapters/api-layer`.

`bitfun-api-layer` owns platform-agnostic API DTOs and handler coordination. It
is the stable boundary between app entrypoints and lower transport/runtime
layers.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, desktop-only adapters,
  concrete services, AI providers, terminal, or tool-runtime implementations.
- Keep request/response DTOs stable and explicit. Avoid catch-all payloads that
  hide product or platform coupling.
- Handlers may coordinate ports and transport surfaces; they must not own
  product policy, tool execution, session lifecycle, filesystem/process IO, or
  platform-specific behavior.
- Preserve command/API compatibility when renaming fields, routes, or response
  shapes.

## Verification

```bash
cargo check -p bitfun-api-layer
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

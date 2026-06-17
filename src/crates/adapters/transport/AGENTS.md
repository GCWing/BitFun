# transport Agent Guide

Scope: this guide applies to `src/crates/adapters/transport`.

`bitfun-transport` owns cross-platform communication contracts and adapters. It
bridges product/API events to concrete delivery channels without owning product
logic.

## Guardrails

- Do not depend on `bitfun-core`, API handlers, app crates, product domains,
  concrete services, AI providers, terminal, or tool-runtime implementations.
- Keep adapter features explicit. Tauri, CLI, and websocket adapters must remain
  feature-gated and must not change the default build surface.
- Transport may serialize and deliver events; it must not decide product policy,
  session lifecycle, tool exposure, permissions, or remote workspace behavior.
- Preserve event names, payload compatibility, ordering assumptions, and
  backpressure/error semantics when refactoring adapters.

## Verification

```bash
cargo check -p bitfun-transport
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

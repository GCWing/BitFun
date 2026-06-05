# runtime-services Agent Guide

Scope: this guide applies to `src/crates/execution/runtime-services`.

`bitfun-runtime-services` owns typed runtime service assembly. It connects
runtime-facing ports to injected providers without becoming a concrete platform
implementation layer.

## Guardrails

- Depend on `bitfun-runtime-ports`; avoid dependencies on `bitfun-core`, app
  crates, Tauri, concrete desktop adapters, or product UI.
- Builders should assemble explicit typed service bundles and capability
  availability. Do not introduce untyped maps, global mutable registries, or
  implicit service lookup.
- Fake/test providers may live here only when they protect port behavior without
  pulling product runtime dependencies.
- Concrete filesystem, terminal, network, Git, MCP, remote, or AI provider
  implementations belong in provider crates or app adapters.

## Verification

```bash
cargo test -p bitfun-runtime-services
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

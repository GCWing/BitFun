# runtime-ports Agent Guide

Scope: this guide applies to `src/crates/runtime-ports`.

`bitfun-runtime-ports` owns stable runtime-facing ports, DTOs, and capability
facts. It is an interface crate, not a runtime implementation crate.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, concrete service crates,
  AI adapters, transport adapters, or tool implementations.
- Keep ports narrow and typed. Avoid untyped service locators, global registries,
  or catch-all context structs.
- This crate may define portable request/response DTOs, runtime handles,
  capability facts, cancellation surfaces, and service traits.
- Do not put filesystem writes, process execution, network clients, Git/AI/MCP
  concrete behavior, product policy, or UI command logic here.
- Preserve serialization compatibility for persisted or cross-process DTOs.

## Verification

```bash
cargo test -p bitfun-runtime-ports
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

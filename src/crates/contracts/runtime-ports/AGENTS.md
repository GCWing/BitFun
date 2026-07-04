# runtime-ports Agent Guide

Scope: this guide applies to `src/crates/contracts/runtime-ports`.

`bitfun-runtime-ports` owns stable runtime-facing ports, DTOs, and capability
facts. It is an interface crate, not a runtime implementation crate.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, concrete service crates,
  AI adapters, transport adapters, or tool implementations.
- Keep ports narrow and typed. Avoid untyped service locators, global registries,
  or catch-all context structs.
- This crate may define portable request/response DTOs, runtime handles,
  capability facts, cancellation surfaces, and service traits.
- `RemoteExecPort` owns only remote command/stdin/control DTOs, bounded
  one-shot command results, and lifecycle event shapes; SSH managers, channels,
  process storage, and workspace lookup do not belong here.
- `SessionStorePort` owns typed session storage-path resolution plus restore /
  load request and timing facts only. Concrete session persistence, file IO,
  session lifecycle, context restore, and prompt assembly do not belong here.
- Do not put filesystem writes, process execution, network clients, Git/AI/MCP
  concrete behavior, product policy, permission decisions, audit outcomes, UI
  extension behavior, UI implementation, or UI command logic here.
- Preserve serialization compatibility for persisted or cross-process DTOs.

## Verification

```bash
cargo test -p bitfun-runtime-ports
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

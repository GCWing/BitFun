# plugin-runtime-client Agent Guide

Scope: this guide applies to `src/crates/execution/plugin-runtime-client`.

`openbitfun-plugin-runtime-client` is the default implementation of the existing,
portable `PluginRuntimeClient` port. The current implementation validates request
identity and responses, serializes the same logical plugin instance, applies
deadlines, caches responses by `idempotency_key`, and returns explicit diagnostics
and fault status around the injected adapter port. Cancellation invalidation,
bounded queues, and rejection of requests or results from a replaced Host connection remain
requirements for a future executable Plugin Host path; do not claim them as
implemented until the port and focused tests exist.
It is not a Host: it runs in the Rust application process, does not execute
JS/TS plugins, does not own concrete ecosystem behavior, and does not hold OS
process handles or process trees. The Plugin Host is the supervised JS/TS child
process on the other side of the injected ports.

## Guardrails

- Depend only on stable contracts such as `openbitfun-runtime-ports`.
- Do not depend on `openbitfun-core`, product assembly, app crates, Tauri, concrete
  services, concrete adapters, `openbitfun-opencode-adapter`, or UI code.
- Physical Plugin Host health, Job Objects/process groups, resource budgets, and
  process-tree termination belong to the services implementation behind the existing
  `ScriptToolRuntime` boundary.
  This client may request and consume those facts through a stable port. The
  current client stores only disposed logical project/workspace identities,
  per-plugin dispatch locks, bounded duplicate-request results, and internal fault
  diagnostics that can pause later dispatches. A future executable Host path must
  bind pending requests to the connection that sent them and reject results from
  replaced connections; do not claim this until the port and focused tests exist.
  Plugin identity/version and contribution state stay with the existing source or
  capability owners; do not introduce a parallel plugin-lifecycle object or version counter.
- A workspace id may key logical source, policy, fault state, and same-instance
  dispatch ordering. It must not become a physical process, capacity, or restart
  key. Do not infer a Plugin Host fault domain from a client instance; future
  process recovery must consume the failed connection from its services module, resolve
  affected pending requests, ignore events from replaced connections, and retain successful
  `idempotency_key` results.
- Client responses must return explicit status and source views, provider
  candidates, diagnostics, or fault facts; never write permission decisions,
  audit success, tool results, kernel state, or UI implementation state.
- Adapter failures, deadline expiry, and disposed projects must return explicit
  diagnostics or `NotAvailable` errors and must not report effects as applied.
- Keep the public API budget small. New public symbols require a responsible module,
  current consumer, P0 OpenCode-compatible trace relation, and boundary rule.

## Verification

```bash
cargo test -p openbitfun-plugin-runtime-client
node scripts/check-core-boundaries.mjs
```

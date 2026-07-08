# plugin-runtime-host Agent Guide

Scope: this guide applies to `src/crates/execution/plugin-runtime-host`.

`bitfun-plugin-runtime-host` owns the minimal, portable Plugin Runtime Host
boundary. It validates dispatch lifecycle facts, idempotency, typed diagnostics,
deadline handling, failure quarantine, and host-owned read-model projection
around an injected adapter. It does
not execute JS/TS plugins and does not own concrete ecosystem adapter behavior.

## Guardrails

- Depend only on stable contracts such as `bitfun-runtime-ports`.
- Do not depend on `bitfun-core`, product assembly, app crates, Tauri, concrete
  services, concrete adapters, `bitfun-opencode-adapter`, or UI code.
- Host responses must return typed status/source projections, provider
  candidates, diagnostics, or quarantine facts; never write permission decisions,
  audit success, tool results, kernel state, or UI implementation state.
- Adapter failures, deadline expiry, and disposed projects must fail closed with
  typed diagnostics or `NotAvailable` errors and must not pretend to materialize
  effects.
- Keep the public API budget small. New public symbols require an owner,
  current consumer, P0 OpenCode-compatible trace relation, and boundary rule.

## Verification

```bash
cargo test -p bitfun-plugin-runtime-host
node scripts/check-core-boundaries.mjs
```

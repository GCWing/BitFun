# plugin-runtime-host Agent Guide

Scope: this guide applies to `src/crates/execution/plugin-runtime-host`.

`bitfun-plugin-runtime-host` owns the minimal, portable Plugin Runtime Host
boundary. It validates request identity, late-response rejection, typed
diagnostics, deadline handling, failure quarantine, logical target state, and
user-facing status views around injected adapter and execution-control ports. It
does not execute JS/TS plugins, own concrete ecosystem behavior, or hold OS
process handles and process trees.

## Guardrails

- Depend only on stable contracts such as `bitfun-runtime-ports`.
- Do not depend on `bitfun-core`, product assembly, app crates, Tauri, concrete
  services, concrete adapters, `bitfun-opencode-adapter`, or UI code.
- Physical worker health, Job Objects/process groups, resource budgets, and
  process-tree termination belong to the injected script-execution service.
  Host may request and consume those facts through a stable port, but stores only
  logical target, in-flight call, contribution, and quarantine state.
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

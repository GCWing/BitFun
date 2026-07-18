# transport Agent Guide

Scope: this guide applies to `src/crates/adapters/transport`.

`bitfun-transport` owns the event delivery abstraction and adapters used by
current product hosts. It bridges owned event projections to concrete delivery
channels without owning product logic or future protocol plans.

## Guardrails

- Do not depend on `bitfun-core`, API handlers, app crates, product domains,
  concrete services, AI providers, terminal, or tool-runtime implementations.
- Keep host adapter features explicit. Retaining a production adapter requires
  a production construction point, a current consumer, and host lifecycle
  ownership. When adding an adapter or changing delivery semantics, add a
  focused delivery test or the nearest host integration test. A short-lived
  pre-integration seam may remain internal only when adjacent design names its
  first consumer, stable contract, integration check, and removal condition; a
  platform plan alone is not enough.
- Transport may serialize and deliver events; it must not decide product policy,
  session lifecycle, tool exposure, permissions, or remote workspace behavior.
- Preserve event names, payload compatibility, ordering assumptions, and
  backpressure/error semantics when refactoring adapters.
- Keep protocol routes and frontend clients in their owning app. Their existence
  does not justify an unused transport adapter with a similar name.

## Verification

```bash
cargo check -p bitfun-transport
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

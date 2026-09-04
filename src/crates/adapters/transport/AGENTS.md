# transport Agent Guide

Scope: this guide applies to `src/crates/adapters/transport`.

`openbitfun-transport` owns the event delivery abstraction and the smallest
protocol-neutral message mechanics reused by current product hosts. It bridges
owned event projections to concrete delivery channels and caps JSON encoding
without owning product logic or protocol plans. Its colocated TypeScript core
owns carrier-neutral message and JSON-RPC correlation mechanics reused by SDK
stdio and the WebSocket client.

## Guardrails

- Do not depend on `openbitfun-core`, API handlers, app crates, product domains,
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
- Shared JSON and message helpers must remain protocol-neutral. Host wire
  shapes, routes, authentication, lifecycle, framing choice, and concrete
  size-limit policy stay in the owning protocol adapter or app. Do not extract
  a framing helper until at least two current consumers share its semantics.
- Preserve event names, payload compatibility, ordering assumptions, and
  backpressure/error semantics when refactoring adapters.
- Keep protocol routes and frontend clients in their owning app. Their existence
  does not justify an unused transport adapter with a similar name.

## Verification

```bash
cargo check -p openbitfun-transport
pnpm --dir sdk/typescript test
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

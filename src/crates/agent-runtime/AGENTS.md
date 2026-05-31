# agent-runtime Agent Guide

Scope: this guide applies to `src/crates/agent-runtime`.

`bitfun-agent-runtime` owns portable agent runtime decisions that can be built
and tested without `bitfun-core`.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, ACP protocol, web UI,
  concrete service crates, or product-domain implementations.
- Keep concrete scheduler/session lifecycle execution in `bitfun-core` until a
  reviewed owner migration proves behavior equivalence.
- Prefer pure facts and decisions first: queue policy, background delivery,
  cancellation routing, runtime event facts, and registry visibility.
- Add focused tests before moving any runtime decision into this crate.

## Verification

```bash
cargo test -p bitfun-agent-runtime
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```

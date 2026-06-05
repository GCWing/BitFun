# harness Agent Guide

Scope: this guide applies to `src/crates/execution/harness`.

`bitfun-harness` owns provider-neutral workflow contracts, descriptors, plans,
and registry wiring for multi-step workflows such as Deep Review,
DeepResearch, MiniApp, and future SDD flows.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, concrete service crates,
  product-domain implementations, AI adapters, transport adapters, or concrete
  tool packs.
- Keep concrete workflow execution on the legacy product path until a reviewed
  migration proves behavior equivalence.
- Harness providers may describe routing, planning, capability, review gate,
  artifact, and post-processing boundaries. They must not own session manager
  internals, filesystem/Git/terminal managers, or UI command behavior.
- Product Assembly should register providers through typed registries; avoid
  global mutable registries or untyped service locators.
- Product capability packs may select provider descriptors; `bitfun-harness`
  owns the provider-neutral descriptor type, legacy-facade descriptor adapter,
  and registry wiring.

## Verification

```bash
cargo test -p bitfun-harness
node scripts/check-core-boundaries.mjs
cargo test -p bitfun-core --features product-full product_harness
```

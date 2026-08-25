# product-capabilities Agent Guide

Scope: this guide applies to `src/crates/assembly/product-capabilities`.

`bitfun-product-capabilities` owns product capability pack assembly facts: which
delivery profiles, runtime services, feature groups, tool provider group ids,
profile-scoped built-in Agent ids, plugin availability, and runtime
service availability wrappers a product capability selects. It does not own
concrete runtime execution.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, product-domain
  implementations, concrete service crates, AI adapters, transport adapters,
  terminal, tool-runtime, or concrete tool implementations.
- Keep this crate limited to stable delivery profile facts, capability ids,
  feature group facts, service capability facts, runtime service availability
  checks, built-in Agent id selection, tool provider group id selection, and
  plugin availability facts.
- `ProductToolPlan` is the assembly-owned authority for the exact tool feature
  owners requested by one runtime. Provider groups preserve atomic ownership;
  Core materialization separately preserves the observable registry order.
  They are not feature unions. The Agent Runtime baseline plan selects only
  `core.basic`, `core.agent`, and `core.session`, while delivery profiles
  select their reviewed product plan explicitly.
- Delivery profiles select built-in Agent ids and atomic tool provider groups
  together. CLI, ACP, and SDK currently select only the Code Agent capability;
  product workflow names must not leak in through a compiled Cargo feature.
- `ProductAssembler` may validate explicit profile input and return immutable
  runtime parts; it must not create concrete services or product state.
- `ProductCoreDependencyMode::ExplicitCoreCapabilityClosure` records that an
  entrypoint selects reviewed Cargo owner capabilities; it is not a feature
  list, runtime availability result, or permission to introduce a profile-named
  umbrella feature.
- Do not encode product UI behavior, permission decisions, session lifecycle,
  filesystem/process IO, Git/AI provider acquisition, or feature defaults here.
- Preserve atomic provider ids and tool ownership when changing capability
  packs; do not make capability-pack order redefine runtime tool order.

## Verification

```bash
cargo test -p bitfun-product-capabilities
node scripts/check-core-boundaries.mjs
```

For documentation-only changes, run `git diff --check`.

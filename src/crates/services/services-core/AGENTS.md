# services-core Agent Guide

Scope: this guide applies to `src/crates/services/services-core`.

`bitfun-services-core` owns platform-neutral service DTOs and helpers that can
compile without the full product runtime. It also owns generic local filesystem
operations/tree/search/listing primitives, LSP plugin registry and command-target
mapping rules, session storage layout helpers, turn file indexing/deletion,
metadata store CRUD/index rebuild, metadata construction/counter/index/field
mutation rules, lineage/branch metadata shaping, and reusable JSON file IO;
product crates may layer remote workspace routing or legacy error mapping outside
this crate.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, tool runtime, or product
  runtime crates.
- Prefer `bitfun-core-types` for shared DTOs and `bitfun-runtime-ports` for
  cross-layer traits.
- Keep the default feature lightweight; feature groups such as search, LSP,
  cron, or snapshot should not become new crates until measured compile cost
  proves the split is needed.
- LSP manifest and protocol DTOs belong in `bitfun-core-types`; plugin package
  filesystem IO and process lifecycle stay in the core compatibility adapter or
  a reviewed concrete service owner.
- Runtime call sites that touch agent execution, scheduler state, workspace
  managers, filesystem orchestration, or product behavior stay in core until a
  reviewed port/provider design and equivalence tests exist.
- Do not add remote SSH, MiniApp storage, tool-result persistence, `PathManager`
  globals, or product runtime bindings to `filesystem`; keep those in core or a
  reviewed adapter/provider.
- Preserve legacy core imports with facade/re-export code when ownership moves.

## Verification

```bash
cargo test -p bitfun-services-core
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```

# services-core Agent Guide

Scope: this guide applies to `src/crates/services/services-core`.

`bitfun-services-core` owns cross-platform service DTOs and helpers that compile
without the full product runtime. This includes generic filesystem/search/JSON
IO helpers, LSP package/protocol/watch/process primitives, session metadata
storage helpers, and local OS action primitives such as command lookup,
clipboard, file/url opening, script execution, and system facts. Product crates
may layer routing, policy, capability selection, event emission, or legacy error
mapping outside this crate.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, tool runtime, or product
  runtime crates.
- Prefer `bitfun-core-types` for shared DTOs and `bitfun-runtime-ports` for
  cross-layer traits.
- Keep dependency features explicit. Non-LSP consumers should use
  `default-features = false`; LSP consumers must enable the `lsp` feature.
- LSP manifest and protocol DTOs belong in `bitfun-core-types`; reusable LSP
  package, protocol, detection, debounce, watch, and process-manager helpers
  belong in `services-core`; product workspace state, event emission, global
  singletons, and file-sync orchestration stay outside this crate.
- Runtime call sites that touch agent execution, scheduler state, workspace
  managers, filesystem orchestration, or product behavior stay in core until a
  reviewed port/provider design and equivalence tests exist.
- Do not add remote SSH, MiniApp storage, tool-result persistence, `PathManager`
  globals, or product runtime bindings to `filesystem`; keep those in core or a
  reviewed adapter/provider.
- Preserve legacy core imports with facade/re-export code when ownership moves.

## Verification

```bash
cargo test -p bitfun-services-core --features lsp
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```

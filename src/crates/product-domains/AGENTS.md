[中文](AGENTS-CN.md) | **English**

# Product Domains Agent Guide

Scope: this guide applies to `src/crates/product-domains`.

`bitfun-product-domains` owns low-risk product-domain contracts that can compile
without the full core runtime. Keep this crate behavior-preserving and
platform-agnostic; `bitfun-core` may keep compatibility facades while ownership
moves here gradually.

## Guardrails

- Do not add a dependency from `bitfun-product-domains` to `bitfun-core`.
- Keep the default feature lightweight. Default builds should not pull runtime,
  service, desktop, network, process, AI, or tool-runtime dependencies.
- This crate may own pure DTOs, enums, serialization contracts, search plans,
  command-selection decisions, host-routing string rules, storage-shape parsers,
  draft/metadata response shapes, small helpers, and file-shape analyzers that
  use `std` or feature-gated lightweight dependencies only.
- This crate may define product-domain port traits for future runtime migration,
  but concrete adapters that perform IO, process execution, AI calls, Git
  service calls, or platform integration still belong outside this crate.
- Do not move runtime execution, filesystem writes, shell/network behavior,
  config/path managers, AI clients, Git service behavior, tool manifests,
  `ToolUseContext`, tool exposure, or desktop/Tauri adapters here without an
  explicit review, a port/provider design, and equivalence tests.
- Preserve existing core import paths with re-export or wrapper facades until
  downstream call sites are intentionally migrated.
- Feature-gated additions must remain narrow. `miniapp` may use MiniApp-only
  dependencies, `function-agents` may use function-agent-only dependencies, and
  `product-full` should only aggregate existing product-domain feature groups.

## Current owners

- `miniapp` owns MiniApp DTOs, compiler/bridge helpers, storage/draft/import
  file shapes, import fallback payloads, runtime search-plan helpers, worker
  install command selection, lifecycle/revision and manager state-transition
  helpers, host-routing string policy, customization metadata policy including
  built-in update/decline decisions, built-in bundle/hash/marker seed plan and
  marker wire helpers, built-in source/placeholder payload contracts, port
  traits, and storage-backed runtime state facade logic.
- `function-agents` owns pure function-agent DTOs, prompt templates and
  assembly helpers, commit prompt preparation, AI-response JSON extraction and
  domain error-mapping policy, diff truncation policy, JSON-string-to-domain
  parsing helpers, local file-shape analysis, Git/AI port traits, and
  port-backed runtime facade orchestration, including the commit-message and
  Startchat work-state facades used by core adapters.
- Core still owns MiniApp filesystem IO, worker process execution, host dispatch
  execution, built-in asset includes/seeding, marker IO, recompile orchestration,
  source-hash input lookup, `PathManager` integration, function-agent Git/AI
  service adapters, AI client calls, provider acquisition, and AI transport
  error mapping.

## Verification

Use the smallest matching check for the changed surface:

```bash
cargo test -p bitfun-product-domains --no-default-features
cargo test -p bitfun-product-domains --features product-full
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```

For documentation-only changes, also run `git diff --check`.

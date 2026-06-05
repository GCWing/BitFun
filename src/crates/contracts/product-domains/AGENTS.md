[中文](AGENTS-CN.md) | **English**

# Product Domains Agent Guide

Scope: this guide applies to `src/crates/contracts/product-domains`.

`bitfun-product-domains` owns platform-agnostic product-domain contracts that can
compile without the full core runtime. Keep it focused on pure state, DTOs,
policies, and narrow ports; concrete runtime behavior belongs outside this crate.

## Guardrails

- Do not add a dependency from `bitfun-product-domains` to `bitfun-core`.
- Keep the default feature lightweight. Default builds must not pull runtime,
  service, desktop, network, process, AI, or tool-runtime dependencies.
- This crate may own pure DTOs, enums, serialization contracts, search plans,
  command-selection decisions, storage-shape parsers, domain policies, and
  product-domain port traits.
- Concrete adapters that perform IO, process execution, AI calls, Git service
  calls, platform integration, tool exposure, or desktop/Tauri work belong
  outside this crate.
- Preserve existing core import paths with re-export or wrapper facades until
  downstream call sites are intentionally migrated.
- Feature-gated additions must remain narrow. `miniapp`, `function-agents`, and
  `product-full` should only enable their declared product-domain feature groups.

## Ownership Boundary

- `miniapp` may own MiniApp data shapes, pure lifecycle decisions, metadata and
  import policies, built-in bundle identity, embedded source assets, seed-plan
  facts, marker wire formats, host primitive call plans, and narrow ports.
- `function-agents` may own function-agent DTOs, prompt/domain policies,
  response parsing and repair rules, file-shape analysis, and Git/AI port traits.
- Core still owns filesystem writes, marker IO, worker/host side effects,
  compile orchestration, `PathManager` integration, concrete Git/AI services,
  provider acquisition, and transport error mapping.

## Verification

Use the smallest matching check for the changed surface:

```bash
cargo test -p bitfun-product-domains --no-default-features
cargo test -p bitfun-product-domains --features product-full
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```

For documentation-only changes, run `git diff --check`.

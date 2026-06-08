# services-integrations Agent Guide

Scope: this guide applies to `src/crates/services/services-integrations`.

`bitfun-services-integrations` owns reviewed integration contracts and runtime
slices that are outside pure product logic but still platform-neutral.

## Guardrails

- Do not depend on `bitfun-core`, app crates, desktop adapters, CLI UI, or web
  presentation code.
- Keep integration families behind explicit features. The default feature set
  should not compile heavy Git, MCP, SSH, network, or file-watch runtimes.
  Boundary checks enforce `default = []` and the current `product-full`
  integration feature-group list.
- MCP config/process/transport lifecycle and dynamic provider helpers may live
  here; product tool registry assembly, manifest filtering, `GetToolSpec`
  execution, and concrete tool behavior remain outside this crate unless a
  reviewed owner move proves behavior equivalence.
- Remote-connect contracts, dialog/cancel orchestration ports, image-context
  adapter contracts, remote workspace helpers, and command/response assembly
  may live here when they stay platform-neutral.
- Remote workspace facts, session metadata, file projection DTOs, and
  workspace/projection host traits belong in `bitfun-runtime-ports`.
- Workspace-root source selection, persistence/workspace service reads,
  concrete scheduler/session restore, terminal pre-warm adapters, and product
  execution remain core-owned unless a reviewed port/provider moves them with
  equivalence tests.
- Remote-SSH path/session identity helpers may live here; SSH channels, SFTP,
  remote FS, remote terminal, and manager assembly remain core-owned unless a
  reviewed port/provider move proves equivalence.
- Workspace search owns the local flashgrep daemon/session lifecycle and
  indexed-search result conversion behind `workspace-search`; product config
  and workspace bootstrap stay in the core facade as injected hooks.
- Remote SSH workspace-search owns path/scope/probe/bundle/retry strategy plus
  flashgrep session/context lifecycle behind a provider boundary. SSH channels
  and manager assembly remain outside this crate until a reviewed provider
  boundary moves them with equivalence tests.

## Verification

```bash
cargo test -p bitfun-services-integrations
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```

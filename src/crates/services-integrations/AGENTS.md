# services-integrations Agent Guide

Scope: this guide applies to `src/crates/services-integrations`.

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
  execution, and concrete tool behavior remain core-owned until H1.
- Remote-connect tracker/wire/pure-policy contracts, dialog submission and
  cancel-task orchestration ports, image-context adapter contracts, and portable
  workspace-file path/read/chunk/info helpers may live here. Workspace-root
  source selection, response/base64 wrapping, concrete scheduler submission,
  concrete session restore / terminal pre-warm adapters, and product execution
  remain core-owned unless a later reviewed port/provider moves them with
  equivalence tests. Core bindings for these runtime adapters are centralized in
  `src/crates/core/src/service_agent_runtime.rs`.
- Remote-SSH path/session identity helpers may live here; SSH channels, SFTP,
  remote FS, remote terminal, and manager assembly remain core-owned unless a
  later reviewed port/provider migration proves equivalence.

## Verification

```bash
cargo test -p bitfun-services-integrations
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```

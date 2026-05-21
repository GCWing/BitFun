# services-integrations Agent Guide

Scope: this guide applies to `src/crates/services-integrations`.

`bitfun-services-integrations` owns reviewed integration contracts and runtime
slices that are outside pure product logic but still platform-neutral.

## Guardrails

- Do not depend on `bitfun-core`, app crates, desktop adapters, CLI UI, or web
  presentation code.
- Keep integration families behind explicit features. The default feature set
  should not compile heavy Git, MCP, SSH, network, or file-watch runtimes.
- MCP config/process/transport lifecycle and dynamic provider helpers may live
  here; product tool registry assembly, manifest filtering, `GetToolSpec`
  execution, and concrete tool behavior remain core-owned until H1.
- Remote-connect tracker/wire/pure-policy contracts may live here; dialog
  submission, file IO/path resolution, terminal pre-warm, and product execution
  remain core-owned until H3.
- Remote-SSH path/session identity helpers may live here; SSH channels, SFTP,
  remote FS, remote terminal, and manager assembly remain core-owned until H3.

## Verification

```bash
cargo test -p bitfun-services-integrations
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```

[中文](AGENTS-CN.md) | **English**

# OpenCode Adapter

This crate owns projection-only OpenCode-compatible source discovery. It
validates OpenCode import shapes such as `opencode.json` and
`.opencode/plugins/*.js|ts`, then exposes the resulting source facts through a
narrow Plugin Runtime Host adapter. It must not own product policy, host
lifecycle, sandboxing, UI implementation, or effect materialization.

Product-source boundary:

- BitFun plugin package/install sources are the production entry point for
  plugin loading. OpenCode config is an optional compatibility import source,
  not the primary plugin registry or runtime state.
- Importing `opencode.json`, `.opencode/plugins/*.js|ts`, or future OpenCode
  global plugin directories must produce typed import facts, candidate BitFun
  plugin source records, manifests, hashes, diagnostics, and trust state before
  a product source loader can enable or execute anything.
- The user's local `opencode` CLI installation is unrelated to loading
  OpenCode-compatible plugins. CLI/server interop with an installed OpenCode
  binary belongs to ACP/external-client work, not this adapter boundary.

## Boundary Rules

- Depend on stable contracts such as `bitfun-runtime-ports` and the
  `PluginHostAdapter` boundary trait, not `bitfun-core`, app crates, Tauri
  APIs, product UI, or concrete service managers.
- Keep OpenCode config JSON import and workspace plugin import parsing inside
  this crate. Cross-crate outputs must go through `load_opencode_workspace_adapter`
  and Plugin Runtime Host DTOs; do not expose raw OpenCode JSON or source
  syntax as product contracts.
- Unsupported OpenCode capabilities must be explicit diagnostics or typed
  unsupported candidates. Do not silently ignore them.
- The public API budget is limited to `load_opencode_workspace_adapter`. New
  public symbols require an updated public API budget, current consumer, and
  focused host-path tests.
- This crate may provide private OpenCode compatibility import projectors and
  contract fixtures for adapter validation, but it must not implement
  `PluginRuntimeClient`, declare executable availability, or become the runtime
  host. Product Assembly decides host binding through the reviewed Plugin
  Runtime Host path.
- Production crates must not import `bitfun_opencode_adapter` directly until a
  reviewed product source loader wires it through the Plugin Runtime Host
  boundary.

## Verification

- `cargo test -p bitfun-opencode-adapter --test opencode_source_adapter`
- `cargo test -p bitfun-opencode-adapter p0_c2_fixture`
- `node scripts/check-core-boundaries.mjs`

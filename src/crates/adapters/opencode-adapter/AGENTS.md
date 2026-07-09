[中文](AGENTS-CN.md) | **English**

# OpenCode Adapter

This crate owns fixture-only OpenCode-compatible import projection contracts.
It validates OpenCode import shapes such as `opencode.json`,
`.opencode/plugins/*.js|ts`, and OpenCode global plugin directories, then
projects them into BitFun plugin runtime contracts for tests. It must not own
product policy, host lifecycle, sandboxing, UI implementation, or effect
materialization.

Product-source boundary:

- BitFun plugin package/install sources are the production entry point for
  plugin loading. OpenCode config is an optional compatibility import source,
  not the primary plugin registry or runtime state.
- Importing `opencode.json`, `.opencode/plugins/*.js|ts`, or OpenCode global
  plugin directories must produce typed import facts, candidate BitFun plugin
  source records, manifests, hashes, diagnostics, and trust state before a
  production consumer can use them.
- The user's local `opencode` CLI installation is unrelated to loading
  OpenCode-compatible plugins. CLI/server interop with an installed OpenCode
  binary belongs to ACP/external-client work, not this adapter boundary.

## Boundary Rules

- Depend on stable contracts such as `bitfun-runtime-ports`, not `bitfun-core`,
  app crates, Tauri APIs, product UI, or concrete service managers.
- Keep OpenCode config JSON import, workspace plugin import, and global plugin
  import parsing inside fixture tests in this crate. Cross-crate outputs must be typed
  `PluginRuntimeReadResponse`, `PluginResponseEnvelope`, diagnostics,
  permission prompts, and effect candidates once a reviewed production consumer
  exists.
- Unsupported OpenCode capabilities must be explicit diagnostics or typed
  unsupported candidates. Do not silently ignore them.
- Current public API budget is empty. This crate owns fixture-scoped projection
  tests only until a reviewed Plugin Runtime Host integration introduces a real
  consumer.
- This crate may provide private OpenCode compatibility import projectors and
  contract fixtures for adapter validation, but it must not implement
  `PluginRuntimeClient`, declare executable availability, or become the runtime
  host. Product Assembly decides host binding through the reviewed Plugin
  Runtime Host path.
- Production crates must not import `bitfun_opencode_adapter` directly until the
  host integration PR removes the temporary boundary guard with a reviewed
  consumer path.

## Verification

- `cargo test -p bitfun-opencode-adapter opencode_fixture_contracts`
- `node scripts/check-core-boundaries.mjs`

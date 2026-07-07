[中文](AGENTS-CN.md) | **English**

# OpenCode Adapter

This crate owns fixture-only OpenCode source projection contracts. It translates
real OpenCode config entries and local `.opencode/plugins/*.js|ts` source facts
into BitFun plugin runtime contracts for validation; it must not own product
policy, host lifecycle, sandboxing, UI implementation, or effect materialization.

## Boundary Rules

- Depend on stable contracts such as `bitfun-runtime-ports`, not `bitfun-core`,
  app crates, Tauri APIs, product UI, or concrete service managers.
- Keep OpenCode config JSON and local plugin source parsing inside fixture
  tests in this crate. Cross-crate outputs must be typed
  `PluginRuntimeReadResponse`, `PluginResponseEnvelope`, diagnostics,
  permission prompts, and effect candidates once a reviewed production consumer
  exists.
- Unsupported OpenCode capabilities must be explicit diagnostics or typed
  unsupported candidates. Do not silently ignore them.
- Current public API budget is empty. This crate owns fixture-scoped projection
  tests only until a reviewed Plugin Runtime Host integration introduces a real
  consumer.
- This crate may provide private OpenCode config/source projectors and contract
  fixtures for adapter validation, but it must not implement
  `PluginRuntimeClient`, declare executable availability, or become the runtime
  host. Product Assembly decides host binding through the reviewed Plugin
  Runtime Host path.
- Production crates must not import `bitfun_opencode_adapter` directly until the
  host integration PR removes the temporary boundary guard with a reviewed
  consumer path.

## Verification

- `cargo test -p bitfun-opencode-adapter opencode_fixture_contracts`
- `node scripts/check-core-boundaries.mjs`

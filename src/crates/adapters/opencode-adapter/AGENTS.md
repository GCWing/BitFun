[中文](AGENTS-CN.md) | **English**

# OpenCode Adapter

This crate owns OpenCode-compatible source discovery and trust-gated candidate
mapping. It validates OpenCode import shapes such as `opencode.json` and
`.opencode/plugins/*.js|ts`, then exposes source facts, diagnostics, and typed
effect candidates through a narrow Plugin Runtime Host adapter. It must not own
product policy, host lifecycle, sandboxing, UI implementation, or effect
result writes.

Product-source boundary:

- BitFun plugin package/install sources are the production entry point for
  plugin loading. OpenCode config is an optional compatibility import source,
  not the primary plugin registry or runtime state.
- Importing `opencode.json`, `.opencode/plugins/*.js|ts`, or future OpenCode
  global plugin directories must produce typed import facts, candidate BitFun
  plugin source records, manifests, hashes, diagnostics, and trust state before
  those facts can enter the product-side enablement or execution path. The
  adapter itself must not enable or execute plugins.
- `load_opencode_workspace_adapter` must receive BitFun source trust snapshots
  through existing `PluginSourceRef` values plus a trust epoch; OpenCode
  directory scanning must not promote sources to trusted on its own.
- Trusted custom tool declarations may only be mapped as provider candidates;
  final tool creation, permission decisions, and audit facts must stay in the
  tool ABI, permission, and product owner path.
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
  public symbols or changes to public entry signatures and semantics require an
  updated public API budget, current consumer, and focused host-path tests.
- This crate may provide private OpenCode compatibility import projectors and
  fixtures for adapter verification. The public entry remains limited to
  `load_opencode_workspace_adapter`, called through Plugin Runtime Host.
- This PR keeps production Product Assembly wiring out of scope. A future
  reviewed registration path may call the public factory through Plugin Runtime
  Host, but must update boundary guards and focused host-path tests in the same
  change.
- Production crates must not depend on `bitfun_opencode_adapter` internals.
  Unsupported capabilities must return diagnostics or typed unsupported states
  instead of failing at runtime on external plugin content.

## Verification

- `cargo test -p bitfun-opencode-adapter --test opencode_source_adapter`
- `cargo test -p bitfun-opencode-adapter p0_c2_fixture`
- `cargo test -p bitfun-opencode-adapter host_path_projects_trusted_custom_tool_candidate_with_permission_prompt`
- `node scripts/check-core-boundaries.mjs`

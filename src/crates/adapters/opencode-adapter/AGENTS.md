[中文](AGENTS-CN.md) | **English**

# OpenCode Adapter

This crate owns OpenCode-compatible package interpretation and trust-gated
candidate mapping. It validates managed package content such as `opencode.json`
and `.opencode/plugins/*.js|ts`, then exposes source facts, diagnostics, and typed
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
- `load_opencode_package_adapter` receives fixed managed package content and an
  optional source-service activation authority. Without that authority,
  `SourceApproved` remains untrusted at the Host boundary and produces no
  candidates.
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
- Keep OpenCode config JSON and plugin source parsing inside this crate.
  Cross-crate outputs must go through `load_opencode_package_adapter`
  and Plugin Runtime Host DTOs; do not expose raw OpenCode JSON or source
  syntax as product contracts.
- Current source inspection recognizes only the tested declarative subset. It is
  not a general JavaScript or TypeScript parser. Packages with no recognized
  entry and recognized unsupported hooks must produce diagnostics; other syntax
  is outside the current compatibility claim.
- Unsupported OpenCode capabilities must be explicit diagnostics or typed
  unsupported candidates. Do not silently ignore them.
- The public API budget is limited to `load_opencode_package_adapter`. New
  public symbols or changes to public entry signatures and semantics require an
  updated public API budget, current consumer, and focused host-path tests.
- This crate may provide private OpenCode compatibility import projectors and
  fixtures for adapter verification. The public entry remains limited to
  `load_opencode_package_adapter`, called by the reviewed product composition
  root before the returned adapter is injected into Plugin Runtime Host.
- Production assembly is limited to `bitfun-core/plugin_runtime`; boundary
  guards and focused host-path tests must change with any additional consumer.
- Production crates must not depend on `bitfun_opencode_adapter` internals.
  Unsupported capabilities must return diagnostics or typed unsupported states
  instead of failing at runtime on external plugin content.

## Verification

- `cargo test -p bitfun-opencode-adapter --test opencode_source_adapter`
- `cargo test -p bitfun-opencode-adapter p0_c2_fixture`
- `cargo test -p bitfun-opencode-adapter host_path_projects_trusted_custom_tool_candidate_with_permission_prompt`
- `node scripts/check-core-boundaries.mjs`

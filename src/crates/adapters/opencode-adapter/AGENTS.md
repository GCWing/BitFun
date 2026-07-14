[中文](AGENTS-CN.md) | **English**

# OpenCode Adapter

The current crate owns the P0 static OpenCode source preview used by the existing
managed-package path. The target design makes this crate the OpenCode-specific
adapter and source coordinator: it preserves source order and ecosystem semantics,
produces versioned source/candidate facts, and creates the adapter injected into
Plugin Runtime Host. It must not own product policy, worker supervision, UI
implementation, credentials, or final effect writes.

Product-source boundary:

- The current `load_opencode_package_adapter` entry remains static-preview only
  until OC-R1/OC-R2 replace its production role. Do not extend this P0 entry into
  another managed OpenCode package format.
- In the target flow, standard OpenCode config, global/project plugin directories,
  tool directories, and package specs are live sources. Source files stay
  read-only, but valid results may affect runtime without BitFun import or a
  second activation step.
- The OpenCode source coordinator owns source identity/order, source watches,
  candidate generations, and the decision to request preparation or switch a
  generation. Config owners provide normalized config snapshots; the script
  execution service owns dependencies/workers; Plugin Runtime Host owns process
  health and contribution registration.
- Effective global/source/target policy and safe-start mode must be resolved
  before third-party module import. The default local policy is compatibility
  mode, not a trust prompt. Source policy changes must not be represented as
  config import approval.
- Final tool creation, permission decisions, authoritative state, and audit facts
  stay in their tool, permission, product, and runtime owner paths.
- The user's local `opencode` CLI installation is unrelated to loading
  OpenCode-compatible plugins. CLI/server interop with an installed OpenCode
  binary belongs to ACP/external-client work, not this adapter boundary.

## Boundary Rules

- Depend on stable contracts such as `bitfun-runtime-ports` and the
  `PluginHostAdapter` boundary trait, not `bitfun-core`, app crates, Tauri
  APIs, product UI, or concrete service managers.
- Keep OpenCode config JSON, source ordering, loader compatibility, and source
  coordination inside this crate. Cross-crate outputs use typed source snapshots,
  adapter bindings, and Plugin Runtime Host DTOs; do not expose raw OpenCode JSON
  or source syntax as product contracts.
- Current source inspection recognizes only the tested declarative subset. It is
  not a general JavaScript or TypeScript parser. Packages with no recognized
  entry and recognized unsupported hooks must produce diagnostics; other syntax
  is outside the current compatibility claim.
- Unsupported OpenCode capabilities must be explicit diagnostics or typed
  unsupported candidates. Do not silently ignore them.
- The current public API budget is limited to `load_opencode_package_adapter`.
  OC-R implementation may replace or supplement it only together with a current
  consumer, explicit source-coordinator/Host ports, boundary updates, and focused
  tests. Target design text alone does not make a new API available.
- The reviewed product composition root selects and constructs the compiled
  OpenCode adapter/provider and injects it into Plugin Runtime Host. It does not
  discover dynamic sources, prepare dependencies, or import plugin modules.
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

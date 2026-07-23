[中文](AGENTS-CN.md) | **English**

# OpenCode Adapter

The current crate owns the static OpenCode source preview used by the existing
managed-package path and the OpenCode-specific implementations of command,
standalone-tool, and subagent provider contracts. It preserves OpenCode source
discovery, precedence, formats, argument expansion, and versioned compatibility semantics.
Shared source catalog, lifecycle coordination, file-watch implementation,
product policy, UI, credentials, worker supervision, and final effect writes
belong elsewhere.

Product-source boundary:

- The current `load_opencode_package_adapter` entry remains static-preview only
  until OC-R1/OC-R2 replace its production role. Do not extend this P0 entry into
  another managed OpenCode package format.
- Standard OpenCode Command, standalone Tool, and Subagent config and directories
  are current read-only live sources. Full plugin directories and package specs
  remain target work rather than executable production sources. Source files need
  no BitFun import. Low-risk declarative results follow the
  user's auto-apply/ask preference; executable sources require a source/target
  decision before first import. Pre-import execution-envelope expansion and
  post-import contribution expansion are separate gates, not repeated approval
  for every internal lifecycle state. Code updates may prepare automatically only
  when source identity/integrity, the source update policy, and the current
  execution envelope still allow it.
- A global source preference is deduplicated by source/target/execution domain, but
  each project/workspace execution instance recomputes its effective source graph,
  working directory/environment, credentials, and policy. Raw parsing and exact
  materialization caches may be shared; candidate workers and health may not be
  treated as one global result. Crossing projects alone does not prompt again;
  only an expanded execution envelope, credential scope, or capability does.
- The shared source coordinator owns candidate generations and atomic provider
  replacement. This adapter supplies OpenCode-qualified source identity/order and
  watch roots through narrow provider contracts; the reusable file-watch service
  supplies change facts. Config owners provide normalized config snapshots; the script
  execution service owns dependencies, workers, process trees, and physical
  health; Plugin Runtime Host owns logical target state and contribution registration.
- Effective policy and safe-start mode must be recomputed before third-party
  module import from the source, target, actual execution domain/user,
  product/organization policy bounds, credential scope, and environment scope.
  Discovery or config-import approval is not an execution decision. The product
  source experience and existing capability owners provide the source/target
  decision; this adapter consumes it but does not own prompts or trust state.
  After activation, the default local runtime policy is compatibility mode.
- Final tool creation, permission decisions, authoritative state, and audit facts
  stay in their tool, permission, product, and runtime owner paths.
- Standalone-tool preparation may return only a version-checked, bounded module
  for an already approved target. It must not spawn a process, install a package,
  persist approval, or interpret another ecosystem. Static import restrictions
  describe the current compatibility subset; they are not a security sandbox.
- The user's local `opencode` CLI installation is unrelated to loading
  OpenCode-compatible plugins. CLI/server interop with an installed OpenCode
  binary belongs to ACP/external-client work, not this adapter boundary.

## Boundary Rules

- Depend on stable contracts such as `bitfun-runtime-ports` and the
  `PluginHostAdapter` boundary trait, not `bitfun-core`, app crates, Tauri
  APIs, product UI, or concrete service managers.
- Keep OpenCode config JSON, source ordering, loader compatibility, and argument
  expansion inside this crate. Cross-crate outputs use typed source snapshots,
  adapter bindings, and Plugin Runtime Host DTOs; do not expose raw OpenCode JSON
  or source syntax as product contracts.
- Current source inspection recognizes only the tested declarative subset. It is
  not a general JavaScript or TypeScript parser. Packages with no recognized
  entry and recognized unsupported hooks must produce diagnostics; other syntax
  is outside the current compatibility claim.
- Unsupported OpenCode capabilities must be explicit diagnostics or typed
  unsupported candidates. Do not silently ignore them.
- Public APIs require a current Product Assembly consumer, a capability-specific
  provider contract, boundary updates, and focused tests. Do not expose generic
  OpenCode JSON access or add APIs only for target-design completeness.
- The reviewed product composition root selects and constructs the compiled
  OpenCode adapter/provider and injects it into Plugin Runtime Host. It does not
  discover dynamic sources, prepare dependencies, or import plugin modules.
- Product Assembly may consume this crate only from reviewed composition modules
  such as `bitfun-core/plugin_runtime` or `bitfun-core/external_sources`; boundary
  guards and focused assembly-path tests must change with any additional consumer.
- This crate must not depend on Codex, Claude Code, or another ecosystem adapter.
  New ecosystems are sibling adapters registered by Product Assembly, not modes of
  this adapter.
- Production crates must not depend on `bitfun_opencode_adapter` internals.
  Unsupported capabilities must return diagnostics or typed unsupported states
  instead of failing at runtime on external plugin content.

## Verification

- `cargo test -p bitfun-opencode-adapter --test opencode_source_adapter`
- `cargo test -p bitfun-opencode-adapter --test opencode_command_adapter`
- `cargo test -p bitfun-opencode-adapter --test tool_source_contracts`
- `cargo test -p bitfun-opencode-adapter --test opencode_subagent_adapter`
- `cargo test -p bitfun-opencode-adapter p0_c2_fixture`
- `cargo test -p bitfun-opencode-adapter host_path_projects_trusted_custom_tool_candidate_with_permission_prompt`
- `node scripts/check-core-boundaries.mjs`

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
- MCP config/process/transport lifecycle, server runtime state
  (registry/connection pool/catalog/reconnect/runtime-only config), lifecycle
  policy, OAuth credential storage/authorization bootstrap, the concrete RMCP
  dependency, and protocol result-content rendering live here. Core may keep
  compatibility exports plus product callback/session/reconnect orchestration,
  but must not reintroduce a direct RMCP dependency. MCP wire types may be
  projected into execution-owned tool bridge descriptors. Product tool registry
  assembly, manifest filtering, `GetToolSpec` execution, and bridge
  presentation/validation behavior remain outside this crate unless a reviewed
  owner move proves behavior equivalence.
- Remote-connect platform-neutral primitives belong here: device identity,
  pairing/encryption, QR payload generation, relay client protocol, dialog/cancel
  orchestration ports, LAN/ngrok provider helpers, IM bot provider clients,
  provider-private cursor caches, mobile-web relay upload, image-context adapter
  contracts, remote workspace helpers, and command/response assembly.
- Remote workspace facts, session metadata, file projection DTOs, and
  workspace/projection host traits belong in `bitfun-runtime-ports`.
- Workspace-root source selection, persistence/workspace service reads,
  concrete scheduler/session restore, terminal pre-warm adapters, and product
  execution remain core-owned unless a reviewed port/provider moves them with
  equivalence tests.
- Remote-SSH registries, disabled surfaces, SSH channels, SFTP, remote FS,
  remote workspace FS/shell providers, remote terminal, remote ExecCommand
  runtime-port adapter, and manager assembly live here behind explicit remote
  SSH features. Stable workspace path/session identity is owned by
  `services-core::workspace_identity`; `remote_ssh::paths` is only its legacy
  compatibility re-export and must not regain transport-independent logic.
- One-click relay self-deploy (`remote_ssh/relay_deploy.rs`) stages embedded
  scripts under `~/.bitfun/relay-deploy/` and clones source to
  `~/.bitfun/relay-src/` (never `$HOME/bitfun`). Embeds
  `src/apps/relay-server/mirror.sh` and runs `bitfun_mirror_init` before apt /
  Docker install / GitHub sync so mainland China hosts use configured mirrors.
  Invariants: `src/web-ui/src/features/relay-deploy/README.md`. Desktop Tauri
  wrapper: `src/apps/desktop/src/api/relay_deploy_api.rs`.
- Workspace search owns the local flashgrep daemon/session lifecycle and
  indexed-search result conversion behind `workspace-search`; product config
  and workspace bootstrap stay in the core facade as injected hooks. The daemon
  returns match positions only, so content output uses
  `search/grouped_line_matches` and hydrates line text from disk in
  `workspace_search/line_hydration.rs`; the preview primitives it shares with
  the ripgrep path live in `bitfun-services-core::filesystem::content_preview`.
- Remote SSH workspace-search owns the disabled surface, path/scope/probe,
  bundle/retry strategy, and flashgrep session/context lifecycle behind a
  provider boundary.
- Browser-control owns provider-neutral browser detection, CDP endpoint HTTP
  probing/page creation, and CDP launch process handling behind
  `browser-control`; product profile paths and tool request/result types stay in higher
  layers.
- Web tool network providers own concrete HTTP/Exa requests behind `web-tools`;
  product validation, readable extraction, and tool result types stay in
  higher layers.
- Review-platform provider detection, repository discovery, token persistence,
  provider DTO mapping, pagination policy, HTTP transport, and Git provider
  integration live behind `review-platform`; core may only inject product data
  paths, remote-workspace classification, and compatibility API wrappers.
- MiniApp runtime here may own host primitive dispatch, built-in seed file
  writes, marker IO, storage/import bundle filesystem IO, and JS worker process/pool
  lifecycle. Manager workflow orchestration remains outside this crate until
  reviewed owner migration.
- Managed plugin source integration may own bounded package discovery,
  integrity checks, fixed package input reads, no-follow path handling,
  trust-file locking, and atomic persistence. Product path selection stays in
  assembly; ecosystem parsing and
  PluginRuntimeClient behavior stays in its adapter and execution modules.
- Script-tool runtime integration owns provider-neutral process supervision,
  bounded framing/output, script load/invoke/cancel/dispose, timeout, and worker
  health behind `script-tool-runtime`. It must not parse OpenCode source paths,
  decide approval/conflicts, register product tools, or claim OS sandboxing.
  Approved modules run in dedicated child processes separated from the Rust application process for
  failure containment, not as a security or protocol-authentication boundary.
  The shared `services-core::process_tree` boundary owns managed-descendant cleanup for
  script workers, local stdio MCP, and other managed service children: Unix uses a dedicated process group; Windows attaches a
  suspended child to a kill-on-close Job Object before resuming it and fails
  closed when attachment fails. This is lifecycle containment, not an OS
  sandbox or a CPU/memory/filesystem/network resource limit; surfaces must keep
  those residual risks explicit. Unix descendants that deliberately create a
  new session/process group are outside the managed boundary.
- Announcement remote fetch/cache lives here; product assembly supplies config
  values such as endpoint, locale, version, platform, and cache path.
- DeepResearch report IO here owns report/citation sidecar filesystem work;
  provider-neutral citation numbering stays in `bitfun-agent-runtime`. The IO
  path must use the injected `WorkspaceFileSystem` for both local and remote
  workspaces; never probe or fall back to the host filesystem for a remote
  workspace path.

## Verification

Select one integration family and its minimum feature set. Remote SSH uses a
grouped target for tests within the same boundary; use
`--test <target> <module>::<filter>` for a single source module instead of
creating another Cargo target. Real transport/system boundaries such as MCP
streamable HTTP stay independent. Representative stable entry points are:

```bash
cargo check -p bitfun-services-integrations --no-default-features
cargo test -p bitfun-services-integrations --no-default-features --features mcp --test mcp_contracts
cargo test -p bitfun-services-integrations --no-default-features --features remote-ssh --test remote_ssh_contracts remote_ssh_disabled_contracts::
cargo test -p bitfun-services-integrations --no-default-features --features remote-ssh-concrete --lib remote_ssh::manager::tests::workspace_
cargo test -p bitfun-services-integrations --no-default-features --features file-watch --test file_watch_contracts
cargo test --locked -p bitfun-services-integrations --no-default-features --features deep-research --lib deep_research::tests::
pnpm run check:core-boundaries
```

Other family-specific targets remain in `Cargo.toml`; add a guide command only
for a recurring workflow, not to mirror every test target.

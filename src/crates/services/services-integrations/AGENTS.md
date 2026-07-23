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
  policy, and protocol result-content rendering live here; MCP wire types may be
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
- Remote-SSH path/session identity helpers, disabled surfaces, SSH channels,
  SFTP, remote FS, remote workspace FS/shell providers, remote terminal, remote
  ExecCommand runtime-port adapter, and manager assembly live here behind
  explicit remote SSH features.
- One-click relay self-deploy (`remote_ssh/relay_deploy.rs`) stages embedded
  scripts under `~/.bitfun/relay-deploy/` and clones source to
  `~/.bitfun/relay-src/` (never `$HOME/bitfun`). Invariants:
  `src/web-ui/src/features/relay-deploy/README.md`. Desktop Tauri wrapper:
  `src/apps/desktop/src/api/relay_deploy_api.rs`.
- Workspace search owns the local flashgrep daemon/session lifecycle and
  indexed-search result conversion behind `workspace-search`; product config
  and workspace bootstrap stay in the core facade as injected hooks.
- Remote SSH workspace-search owns the disabled surface, path/scope/probe,
  bundle/retry strategy, and flashgrep session/context lifecycle behind a
  provider boundary.
- Browser-control owns provider-neutral browser detection, CDP endpoint HTTP
  probing/page creation, and CDP launch process handling behind
  `browser-control`; product profile paths and tool envelopes stay in higher
  layers.
- Web tool network providers own concrete HTTP/Exa requests behind `web-tools`;
  product validation, readable extraction, and tool result envelopes stay in
  higher layers.
- Debug log file append, redaction, default path/env config, and optional HTTP
  dispatch live behind `debug-log`; core only keeps ingest-server and product
  workspace path adaptation.
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
  Plugin Runtime Host behavior stay in their adapter and execution owners.
- Script-tool runtime integration owns provider-neutral process supervision,
  bounded framing/output, target load/invoke/cancel/dispose, timeout, and worker
  health behind `script-tool-runtime`. It must not parse OpenCode source paths,
  decide approval/conflicts, register product tools, or claim OS sandboxing.
  Approved modules run in target child processes separated from the Rust host for
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
- DeepResearch report IO here may own report/citation sidecar filesystem work;
  provider-neutral citation numbering stays in `bitfun-agent-runtime`.

## Verification

```bash
cargo test -p bitfun-services-integrations
cargo test -p bitfun-services-integrations --no-default-features --features plugin-source plugin_source --lib
cargo test -p bitfun-services-integrations --features debug-log --test debug_log_owner_contracts
cargo test -p bitfun-services-integrations --features remote-ssh --test remote_ssh_disabled_contracts
cargo test -p bitfun-services-integrations --features remote-ssh,workspace-search --test remote_workspace_search_disabled_contracts
cargo test -p bitfun-services-integrations --features remote-ssh,remote-ssh-concrete,workspace-search remote_ssh
node scripts/check-core-boundaries.mjs
cargo check -p bitfun-core --features product-full
```

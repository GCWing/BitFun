# services-core Agent Guide

Scope: this guide applies to `src/crates/services/services-core`.

`bitfun-services-core` owns cross-platform service DTOs and helpers that compile
without the full product runtime. This includes generic filesystem/search/JSON
IO helpers, bounded local Instruction file reads, session metadata storage
helpers, and local OS action primitives such as command lookup,
clipboard, file/url opening, script execution, workspace runtime FS/shell
providers, process-wide TLS provider selection, managed process-tree lifecycle,
process-level Agent Runtime ownership locks, and system facts. Product crates may layer routing, policy,
capability selection, event emission, or legacy error mapping outside this
crate.

## Guardrails

- Do not depend on `bitfun-core`, app crates, Tauri, tool runtime, or product
  runtime crates.
- Prefer `bitfun-core-types` for shared DTOs and `bitfun-runtime-ports` for
  cross-layer traits.
- Keep dependency features explicit and keep `default = []`. The coarse service
  capability owners are `credential-vault` (prompt-free encrypted local
  credential files), `diagnostics` (diagnostic-log redaction), `diff`
  (local text diff calculation), `filesystem` (local file operations/search),
  `json-io` (generic locked and atomic JSON file IO), `local-storage`
  (JSON/session/usage persistence), `process-runtime` (command
  lookup and supervised child lifecycle), and `workspace-instructions`
  (declarative instruction discovery). Consumers enable those or the narrower
  `workspace-runtime`, `workspace-identity`, `runtime-ownership`, `tls-provider`,
  `permission`, `dispatch-workspace`, `markdown`, `session-git`, and
  `workspace-text-runtime` extensions only for behavior they use. Products
  needing IANA time-zone ranges and dashboard aggregation additionally select
  `token-usage-statistics`. In particular, session metadata consumers must
  not compile libgit2 unless they use the memory-workspace baseline/diff API.
  Keep Tokio and platform API capabilities owner-scoped too: the empty profile
  carries no Tokio dependency, `workspace-runtime` explicitly composes
  `process-runtime`, and Windows storage/process bindings
  must not be enabled from one shared dependency feature union.
- `tls-provider` is the single owner of the process-wide Rustls provider. It
  selects only `ring`, `std`, and `tls12`; provider-neutral Reqwest consumers
  call `tls_provider::ensure_ring_crypto_provider` before client construction.
  Do not install a Rustls provider from another crate.
- Runtime call sites that touch agent execution, scheduler state, workspace
  managers, filesystem orchestration, or product behavior stay outside this
  crate. `workspace-runtime` may implement local `bitfun-runtime-ports`
  providers, but not workspace selection or product orchestration.
- `runtime_ownership` owns only canonical identity plus Embedded shared-lock and
  Shared exclusive-lock primitives. It must not select workspaces, start or
  cache Runtime instances, or define Session/Turn ownership.
- `workspace_identity` owns canonical local roots plus stable local/remote
  workspace and session-storage identifiers. It has no SSH registry, transport,
  authentication, SFTP, PTY, or remote lifecycle responsibility; integrations
  may preserve old paths through re-exports.
- Do not add remote SSH, MiniApp storage, tool-result persistence, `PathManager`
  globals, or product runtime bindings to `filesystem`; keep those in core or a
  reviewed adapter/provider.
- Preserve legacy core imports with facade/re-export code when ownership moves.
- `process_tree` is the single reusable owner for supervised child-process
  lifecycle. Unix implementations use a dedicated process group; Windows must
  attach a suspended child to a kill-on-close Job Object before resuming and
  fail closed if attachment fails. Consumers own protocol shutdown; this owner
  owns cleanup for managed descendants and does not claim sandbox or
  resource-limit safety. Unix descendants that deliberately create a new
  session/process group are outside this boundary and must be treated as a
  disclosed residual risk until a platform supervisor is introduced.

## Verification

Start from the capability that owns the change. Integration targets group test
source files with the same owner and feature closure; keep a focused run small
with `--test <target> <module>::<filter>` instead of adding another Cargo
target. Representative stable entry points are:

```bash
cargo check -p bitfun-services-core --no-default-features
cargo test -p bitfun-services-core --no-default-features --features credential-vault --lib credential_vault::tests::
cargo check -p bitfun-services-core --no-default-features --features filesystem
cargo test -p bitfun-services-core --no-default-features --features diagnostics --lib diagnostics::contract_tests::
cargo test -p bitfun-services-core --no-default-features --features diff --lib diff::contract_tests::
cargo test -p bitfun-services-core --no-default-features --features workspace-text-runtime --lib workspace_text::tests::
cargo test -p bitfun-services-core --no-default-features --features workspace-runtime --lib workspace::tests::
cargo test -p bitfun-services-core --no-default-features --features local-storage --test session_contracts session_metadata_contracts::
cargo test -p bitfun-services-core --no-default-features --features local-storage --test session_write_lock_contracts
cargo test -p bitfun-services-core --no-default-features --features token-usage-statistics --lib token_usage::
cargo test -p bitfun-services-core --no-default-features --features process-runtime --test process_runtime_contracts
cargo test --locked -p bitfun-services-core --no-default-features --features tls-provider --lib tls_provider::tests
pnpm run check:core-boundaries
```

Other capability-specific target names remain in `Cargo.toml`; document a new
command here only when it becomes a recurring owner workflow.

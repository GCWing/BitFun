# Claude Code / Codex Command Hook Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `executing-plans` for inline execution or the repository-approved subagent workflow for task-by-task execution. Keep the implementation in one final commit and run a context-isolated adversarial review before any push or PR update.

**Goal:** Let local CLI/TUI and Desktop users explicitly review, import, update, enable, disable, and remove the synchronous Claude Code/Codex command-Hook subset through BitFun's existing native Hook runtime.

**Architecture:** Keep `AgentHookEngine` as the only executor and `ExternalHookCatalogCoordinator` as the only external Hook discovery owner. Add one command-bearing product-domain contract, one managed persistence service, and one core plan/apply orchestrator. Extend existing adapters and product surfaces; do not add a Hook SDK, generic import framework, second coordinator, second executor, watcher, or OpenCode runtime.

**Tech stack:** Rust, Tokio, Serde, SHA-256, Clap/TUI, Tauri, React/TypeScript, i18next.

**Approved design:** `docs/superpowers/specs/2026-07-28-claude-codex-command-hook-import-design.md`

## Hard scope and interface limits

These are implementation acceptance gates, not suggestions:

1. Do not change `bitfun-agent-runtime` Hook parsing, dispatch, scheduling, payload, or outcome interfaces. Imported data must enter as ordinary `AgentHookSettingsLayer` values.
2. Do not add commands, bodies, environment values, asset bytes, or trust state to `ExternalHookCatalogSnapshotV1`; its current redacted `content_version` remains unchanged.
3. Do not introduce a generic importer, Hook manager hierarchy, HookBus, executor registry, plugin protocol, external watcher, or remote execution path.
4. Do not add a second JSON/TOML Hook traversal. Refactor `static-hook-support` once, keeping `parse_hook_document` as a compatibility wrapper.
5. Do not duplicate backend behavior in CLI or React. Both surfaces call the same core plan/apply/snapshot/mutation operations.
6. Do not share MCP-specific DTOs with Hook import. Reuse only its established preview/apply/stale interaction and the existing atomic JSON primitive.
7. The serialized public surface is limited to the types listed in Task 1. Any additional serialized DTO, fourth backend owner module, or change to OpenCode execution requires a design review before proceeding.
8. Do not add a new frontend state framework, stylesheet, or Hook-specific dialog component. Extend `HooksConfig.tsx` and reuse `Modal`, `ConfirmDialog`, `ConfigPageSection`, `Button`, and `Switch`.

## File map

### New production files

| File | Single responsibility |
|---|---|
| `src/crates/contracts/product-domains/src/external_hook_import.rs` | Versioned local-only import DTOs plus non-serialized prepared adapter facts. |
| `src/crates/services/services-integrations/src/hook_import.rs` | Bounded managed bundle/index IO and in-memory store snapshot. |
| `src/crates/assembly/core/src/external_hook_import.rs` | Product path selection, plan/apply fencing, mutations, update checks, and native-layer projection. |
| `src/apps/cli/src/hook_import.rs` | Root CLI text/JSON projection over the shared core operations. |

### Existing files to extend

| Area | Files |
|---|---|
| Shared parsing | `src/crates/adapters/static-hook-support/src/lib.rs`, `src/crates/adapters/static-hook-support/tests/parser.rs` |
| Provider port/coordinator | `src/crates/contracts/product-domains/src/external_hook_catalog.rs`, `src/crates/assembly/external-sources/src/hook.rs`, `src/crates/assembly/external-sources/tests/hook_coordinator.rs` |
| Ecosystem conversion | `src/crates/adapters/codex-adapter/src/hook_source.rs`, `src/crates/adapters/codex-adapter/tests/hook_source.rs`, `src/crates/adapters/claude-code-adapter/src/hook_source.rs`, `src/crates/adapters/claude-code-adapter/tests/hook_source.rs` |
| Native runtime wiring | `src/crates/assembly/core/src/native_hooks.rs`, `src/crates/assembly/core/src/native_hooks_tests.rs`, `src/crates/assembly/core/src/lib.rs`, plus the existing `PathManager` native path digest visibility needed to isolate colliding workspace slugs |
| Cargo and boundary facts | `src/crates/services/services-integrations/Cargo.toml`, `scripts/core-boundaries/rules/feature-rules.mjs`, `scripts/core-boundaries/rules/source/public-api-rules.mjs` |
| CLI/TUI | `src/apps/cli/src/main.rs`, `src/apps/cli/src/actions.rs`, `src/apps/cli/src/modes/chat.rs`, `src/apps/cli/src/modes/chat/native_hooks.rs`, `src/apps/cli/src/modes/chat/external_hooks.rs`, `src/apps/cli/src/modes/chat/commands.rs`, `src/apps/cli/src/modes/chat/run.rs`, `src/apps/cli/src/modes/chat/tests.rs` |
| Desktop transport | `src/apps/desktop/src/api/external_hooks_api.rs`, `src/apps/desktop/src/api/remote_workspace_policy.rs`, `src/apps/desktop/src/lib.rs` |
| Web API/UI | `src/web-ui/src/infrastructure/api/service-api/ExternalHooksAPI.ts`, `src/web-ui/src/infrastructure/api/service-api/ExternalHooksAPI.test.ts`, `src/web-ui/src/infrastructure/config/components/HooksConfig.tsx`, new `HooksConfig.test.tsx` beside it |
| i18n/docs | `src/web-ui/src/locales/{en-US,zh-CN,zh-TW}/settings/hooks.json`, `docs/features/agent-hooks.md`, `docs/features/agent-hooks.zh-CN.md`, only the architecture sentences that still call Codex/Claude Hooks reference-only |

No production change is planned under `src/crates/execution/agent-runtime` or the OpenCode adapter.

## Stable operation surface

Task 1 must implement only this serialized shape, with `camelCase`, `deny_unknown_fields`, bounded validation, schema version `1`, and redacted custom `Debug` wherever exact commands can appear:

```rust
pub struct ExternalHookImportHandlerV1 {
    pub stable_key: String,
    pub event: String,
    pub matcher: Option<String>,
    pub command: String,
    pub command_windows: Option<String>,
    pub timeout_seconds: Option<u64>,
    pub status_message: Option<String>,
    pub dependencies: Vec<ExternalHookImportDependencyV1>,
}

pub struct ExternalHookImportSkippedV1 {
    pub reason_code: String,
    pub count: u32,
}

pub struct ExternalHookImportPlanV1 {
    pub schema_version: u32,
    pub source: ExternalHookSource,
    pub disposition: ExternalHookImportDispositionV1,
    pub behavior_version: String,
    pub handlers: Vec<ExternalHookImportHandlerV1>,
    pub skipped: Vec<ExternalHookImportSkippedV1>,
    pub plan_fingerprint: String,
}

pub struct ExternalHookImportApplyRequestV1 {
    pub schema_version: u32,
    pub source: SourceKey,
    pub plan_fingerprint: String,
}

pub enum ExternalHookImportApplyOutcomeV1 {
    Applied { snapshot: ExternalHookImportSnapshotV1 },
    Unchanged { snapshot: ExternalHookImportSnapshotV1 },
    Stale { refreshed_plan: ExternalHookImportPlanV1 },
}

pub struct ExternalHookImportApplyResultV1 {
    pub schema_version: u32,
    pub outcome: ExternalHookImportApplyOutcomeV1,
}

pub struct ImportedHookSourceSnapshotV1 {
    pub import_id: String,
    pub source: ExternalHookSource,
    pub enabled: bool,
    pub behavior_version: String,
    pub state: ImportedHookSourceStateV1,
}

pub struct ExternalHookImportSnapshotV1 {
    pub schema_version: u32,
    pub revision: String,
    pub catalog: ExternalHookCatalogSnapshotV1,
    pub imports: Vec<ImportedHookSourceSnapshotV1>,
    pub diagnostics: Vec<ExternalSourceDiagnostic>,
}

pub struct ExternalHookImportMutationRequestV1 {
    pub schema_version: u32,
    pub expected_revision: String,
    pub action: ExternalHookImportMutationV1,
}

pub enum ExternalHookImportMutationV1 {
    SetEnabled { import_id: String, enabled: bool },
    Remove { import_id: String },
    ResetCorruptStore { scope: ExternalSourceScope },
}
```

`ExternalHookImportDispositionV1`, `ExternalHookImportDependencyV1`, and `ImportedHookSourceStateV1` are closed enums used by these DTOs; do not add parallel summary/action types. The snapshot embeds the existing redacted catalog so CLI and Desktop do not invent different “available plus imported” merge contracts. The public `revision` is a stable hash of the user-store generation, workspace-store generation/state, and workspace identity, because one `u64` cannot fence two scope stores. Mutations return `ExternalHookImportSnapshotV1`; stale revisions use the existing `ExternalSourceOperationErrorCode::StaleRevision` rather than another result envelope.

The non-serialized provider result is one `PreparedExternalHookImport` containing normalized handler facts, aggregated skip facts, and bounded asset bytes. It has a custom redacted `Debug`, computes one private `behavior_version`, and is not a persisted or user-authored format.

## Task 1: Lock contracts and make one shared document walk

**Files:**

- Add `src/crates/contracts/product-domains/src/external_hook_import.rs`.
- Modify `src/crates/contracts/product-domains/src/external_hook_catalog.rs`.
- Modify `src/crates/contracts/product-domains/src/lib.rs`.
- Modify `src/crates/contracts/product-domains/tests/external_hook_catalog_contracts.rs`.
- Modify `src/crates/adapters/static-hook-support/src/lib.rs`.
- Modify `src/crates/adapters/static-hook-support/tests/parser.rs`.

- [x] Add failing contract tests for exact JSON field names, schema rejection, unknown-field rejection, bounded command/diagnostic counts, invalid identifiers, stable SourceKey round trips, and redacted `Debug` output.
- [x] Add failing parser tests proving one traversal preserves current redacted results for JSON and TOML while exposing borrowed group/handler values only to a closure.
- [x] Introduce `visit_hook_document(bytes, format, max_handlers, visitor)` and a borrowed `StaticHookHandlerRef` in `static-hook-support`. The returned summary contains only `all_disabled`, structural issues, and `inspected_handlers`; it never retains raw `serde_json::Value` data.
- [x] Reimplement the existing `parse_hook_document` as a wrapper around `visit_hook_document`. Keep its signature, ordering, issue de-duplication, handler limits, and `redacted_parse_content_version` byte-for-byte compatible in tests.
- [x] Add `PreparedExternalHookImport` and the versioned DTOs above. Centralize prepared-fact validation and behavior hashing here so Codex and Claude adapters do not implement separate fingerprint logic.
- [x] Extend `ExternalHookSourceProvider` with one default `prepare_import(context, source, expected_catalog_content_version)` method returning the standard unsupported provider error. Do not add another provider trait.
- [x] Register the module under the existing `external-sources` feature; do not add a new product-domain feature.

Run:

```powershell
cargo test -p bitfun-product-domains --features external-sources external_hook
cargo test -p bitfun-static-hook-support
```

Expected: new tests fail before implementation, then pass; existing catalog snapshots and redacted content versions remain unchanged.

## Task 2: Convert the conservative Codex and Claude command subset

**Files:**

- Modify both existing adapter `hook_source.rs` files and their existing tests.
- Modify `static-hook-support` only for shared static path recognition/asset collection needed by both adapters.

- [x] Add Codex fixtures for user/project `hooks.json`, inline TOML, `commandWindows`, timeout/status, unsupported handler types, async/unknown behavior fields, unsupported events, malformed matchers, and command-only changes that keep catalog `content_version` stable but change private `behavior_version`.
- [x] Add Claude fixtures for `settings.json` and `settings.local.json`, missing `type`, `timeoutSec`, `disableAllHooks`, group/handler `if`, `async`, `asyncRewake`, `args`, `shell`, `once`, non-command types, unknown fields, and Claude-only events.
- [x] Implement both conversions through `visit_hook_document`; do not parse the same file again in an adapter-specific walker.
- [x] Preserve deterministic source/group/handler ordering and aggregate skipped reasons by stable reason code.
- [x] Add one shared conservative path helper for statically recognizable references under the source `hooks/` directory. Preserve unrecognized commands verbatim, reject dynamic source-root expressions, and identify absolute external dependencies for review.
- [x] Collect only referenced regular files. Reject links/reparse points, traversal, unreadable files, and budgets over 256 files, 1 MiB per file, 16 MiB total, or depth 8. Read asset bytes during preparation so the behavior digest and later write use the same bounded content.
- [x] Ensure no `CLAUDE_*`/`CODEX_*` environment, credentials, enablement, or trust data is copied.

Run:

```powershell
cargo test -p bitfun-codex-adapter hook_source
cargo test -p bitfun-claude-code-adapter hook_source
cargo test -p bitfun-static-hook-support
```

Expected: eligible handlers normalize to the current native field names; unsupported semantics remain visible only as skip counts/reasons; OpenCode code and tests do not change.

## Task 3: Persist one bounded managed snapshot store

**Files:**

- Add `src/crates/services/services-integrations/src/hook_import.rs` with inline unit tests.
- Modify `src/crates/services/services-integrations/src/lib.rs` and `Cargo.toml`.
- Modify `src/crates/assembly/core/Cargo.toml` to enable the narrow service feature through `product-full`.

- [x] Add focused tests for empty/valid restart-safe load, corrupt index fail-closed/reset without generation ABA, generation fencing, idempotent apply, invalid scope and management IDs, missing-bundle repair, same-path repair failure preserving the indexed bundle, pre-index-commit publication rollback, successful retired-bundle cleanup, refusal to re-enable a missing bundle, valid-but-modified Hook/asset rejection, exact-version removal, and managed-ancestor link/reparse rejection. Fixed source-asset budgets stay covered by the shared preparation tests; atomic index replacement stays covered by `JsonFileStore`.
- [x] Add a `hook-import` service feature depending only on `bitfun-services-core`, `bitfun-product-domains/external-sources`, `bitfun-agent-runtime` for native document validation, `sha2`, `hex`, `thiserror`, and `uuid`. Add it to the existing `product-full` feature; do not reuse the broader `plugin-source` feature.
- [x] Implement `HookImportStore` with one root path supplied by assembly. Its private index contains only schema, a generation token that increments while valid and is freshly reseeded after corrupt reset, import identity, source key/kind/scope plus redacted display/location facts needed after restart, behavior version, active behavior-digest directory, exact managed-content digest, and enabled state. Do not persist the generation-sensitive review fingerprint, catalog diagnostics, exact review DTOs, source commands outside the managed native file, or foreign trust state.
- [x] Reuse `JsonFileStore::acquire_cross_process_lock` and `write_atomic_strict` for the index. Do not add another atomic JSON helper. Bound index reads before deserialization without expanding `JsonFileStore` unless a second real consumer is found.
- [x] Stage `hooks.json` and asset bytes in a sibling temporary directory before touching the indexed path, validate the generated layer with `AgentHookSettings::from_layers`, compute the exact managed-content digest, move to `bundles/<import-id>/<behavior-digest>`, atomically publish the index, then update the in-memory snapshot. A same-path repair retires the old directory only after staging and restores it when final rename or index publication fails. Verify exact indexed content on store open/index change. Cleanup old versions only after publication; cleanup failure leaves inactive residue and never rolls back or reactivates it.
- [x] Make disable an index-only change. Make remove publish the index first and then delete only the removed record's exact digest directory while retaining the same cross-process lock. `ResetCorruptStore { scope }` is accepted only while that scoped store is corrupt and requires the caller's current combined revision. A corrupt store contributes an index-metadata/error marker to the revision; reset publishes a freshly seeded non-zero generation so old generation-zero plans cannot recur.
- [x] Track the index metadata fingerprint so another local BitFun process causes one reload on the next access; do not enumerate bundle directories or scan external products.

Run:

```powershell
cargo test -p bitfun-services-integrations --no-default-features --features hook-import hook_import
```

Expected: every failed stage keeps the previous index and active bundle; no test writes outside its temporary managed root.

## Task 4: Add plan/apply orchestration and feed the existing engine

**Files:**

- Add `src/crates/assembly/core/src/external_hook_import.rs` with inline unit tests.
- Modify `src/crates/assembly/core/src/external_hooks.rs`, `src/crates/assembly/core/src/native_hooks.rs`, `src/crates/assembly/core/src/native_hooks_tests.rs`, and `src/crates/assembly/core/src/lib.rs`.
- Modify `src/crates/assembly/external-sources/src/hook.rs` and its existing coordinator tests.

- [x] Add focused tests for guarded preparation and redacted-version mismatch, provider-default unsupported behavior, private plan fingerprint fencing, native document compatibility, stable import IDs, missing-bundle recovery, deterministic native layer order, project imports independent of the manual project-file gate, next-generation cache replacement with an unchanged captured engine, and remote dispatch rejection. Keep source-missing/update-failure behavior fail-closed in the typed snapshot path without adding a second test-only coordinator.
- [x] Add a guarded synchronous preparation method to the existing `ExternalHookCatalogCoordinator`; it resolves the registered provider and current source and verifies the expected redacted catalog version. Wrap that method in a per-source in-flight `OnceCell` inside the existing core `WorkspaceExternalHookCatalogService`, using `spawn_blocking`, so simultaneous Desktop/TUI requests await the same preparation. Remove the cell after completion; do not cache completed prepared commands across requests because command-only changes do not change catalog `content_version`.
- [x] Expose these core functions and no second facade:

```rust
pub async fn external_hook_import_snapshot(
    workspace: Option<&Path>,
    refresh_updates: bool,
) -> ExternalSourceOperationResult<ExternalHookImportSnapshotV1>;

pub async fn plan_external_hook_import(
    workspace: Option<&Path>,
    source: SourceKey,
) -> ExternalSourceOperationResult<ExternalHookImportPlanV1>;

pub async fn apply_external_hook_import(
    workspace: Option<&Path>,
    request: ExternalHookImportApplyRequestV1,
) -> ExternalSourceOperationResult<ExternalHookImportApplyResultV1>;

pub async fn mutate_external_hook_import(
    workspace: Option<&Path>,
    request: ExternalHookImportMutationRequestV1,
) -> ExternalSourceOperationResult<ExternalHookImportSnapshotV1>;
```

- [x] Derive user/workspace store roots from the existing `PathManager`; pass roots down to the service. Reject remote workspaces before local file access.
- [x] Materialize the managed asset root before public review, serialize one native `hooks.json` from the same normalized handler facts, validate it with the current parser, and compute the plan fingerprint over source/catalog guard, private behavior version, fully materialized handler review, and current target generation.
- [x] On apply, refresh/guard the source, prepare again, rebuild the plan, compare the accepted fingerprint, and return `Stale { refreshed_plan }` without a write when anything changed.
- [x] Keep one bounded core cache of `Arc<HookImportStore>` by managed root so native dispatch, CLI, and Desktop share in-process generation/state. Do not create a generic import-store registry.
- [x] In `native_hooks::engine_for`, obtain the known imported layer snapshot asynchronously, then assemble this order: manual user, enabled imported user ordered by import ID, gated manual project, enabled imported workspace ordered by import ID. Add store generation/index fingerprint to `CachedHookEngine`; keep existing manual metadata fingerprints.
- [x] Keep an in-flight `Arc<AgentHookEngine>` unchanged. Mutations affect the next event only. Do not change command concurrency or outcome aggregation.
- [x] Extend `NativeHookOverview` construction to show imported files/rules through the same assembled paths; do not add a second imported-rule projection.

Run:

```powershell
cargo test -p bitfun-external-sources hook
cargo test -p bitfun-core external_hook_import
cargo test -p bitfun-core native_hook
node scripts/check-core-boundaries.mjs
```

Expected: manual Hooks still work when import state is absent/corrupt; a successful mutation invalidates exactly one cached generation; remote dispatch remains skipped.

## Task 5: Make CLI/TUI the complete primary surface

**Files:**

- Add `src/apps/cli/src/hook_import.rs`.
- Modify the existing CLI/TUI files in the file map.

- [x] Add focused Clap/TUI parser tests for preview-only import, confirmed destructive removal/reset, aliases, and the explicit second step for interactive writes; expose this complete command family:

```text
bitfun hooks list [--refresh] [--format text|json]
bitfun hooks import --source <SourceKey::stable_key()> [--confirm <plan-fingerprint>] [--format text|json]
bitfun hooks update <import-id> [--confirm <plan-fingerprint>] [--format text|json]
bitfun hooks enable <import-id>
bitfun hooks disable <import-id>
bitfun hooks remove <import-id> --confirm
bitfun hooks reset <user|project> --confirm
```

- [x] Implement root commands as thin projections over the four core operations. `hooks list` renders `snapshot.catalog` plus `snapshot.imports`, including each catalog source's stable key. Import/update without a fingerprint is preview-only; a stale fingerprint prints/serializes the refreshed plan and exits without writing. Mutation staleness is returned to the user and never automatically replayed. Keep protocol stdout free of logs.
- [x] Replace the separate read-only TUI mental model with one `/hooks` renderer combining `NativeHookOverview`, `ExternalHookImportSnapshotV1`, and the existing redacted catalog. Keep `/hooks_external` and `/hooks-external` as compatibility aliases to the same management view.
- [x] Support `/hooks [refresh|import <n>|update <n>|enable <n>|disable <n>|remove <n>|reset <user|project>]`. The first import/update invocation stores exactly one pending plan in `ChatMode` and renders full commands, Windows overrides, effective timeout, dependencies, skips, and fingerprint. Repeating the same action with `--confirm` applies that cached fingerprint; remove and corrupt-store-only reset also require `--confirm`. Do not add another confirmation popup or ask the user to type the long hash.
- [x] Replace the current Hook catalog receiver with one CLI-local Hook management result enum/receiver and one optional pending plan. Do not add parallel receivers for plan/apply/mutation.
- [x] Escape every source label, command, path, diagnostic, and error with the existing terminal escaping helper. Bound displayed lists while keeping JSON complete.
- [x] Update action help, command palette, asynchronous management refresh, and existing chat tests. Preserve old aliases and root command compatibility.

Run:

```powershell
cargo test -p bitfun-cli hooks
cargo check -p bitfun-cli
```

Expected: CLI is functionally complete without Desktop; every write requires explicit review confirmation; `/hooks_external` no longer exposes a separate implementation path.

## Task 6: Add the Desktop transport and compact Settings UX

**Files:**

- Modify the Desktop and Web files in the file map; add only `HooksConfig.test.tsx`.

- [x] Add Desktop tests for structured camelCase requests, nested core requests, unknown-field rejection, and explicit `Reject` remote policies for snapshot/plan/apply/mutate commands; retain the existing command-registration contract coverage.
- [x] Extend `external_hooks_api.rs` with four Tauri commands that map directly to the four core operations. Keep the existing catalog command and file; do not add another API module.
- [x] Extend `ExternalHooksAPI.ts` with exact bounded validators for the v1 DTOs. Reuse `invokeExternalSourceCommand`, `exactRecord`, `boundedString`, SourceKey parsing, and common error handling; do not create a second client service.
- [x] Add component tests for the highest-risk interactions: parallel initial load plus exact command/skipped-reason review before apply; stale plan replacement without implicit apply; stale mutation refresh without action replay; and destructive removal only after a source-preservation confirmation. Keep remaining state transitions thin projections over the closed, validator-tested API.
- [x] Extend `HooksConfig.tsx` with one imported-source section and one reused `Modal` for plan review. Reuse `ConfirmDialog` for remove/reset. Keep the two existing switches and make the master switch gate imported execution without deleting state.
- [x] Load discovery only when the Hooks settings page mounts or the user refreshes. Do not poll after unmount and do not add a file watcher.
- [x] Add only owner-namespace strings to `settings/hooks.json` for `en-US`, `zh-CN`, and `zh-TW`; preserve product names as interpolated/provider display facts rather than hard-coded translated brand variants.

Run:

```powershell
pnpm --dir src/web-ui run test:run src/infrastructure/api/service-api/ExternalHooksAPI.test.ts src/infrastructure/config/components/HooksConfig.test.tsx
pnpm run i18n:contract:test
pnpm run type-check:web
cargo check -p bitfun-desktop
```

Expected: the common path is “review once, then enabled”; only arbitrary-code import/update and destructive remove/reset require confirmation.

## Task 7: Documentation, focused verification, and adversarial scope review

**Files:**

- Modify `docs/features/agent-hooks.md` and `docs/features/agent-hooks.zh-CN.md`.
- Modify only architecture sentences that still describe Claude/Codex Hooks as permanently reference-only.
- Update this plan's checkboxes during execution; do not add another implementation spec.

- [x] Document import destination, user/workspace scope, enable/disable/remove, no startup re-import, no watcher, explicit update review, next-event hot reload, actual deterministic layer order, external dependency behavior, remote unsupported state, and OpenCode runtime deferral.
- [x] Run `pnpm run fmt:rs` and `git diff --check`.
- [x] Run all focused commands from Tasks 1-6, then `cargo check --workspace`, `pnpm run i18n:audit`, and `pnpm run check:repo-hygiene`.
- [x] Inspect `git diff --stat` and the public API diff. Stop and redesign if the implementation added a fourth backend owner, a second parser/executor/coordinator, a generic import abstraction, an unlisted serialized DTO, OpenCode runtime behavior, watcher code, or agent scheduling changes.
- [x] Search for duplicated conversions and surface logic:

```powershell
rg -n "prepare_import|behavior_version|plan_fingerprint|visit_hook_document|HookImportStore" src/crates src/apps/cli/src src/web-ui/src
```

Expected: one provider port, one shared document walk, one behavior-version implementation, one store, one core plan/apply owner, and thin surfaces.

- [x] Before any push, spawn a context-isolated subagent to adversarially review the entire diff against the approved design, `AGENTS.md`, architecture boundaries, command-execution safety, atomicity, performance, i18n, and the explicit non-goals. Fix every actionable finding and rerun affected checks.
- [x] Rebase onto latest `gcwing/main`, resolve conflicts without broad rewrites, rerun affected focused checks, and repeat the adversarial diff review if the rebase changes behavior.
- [x] Squash/amend the design, plan, implementation, fixes, and documentation into one accurate final commit. Push only to `limityan/BitFun`; never push directly to `GCWing/BitFun`. Do not wait for CI unless explicitly requested, but do not claim pending/unreported checks passed.

## Completion evidence

The PR is ready only when the handoff can show:

- one CLI import/update transcript proving preview, stale rejection, apply, disable, and remove;
- one Desktop component test proving exact command review and shared backend state;
- one restart/load test proving no external rediscovery or re-import;
- one next-event generation test proving hot reload without changing an in-flight engine;
- one atomic failure test proving the old version remains active;
- one unsafe asset test for links/reparse points and budget rejection;
- no changes to OpenCode execution, Agent Hook scheduling, or the redacted catalog DTO;
- a clean, context-isolated adversarial review after the final diff.

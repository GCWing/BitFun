# Claude Code / Codex command Hook import design

## Status and baseline

Status: approved for implementation planning.

This design is based on the following current sources as of 2026-07-28:

- BitFun `gcwing/main` at `9f705a41c`;
- Codex `openai/codex` `main` at `e597169e9a`;
- OpenCode `anomalyco/opencode` `dev` at `017a5977d2`;
- the current [Codex Hooks reference](https://developers.openai.com/codex/hooks) and
  [Claude Code Hooks reference](https://code.claude.com/docs/en/hooks).

The implementation belongs in one focused PR. It makes the already-discovered,
compatible Claude Code and Codex command Hooks usable in BitFun without waiting
for the JavaScript Plugin Host. It does not implement OpenCode Hook execution or
create a general Hook SDK.

## Problem

BitFun currently has two separate, valid pieces:

1. `bitfun-agent-runtime::native_hooks::AgentHookEngine` executes BitFun's
   Codex-compatible `hooks.json` command handlers. Product wiring in
   `assembly/core/src/native_hooks.rs` reads the user and project files and
   dispatches the eleven supported lifecycle events.
2. The external Hook catalog discovers Claude Code, Codex, and OpenCode Hook
   declarations through ecosystem adapters. It is intentionally read-only and
   redacts handler bodies, commands, environment data, and credentials.

Consequently, `/hooks_external` can tell a user that compatible command Hooks
exist, but using them still requires manually copying and rewriting source
configuration. Manual copying has four concrete problems:

- BitFun cannot distinguish imported declarations from hand-written BitFun
  Hooks, so disabling, updating, or removing one source is unsafe;
- source-relative scripts and product-specific variables may stop resolving;
- unsupported Claude Code fields can be silently misunderstood;
- there is no version-fenced review between discovering arbitrary commands and
  enabling their execution.

The product goal is therefore narrow: let a local user explicitly review,
import, enable, update, disable, and remove the synchronous command-Hook subset
that the existing BitFun engine can execute faithfully.

## Decision summary

The first version uses a **BitFun-managed import snapshot**:

- external files remain read-only and are never mounted as live runtime config;
- an import writes a separate BitFun-owned native `hooks.json` layer plus only
  the source-owned Hook assets that can be copied and rewritten safely;
- discovery and import planning are on demand and asynchronous;
- startup never re-imports a source and no persistent external file watcher is
  installed;
- import/update is an atomic, version-fenced plan/apply operation;
- one source-level switch controls the imported snapshot; removal touches only
  BitFun-owned files;
- a successful mutation invalidates the current Hook engine generation and is
  effective for the next lifecycle event, without changing in-flight Hooks;
- imported command layers reuse the current native Hook dispatch and execution
  path;
- OpenCode remains a static catalog until its JavaScript Hook execution is
  delivered through `PluginRuntimeClient`.

No public concept named "bundle" or "manifest" is introduced. Product surfaces
say "Imported from Claude Code" or "Imported from Codex".

### Alternatives rejected

| Approach | Why it is not v1 |
| --- | --- |
| Merge into the user's existing `hooks.json` | Loses provenance and makes source-level update, disable, and removal unsafe. |
| Read external files directly at runtime | Creates two live truth sources, source-product trust ambiguity, and startup/event-path IO. |
| Managed snapshot (selected) | Adds one small private store, but gives stable review, rollback, and lifecycle behavior without a new runtime. |

## Goals

- Preserve one Hook lifecycle owner and one command execution implementation.
- Make the imported source, scope, exact commands, and skipped items reviewable
  before arbitrary code is enabled.
- Make disable and removal reversible and source-scoped.
- Avoid adding work to ordinary startup and Agent-event hot paths beyond loading
  already-enabled BitFun-owned layers.
- Give CLI/TUI and Desktop the same typed plan/apply and lifecycle operations.
- Fail without changing the last active snapshot when discovery, conversion, or
  persistence fails.

## Non-goals

This PR does not add:

- live mounting or continuous watching of `.claude` or `.codex` files;
- automatic updates after an external source changes;
- OpenCode JavaScript Hook execution;
- Claude Code `http`, `mcp_tool`, `prompt`, or `agent` handlers;
- Claude Code async handlers, `asyncRewake`, `args`, custom `shell`, `if`, or
  component-frontmatter Hooks;
- Codex plugin-bundled or administrator-managed Hooks;
- a generic Hook SDK, HookBus, executor registry, or new plugin protocol;
- remote-workspace Hook execution;
- per-handler editing, reordering, or policy overrides;
- migration of credentials, environment variables, or external product trust
  records.

## Current implementation constraints

### Native command Hooks

The current native owner already supplies the required execution primitive:

- user file: `<user config dir>/config/hooks.json`;
- project file: `<workspace>/.bitfun/config/hooks.json`;
- global master gate: `app.hooks.enabled`;
- manual project-file gate: `app.hooks.project_hooks_enabled`;
- local-only execution, with all Hooks skipped for remote workspaces;
- bounded files, handler count, timeouts, captured output, and model-visible
  output;
- Tokio child processes with JSON stdin and bounded stdout/stderr;
- a per-workspace engine cache rebuilt when configured file fingerprints change.

The cache currently checks manual file metadata at every dispatch and reads a
changed file synchronously while rebuilding. The import design must not add
external source scans or imported-directory enumeration to that event path.

The current engine runs matching handlers sequentially and stops launching later
handlers after a final blocking outcome. Current Codex and Claude Code run all
matching command Hooks concurrently. That is an existing native compatibility
gap, not created by import. This PR must document the actual BitFun ordering but
must not combine import work with a scheduler and outcome-aggregation rewrite.

### External Hook catalog

`ExternalHookCatalogCoordinator` already supplies bounded, coalesced background
discovery with last-valid snapshots. Claude Code, Codex, and OpenCode adapters
own native file discovery and syntax interpretation.

`ExternalHookCatalogSnapshotV1` must remain a safe summary. It must not be
expanded to carry commands simply because import needs them. Import preparation
is a separate, explicit operation with a narrower local consumer.

Its current `content_version` is also deliberately computed from redacted
catalog facts, so changing only a command body does not change that value. It
must remain a catalog-generation guard and must not be reused as an import
update signal. Explicit import preparation computes a separate private
`behavior_version` over the complete normalized import behavior.

### Architecture guardrails

The repository architecture requires one Hook Coordinator. Source adapters may
interpret native declarations, while deadline, cancellation, permission
effects, event ordering, and outcome aggregation remain in the Agent owner. The
first import implementation therefore extends the inputs assembled by the
existing `native_hooks` wiring; it does not create a parallel runtime owner.

## Product behavior

### Unified mental model

`/hooks` and **Settings -> Agent Hooks** become the single management surface:

```text
Agent Hooks
  Active
    BitFun user Hooks
    Imported from Claude Code      Enabled
  Available to import
    Codex project Hooks            3 compatible, 1 skipped
  Needs attention
    Claude Code source changed     Review update
```

The only normal workflow is:

```text
Detect -> review exact commands and skipped reasons -> import and enable
```

Afterwards the user manages the source with `Update`, `Disable/Enable`, or
`Remove`. Import and enable is one confirmed action; there is no second
activation ceremony.

`/hooks_external` remains as a compatibility alias that opens or renders the
external/available section of `/hooks`. It no longer represents a separate
product concept.

### Scope behavior

- User-global Claude Code or Codex configuration imports into a user-global
  BitFun layer and applies to local workspaces.
- Project or local-project configuration imports into a BitFun-managed layer
  associated with that local workspace.
- Import scope follows the source scope and is not user-remappable in v1.
- `app.hooks.enabled` remains the master switch for manual and imported Hooks.
- `app.hooks.project_hooks_enabled` continues to gate only the repository-owned
  `.bitfun/config/hooks.json` file. A reviewed imported workspace snapshot has
  its own source-level enabled state, so importing it does not silently enable
  unrelated repository-owned BitFun Hooks.

### Additive behavior and conflicts

Hook declarations are additive; two different commands on the same event and
matcher are not a name conflict and both remain visible and executable. The
design does not add a main selector or a conflict-resolution UI.

Exact repeated declarations are shown as duplicates in the review, but v1 does
not silently change their execution semantics across independently imported
sources. This avoids choosing an arbitrary provider as owner and matches the
current BitFun layer model. A user can disable the duplicate source as one unit.

## Import compatibility

### Codex

Supported sources:

- user and project `hooks.json`;
- user and project inline `[hooks]` tables in `config.toml`.

Supported declarations are the eleven event names already implemented by
`AgentHookEvent`, with `type = "command"` handlers containing only fields the
BitFun engine currently honors:

- `command`;
- `commandWindows` / `command_windows`;
- `timeout`;
- `statusMessage`.

`prompt`, `agent`, async execution, `additionalContextLimit`, plugin-bundled
Hooks, managed Hooks, and unknown behavior-affecting fields are skipped with an
explicit reason. Codex trust and per-Hook enablement records are not copied;
confirmation of the exact BitFun import plan is a new BitFun-local decision.

### Claude Code

Supported sources:

- `~/.claude/settings.json`;
- `<workspace>/.claude/settings.json`;
- `<workspace>/.claude/settings.local.json`.

Conversion follows the conservative subset used by Codex's current Claude Hook
migration:

- a missing handler `type` is normalized to `command`;
- `timeoutSec` is normalized to `timeout`;
- synchronous command handlers with `command`, optional timeout, and optional
  `statusMessage` are eligible;
- group-level or handler-level `if`, unknown group fields, `async`,
  `asyncRewake`, `args`, `shell`, `once`, and non-command handler types are
  skipped;
- `disableAllHooks: true` is respected and produces no eligible import.

Only events implemented by BitFun are considered. Newer Claude-only events such
as `Setup`, `PostToolUseFailure`, `Notification`, task/team events, worktree
events, file/config change events, and MCP elicitation events remain visible in
the static catalog but are not importable.

### Commands and source-owned assets

Import does not attempt to parse arbitrary shell syntax or emulate another
product's complete environment.

- Commands without a known source-relative Hook path are preserved verbatim.
- Statically recognizable references under the source's `hooks/` directory may
  be copied into the managed snapshot and rewritten to that copied location.
- Only regular files inside the source Hook directory are copied. Links,
  reparse points, escapes outside the source root, unreadable files, and copies
  exceeding fixed implementation budgets make the affected handler ineligible.
- Dynamic source-root expressions that cannot be rewritten safely are skipped.
- Absolute dependencies outside the source Hook directory are not copied; they
  remain explicit external dependencies in the review.
- BitFun does not inject `CLAUDE_*` or `CODEX_*` compatibility variables and
  never copies credentials or environment values.

The copied asset budgets are internal safety constants, not user-facing
configuration. V1 uses at most 256 regular files, 1 MiB per file, 16 MiB total,
and eight directory levels. These match the existing managed-package byte
budgets where applicable and must be covered by tests.

## Persistent representation

Imported Hooks are product-owned runtime data, not user-authored configuration
and not repository content.

User-global imports live below:

```text
PathManager::user_data_dir()/hook-imports/
```

Workspace imports live below:

```text
PathManager::project_runtime_root(workspace)/hook-imports/
```

Each root contains one private `index.json` and versioned bundle directories:

```text
hook-imports/
  index.json
  bundles/<import-id>/<behavior-digest>/
    hooks.json
    hooks/...
```

The private index stores only fields needed by a real consumer:

- schema version and a generation token that increments while the index is
  valid and is freshly reseeded after explicit corrupt-store reset;
- stable import id, provider id, source key, source kind/scope, and the
  catalog's redacted display/location facts needed after restart;
- imported private behavior version and a digest of the exact managed
  `hooks.json` plus asset bytes;
- active bundle version and enabled state.

There is no public import file format and no compatibility promise for the
directory layout. Product surfaces consume typed snapshots and actions, never
read the index directly.

### Atomic writes

Import and update use the following order:

1. prepare and validate the complete new bundle in a sibling temporary
   directory without touching the indexed path;
2. parse its `hooks.json` with the existing `AgentHookSettings` parser;
3. move it to a new immutable version directory; when repairing the same
   behavior path, first retire the old directory and restore it if the final
   rename fails;
4. atomically replace `index.json` so it points at the new version and increments
   generation; if index publication returns an error, remove the newly published
   bundle and restore the retired indexed directory before releasing the store lock;
5. publish the new in-memory registry snapshot;
6. remove only the old unreferenced digest directory on a best-effort cleanup
   path while the same per-store lock is still held.

If steps 1-3 fail, the old index and active bundle are unchanged. A handled step-4
failure rolls the bundle path back to match the old index. A process crash between
bundle and index publication can leave residue or a digest mismatch; bounded
verification on the next open fails that imported layer closed. If old-version
cleanup fails after step 4, the old directory is inactive residue; it is never
reactivated implicitly and can be removed by a later maintenance pass.

Disable changes only the indexed enabled state. Remove first removes the index
record and publishes the new generation, then deletes only the digest directory
named by that removed record while the same per-store lock is held. It never
deletes a sibling version that another process may have published, and never
edits or deletes the Claude Code/Codex source. A removed source can be
re-imported later.

## Typed operations and ownership

### Contracts

The Hook domain adds the minimum plan/apply DTOs needed by CLI and Desktop:

- `ExternalHookImportPlanV1` with source identity/version, current disposition,
  exact eligible command summaries, skipped items with reason codes, and a plan
  fingerprint;
- `ExternalHookImportApplyRequestV1` with source key and accepted fingerprint;
- `ExternalHookImportApplyResultV1`, returning applied, unchanged, or stale with
  a refreshed plan;
- `ImportedHookSourceSnapshotV1` and closed enable/disable/remove actions, plus
  a corrupt-store-only reset action scoped to the affected user/workspace
  store.

The management snapshot embeds the existing redacted catalog and exposes one
combined revision over the user and workspace stores. Mutations fence that
revision instead of inventing separate surface actions for each store. A stale
mutation is never replayed automatically: Desktop refreshes its visible state,
while CLI/TUI reports staleness and requires an explicit refresh/retry. The user
must trigger enable, disable, remove, or reset again against the new revision.
Corrupt reset chooses a fresh non-zero generation token so a pre-reset plan or
mutation cannot become valid again through ABA.

Command-bearing DTOs require redacted `Debug` implementations and must never be
written to logs or exposed through remote/peer APIs. Exact commands are shown
only because local users must review the code BitFun is about to execute.

The existing catalog DTO remains unchanged and redacted.

Interface stability is protected by three limits:

- the public serialized surface contains one versioned plan, apply request and
  result, imported-source snapshot, and one closed source mutation action;
- provider-to-assembly prepared data is non-serialized and has redacted
  `Debug`; it does not become a user configuration format;
- there is no generic import manager, import registry, Hook executor trait, or
  second management state model shared with MCP merely because both use a
  review/apply interaction.

### Source adapters

`ExternalHookSourceProvider` gains a default `prepare_import` operation keyed by
source key and expected redacted catalog version. The default result is
unsupported, so OpenCode and future catalog-only providers do not acquire
execution behavior. The Claude Code and Codex adapters implement it by rereading
the selected source and producing a prepared native layer, a private
`behavior_version`, and asset copy facts.

Both implementations extend the existing `static-hook-support` document walk.
The current redacted `parse_hook_document` remains a compatibility wrapper over
that single walk; import conversion receives borrowed group/handler objects
through a closure and does not introduce a second JSON/TOML traversal or copy
raw handler values into the catalog result.

Adapters own only native syntax, source precedence, conservative conversion,
and rewrite diagnostics. They do not write BitFun state or enable execution.

The OpenCode provider keeps the default unsupported preparation result. Its
JavaScript callbacks are not converted into commands.

### Durable store and assembly

- `services-integrations` owns a small `HookImportStore`: containment checks,
  bounded asset persistence, index/bundle persistence, and cleanup. It reuses
  `services-core::JsonFileStore::write_atomic_strict` for the index instead of
  adding another atomic JSON writer.
- `assembly/external-sources` continues to own coalesced Hook discovery and the
  current provider generation.
- a focused `assembly/core::external_hook_import` module follows the existing
  external MCP import plan/apply pattern: build plan, re-prepare on apply,
  compare fingerprints, call the store, and return typed outcomes.
- `assembly/core::native_hooks` remains the product Hook dispatch owner. It adds
  enabled imported paths to the current native layer list and includes the
  import generation in its engine cache key.
- `bitfun-agent-runtime` keeps the same parser, engine, payload, and outcome
  types; it receives ordinary `AgentHookSettingsLayer` values and remains free
  of filesystem/import/provider dependencies.

This reuses plan/apply and generation-fencing logic structurally without
sharing MCP-specific data types or adding Hook to the generic external-source
control object.

The immutable bundle directory is named from the private behavior digest, not
the accepted plan fingerprint. A separate private content digest covers the
exact generated `hooks.json`, sorted relative asset paths, and asset bytes. This
keeps the final managed asset path known before the review DTO is rendered,
avoids a fingerprint that contains its own path, and lets a restart reject a
syntactically valid but modified snapshot. The plan fingerprint covers the
fully materialized commands, behavior digest, source/catalog guard, and current
target generation; it is not persisted.

## Runtime loading and performance

### Startup

Process startup never discovers Claude Code/Codex sources and never re-imports
them. On first access to an import store, and only after its index metadata
changes, BitFun verifies the content of the exact indexed bundle paths against
their bounded private digests. It does not enumerate unreferenced version
directories or external product files. Normal Hook events check only index
metadata and reuse the existing `AgentHookEngine` until the generation changes.

If the import index is unreadable or invalid, imported Hooks fail closed and a
diagnostic is surfaced; manual BitFun Hooks continue to work. Recovery does not
delete data automatically. A destructive reset is offered only from the error
state and requires explicit confirmation.

### Discovery and update checks

External discovery runs only when:

- the user opens `/hooks` or Agent Hooks settings;
- the user explicitly refreshes;
- an import/update operation needs a fresh plan.

It reuses the current `ExternalHookCatalogCoordinator` discovery lane, so
simultaneous Desktop/TUI requests coalesce and blocking filesystem reads remain
off the UI thread. Concurrent preparation for the same source/version also
coalesces. Closing the view discards late UI results and does not leave a poller
or watcher behind.

Catalog refresh alone produces only source presence and redacted structure. For
an already imported source, the management refresh asynchronously calls guarded
import preparation and compares its private `behavior_version` with the index.
That produces `current`, `update available`, `source missing`, or `update check
failed`. This work runs only in the management/explicit-refresh path and is
coalesced per source. A changed source never changes execution until the user
reviews and applies a new plan.

### Runtime hot reload

Import, update, enable, disable, and remove atomically publish a new import
generation. `native_hooks::engine_for` includes that generation and the known
manual-file fingerprints in its cache comparison. The next lifecycle event
rebuilds the engine once and reads the new immutable imported layer set.

An event that already captured an `Arc<AgentHookEngine>` completes with that
engine. In-flight child processes are neither killed nor reconfigured by a
management action.

External file edits are not runtime hot reload. They appear as `update
available` after an on-demand refresh.

## Layer order and permission behavior

Native layers are assembled in deterministic order:

1. manual user `hooks.json`;
2. enabled user-global imported sources ordered by stable import id;
3. manual project `.bitfun/config/hooks.json`, when its existing gate is on;
4. enabled imported workspace sources ordered by stable import id.

The order is visible in `/hooks`. The implementation does not add source
priority editing.

Imported Hooks have exactly the current BitFun command-Hook authority:

- they execute as the local BitFun user;
- a Hook can narrow permission but cannot widen a rule-based denial;
- modified tool input must continue through the owning validation and permission
  path;
- post-tool feedback cannot undo a completed side effect;
- remote workspaces skip all Hook dispatch rather than running locally against a
  remote path.

No import action establishes plugin, MCP, tool, or OpenCode execution approval.

## CLI and Desktop surfaces

### Interactive CLI/TUI

`/hooks` becomes asynchronous and renders the last current native/import state
immediately while external discovery refreshes in the background. It supports:

```text
/hooks
/hooks refresh
/hooks import <source-number> [--confirm]
/hooks update <import-number> [--confirm]
/hooks enable <import-number>
/hooks disable <import-number>
/hooks remove <import-number> --confirm
/hooks reset <user|project> --confirm
```

Import/update first renders:

- source and scope;
- every exact command that will execute, with effective timeout and Windows
  override where applicable;
- copied or external asset dependencies;
- skipped event/handler count and concise reason for each class;
- the plan fingerprint.

The existing interactive confirmation accepts that exact plan. Terminal text is
escaped through the existing CLI diagnostic helpers.

### Root CLI

For scripting and non-interactive use, add the matching root command family:

```text
bitfun hooks list [--refresh] [--format text|json]
bitfun hooks import --source <source-key> [--confirm <plan-fingerprint>]
bitfun hooks update <import-id> [--confirm <plan-fingerprint>]
bitfun hooks enable <import-id>
bitfun hooks disable <import-id>
bitfun hooks remove <import-id> --confirm
bitfun hooks reset <user|project> --confirm
```

Without `--confirm <plan-fingerprint>`, import is preview-only. Non-interactive
execution never approves the plan it just discovered. A stale confirmation
returns the refreshed plan and performs no write.

Reset is exposed only for explicit recovery of a corrupt BitFun-managed scope.
It requires confirmation and leaves the Claude Code/Codex source intact.

### Desktop

The current **Agent Hooks** settings page keeps its two existing global/manual
project switches and adds one compact imported-source section. It uses the same
plan/apply APIs as CLI and does not parse source files in React.

The review dialog shows exact commands and skipped reasons before import or
update. Enable/disable is immediate; remove confirms that only the BitFun copy
will be deleted and the source application is unchanged.

All new copy uses the owning settings namespace and the repository i18n flow.

## Failure behavior

| Failure | Behavior |
| --- | --- |
| External discovery fails | Keep current imports active; show stale/failed discovery. |
| Source changes after review | Return `stale` with a refreshed plan; write nothing. |
| Import state changes during enable/disable/remove/reset | Refresh visible state, report stale, and require the user to trigger the action again. |
| Source is invalid or has no compatible handlers | Show skip reasons; do not create an empty import. |
| Asset cannot be copied safely | Skip the affected handler; never follow an escaping link. |
| Bundle validation or persistence fails | Keep the previous active version and generation. |
| Indexed `hooks.json` or asset content no longer matches its digest | Fail that imported bundle closed; update republishes the reviewed snapshot. |
| Enable finds a missing/corrupt bundle | Stay disabled and require update or removal. |
| Source disappears later | Continue using the snapshot; show `source missing`; allow disable/remove. |
| Old bundle cleanup fails | New state remains authoritative; inactive residue is never selected again. |
| A managed path ancestor is a link or Windows reparse point | Reject the operation without reading or deleting through it. |
| Import index is corrupt | Disable imported layers only; manual Hooks continue; offer explicit reset. |
| Corrupt index is explicitly reset | Publish an empty index with a fresh generation token; pre-reset plans remain stale. |
| Workspace is remote | Return a clear unsupported state; never execute locally as a fallback. |

## OpenCode consistency boundary

Claude Code and Codex command Hooks become native layers before runtime, so they
reuse the existing Hook lifecycle dispatch and command backend. OpenCode Hooks
are JavaScript callbacks that can mutate input/output objects and receive an
OpenCode plugin context; they require the plugin execution domain.

The consistent architecture is therefore:

```text
Claude/Codex source adapters --explicit import--> native command layers
                                                   |
BitFun native hooks.json ---------------------------+--> current native_hooks owner
                                                   |      + AgentHookEngine
OpenCode adapter --future approved declarations----+--> future PluginRuntimeClient backend
```

Consistency means one lifecycle owner, one permission revalidation path, and
one aggregate outcome policy. It does not mean one source schema or one physical
executor.

The PR does not expand `ExternalHookPoint` beyond the events consumed by a real
OpenCode runtime, does not add a generic executor trait in anticipation of that
runtime, and does not claim the existing OpenCode catalog is executable.

## Verification

Minimum automated coverage:

- adapter fixtures for Codex JSON/TOML and Claude user/project/local sources;
- table-driven eligible/skipped cases for every supported and explicitly
  unsupported field/type/event class;
- path rewrite, link/reparse escape, copy-budget, and containment tests on the
  platforms where those rules differ;
- plan fingerprint and stale-apply tests;
- atomic import/update failure tests proving the previous bundle remains active;
- enable, disable, remove, missing source, missing bundle, and corrupt index
  lifecycle tests;
- engine layer-order and generation-invalidation tests;
- remote-workspace skip tests;
- CLI text/JSON contract tests and interactive command tests;
- Desktop API serialization, remote policy declaration, component behavior,
  accessibility, and i18n checks.

The implementation PR should run the smallest matching checks, at minimum:

```text
cargo test -p bitfun-agent-runtime native_hook
cargo test -p bitfun-external-sources hook
cargo test -p bitfun-codex-adapter hook
cargo test -p bitfun-claude-code-adapter hook
cargo test -p bitfun-cli hooks
cargo check --workspace
node scripts/check-core-boundaries.mjs
pnpm run i18n:contract:test
pnpm run type-check:web
```

Documentation updates in the implementation PR:

- update `docs/features/agent-hooks.md` and its Chinese counterpart with import,
  lifecycle, no-live-sync, and actual ordering behavior;
- update CLI help and Settings copy;
- update architecture documents only where they currently say Codex/Claude are
  reference-only, without restating this entire feature design.

## Acceptance criteria

The slice is complete when all of the following are true:

1. A local CLI user can discover a Claude Code or Codex source, review exact
   compatible commands and skipped reasons, confirm one version-fenced plan,
   and have the imported source run on the next matching BitFun Hook event.
2. The same imported source and actions are visible in Desktop Agent Hooks
   settings through the same backend state and operations.
3. Disable takes effect on the next event; remove deletes only BitFun-owned data;
   re-import does not duplicate the source record.
4. Restart loads enabled snapshots without rediscovering or re-importing the
   external products.
5. Editing an external source does not change runtime behavior until a reviewed
   update is applied.
6. A stale plan, failed write, unsafe asset, corrupt bundle, or remote workspace
   cannot silently execute new code.
7. OpenCode Hook execution and all runtime-dependent handler kinds remain
   unchanged and explicitly unsupported.

## Scope stop conditions

The implementation must stop and return to design review if it begins to
require any of the following:

- a general-purpose Hook SDK or third-party executor registry;
- JavaScript module loading or Plugin Host lifecycle changes;
- persistent external file watchers;
- migration of secrets, permissions, or foreign trust databases;
- remote Hook execution;
- per-handler editing or source-priority policy;
- changes to Agent loop scheduling unrelated to loading imported layers.

These are separate capabilities, not prerequisites for the command-Hook import
user outcome.

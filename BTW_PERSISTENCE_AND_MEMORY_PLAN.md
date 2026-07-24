# /btw Persistence and Memory Plan

## 1. Background

`/btw` currently creates a transient child conversation. The frontend marks the
session as `isTransient: true` and `sessionKind: 'btw'`; the backend creates an
`EphemeralChild` session. `EphemeralChild` is intentionally excluded from
session persistence, so closing the desktop window makes an active `/btw`
conversation unavailable.

This is unsuitable when a user turns a `/btw` discussion into a real task. The
conversation must remain available after restart, while still being presented
as a child of the parent conversation rather than as an unrelated root session.

At the same time, a persistent `/btw` conversation must not silently become a
source of global memories. Its inclusion in the memory-generation lifecycle is
an explicit user configuration, disabled by default.

## 2. Goals

1. Persist every newly created `/btw` conversation and restore it after an app
   restart.
2. Preserve the initial parent-context fork, model, mode, prompt-cache, and
   constraints used by the current `/btw` flow.
3. Identify persistent BTW children structurally, so the UI can restore their
   nested relationship with the parent.
4. Default persistent BTW children to excluded from memory generation, even
   when normal-session memory generation is enabled.
5. Provide a Memory settings switch that lets a user opt new BTW conversations
   into the ordinary Phase 1 and Phase 2 memory-generation lifecycle.

## 3. Non-goals

1. Do not change the behavior of existing ordinary sessions.
2. Do not migrate, repair, or reconstruct parent relationships for first-generation
   BTW records.
3. Do not add a remote-workspace restriction to an existing remote `/btw`
   workflow. Persistence must preserve its current remote behavior.
4. Do not couple "generate a new memory from this session" with "inject
   existing global memory into this session". They are separate policies.

## 4. Target Session Model

New `/btw` sessions will be runtime `SessionKind::Standard` sessions with a
structured relationship:

```text
session_kind: Standard
relationship:
  kind: Btw
  parent_session_id: <parent session>
  parent_request_id: <originating request, when available>
  parent_dialog_turn_id: <originating turn, when available>
  parent_turn_index: <originating turn index, when available>
tags: ["btw"]
```

`Standard` is required for normal persistence, message updates, cancellation,
model changes, transcript reload, and memory-mode storage. `relationship.kind =
Btw` retains the product meaning that was previously inferred from the
ephemeral session kind and/or legacy metadata.

The relationship is the source of truth. The `btw` tag is only a convenient
index/display hint and must not be used to classify structured and legacy BTW
sessions as equivalent.

## 5. Creation and Runtime Flow

The `/btw` command must continue to enter through the dedicated backend flow,
not through a generic frontend `createSession` followed by a normal send. The
current backend path forks the parent context before starting the first turn;
creating an empty persistent session in the frontend would lose that snapshot.

The new backend flow is:

1. Resolve the parent session and capture the same context snapshot currently
   used by `ensure_hidden_btw_session` / `start_hidden_btw_turn`.
2. Create a persistent `Standard` child session.
3. Write `relationship.kind = Btw` and its parent linkage before the first
   user turn is started.
4. Determine and persist the session's `memory_mode` as described in section
   7.
5. Persist the inherited context snapshot, then start the first BTW turn.
6. Route subsequent turns through the normal persistent-session send, cancel,
   model-update, and persistence paths.

The frontend API may retain BTW-specific names, but it must stop creating an
`isTransient` session. Its returned session identifier is a normal durable
session identifier.

## 6. Restore and UI Behavior

On startup, the session loader must retain structured BTW children and the
session navigation must group them under `relationship.parent_session_id`.

- When the parent exists, opening the child restores it in the existing BTW
  auxiliary panel.
- When the parent is missing or archived, opening the child falls back to an
  independent session view rather than hiding an otherwise valid task.
- Remove the legacy BTW hiding predicate. First-generation records are not
  recognized as BTW children and are not migrated; the ordinary session loader
  may show them as independent root sessions. They have no supported parent
  placement or auxiliary-panel behavior.

## 7. Memory Policy

### 7.1 Configuration

Add this field to `MemoriesConfig`:

```text
memories.generate_for_btw_sessions: false
```

It appears as a switch in the Memory settings page, adjacent to the global
"Generate memories" switch. The setting is effective only when the global
`memories.generate_memories` switch is also enabled.

The field must be aligned in all configuration surfaces:

1. Rust `MemoriesConfig`, serde default, and config persistence/default-pruning
   tests.
2. Frontend `MemoriesConfig` type and config fallback/default handling.
3. Memory settings UI, locale strings, and settings search/index metadata.

Because the Rust config is serde-defaulted, existing config files that lack the
new field resolve safely to `false`.

### 7.2 Source Eligibility

`SessionMetadata.memory_mode` is the durable source-eligibility contract. Phase
1 currently only extracts sessions that are both `SessionKind::Standard` and
`SessionMemoryMode::Enabled`.

When a persistent BTW child is created, set its mode as follows:

| Global `generate_memories` | `generate_for_btw_sessions` | BTW `memory_mode` |
| --- | --- | --- |
| false | false or true | `Disabled` |
| true | false | `Disabled` |
| true | true | `Enabled` |

This decision is stored with the newly created session rather than only
skipping the BTW completion hook. Memory Phase 1 scans historical sessions each
time it starts; a startup-only guard would allow a disabled BTW to be extracted
later when an unrelated normal session starts a scan.

The setting applies to conversations created after the setting is chosen. It
does not retrospectively enable previously excluded BTW transcripts. This
preserves the user's original choice not to contribute that task to global
memory. A future product request can add an explicit per-session migration or
selection action if retroactive enrollment is wanted.

### 7.3 Existing-memory Injection

`memories.use_memories` controls injection of the existing consolidated memory
summary into prompts. It is currently a global prompt-building decision and is
not session-kind aware.

Recommended initial policy: a persistent BTW child continues to read existing
memory whenever `use_memories` is enabled. This is useful for a task
continuation and does not make the BTW transcript a new memory source.

If product policy requires a fully isolated BTW environment, add a separate
future flag such as `memories.use_for_btw_sessions`, default `false`. Do not
reuse `generate_for_btw_sessions` for this purpose, because the two data flows
have different privacy and product semantics.

## 8. Remote Workspace Support and Audit

`/btw` already works in remote workspaces. The BTW child inherits the parent
`SessionConfig`, and each turn continues to pass the session's
`remote_connection_id` and `remote_ssh_host` into the normal dialog-turn path.
Remote sessions also have a dedicated local mirror storage path for their
session metadata and transcripts.

The desktop command-policy registry currently marks `btw_ask_stream` and
`btw_cancel` as `LegacyUnaudited`. That is a classification backlog, not a
runtime rejection or a claim that remote BTW is unsupported.

The persistence change must preserve this existing capability:

1. Create the durable BTW child from the inherited remote-aware `SessionConfig`.
2. Resolve and store it through the same remote-session mirror path as the
   parent, retaining remote connection identity across restart.
3. Add focused remote regression coverage for create, restart/restore, resume,
   and cancel.
4. Audit the two desktop handlers with this flow and then promote
   `btw_ask_stream` and `btw_cancel` to `RemoteRouted` in the command-policy
   registry.

No explicit unsupported state or remote-only feature gate is part of this plan.

## 9. Implementation Areas

| Area | Main files | Change |
| --- | --- | --- |
| Frontend BTW entry | `src/web-ui/src/flow_chat/services/BtwThreadService.ts` | Stop requesting a transient BTW session and consume the durable child result. |
| Desktop API | `src/apps/desktop/src/api/btw_api.rs` | Preserve the command contract while calling the persistent BTW coordinator path. |
| Coordinator | `src/crates/assembly/core/src/agentic/coordination/coordinator.rs` | Replace ephemeral BTW child construction with durable Standard creation, context fork, relationship, and memory-mode selection. |
| Session persistence | `src/crates/assembly/core/src/agentic/session/session_manager.rs` and `src/crates/assembly/core/src/agentic/persistence/manager.rs` | Persist and reload the structured BTW child without changing ordinary-session behavior. |
| Shared session contract | `src/crates/services/services-core/src/session/types.rs` and related metadata helpers | Reuse `SessionRelationshipKind::Btw`; do not add a second BTW classification format. |
| Session navigation | `src/web-ui/src/app/components/NavPanel/sections/sessions/SessionsSection.tsx` | Restore/group structured BTW children; use orphan fallback. |
| Frontend metadata parsing and loading | `src/web-ui/src/flow_chat/utils/sessionMetadata.ts` and `src/web-ui/src/flow_chat/store/FlowChatStore.ts` | Use `relationship.kind = Btw` as the only BTW classifier; remove `isLegacyPersistedBtwSession` and both metadata-load skip branches. |
| Memory configuration | `src/crates/assembly/core/src/service/config/types.rs`, config manager, frontend config types, and `MemoriesConfig.tsx` | Add the default-off BTW source-generation switch and UI/i18n plumbing. |
| Memory source selection | `src/crates/assembly/core/src/agentic/memories/service.rs` | Continue honoring durable `memory_mode`; add an explicit test proving an excluded BTW is not claimed during a later ordinary scan. |
| Remote BTW audit | `src/apps/desktop/src/api/remote_workspace_policy.rs`, BTW API, and remote-session tests | Preserve inherited remote identity, verify the persistent flow, then promote the BTW commands to `RemoteRouted`. |

## 10. Test Plan

### Rust

1. Config defaults and deserialization: missing
   `generate_for_btw_sessions` resolves to `false`; non-default values persist
   and reload correctly.
2. BTW creation: durable child is `Standard`, carries `Btw` relationship and
   parent linkage, and retains the parent-context snapshot.
3. Memory mode matrix: verify all three effective cases in section 7.2.
4. Restart/reload: child transcript and relationship survive persistence.
5. Phase 1 candidate selection: a BTW created while the new flag is off is
   never claimed, including during a memory run started by another session.
6. Remote persistence: create a BTW in a remote workspace, restart and restore
   it, resume/cancel it, and confirm that its remote identity and mirror
   storage remain intact. Promote the two BTW commands to `RemoteRouted` after
   that audit passes.

### Frontend

1. New BTW session is not transient and can send further messages through the
   normal session path.
2. Structured BTW child restores beneath its parent and opens in the auxiliary
   panel.
3. Orphaned structured BTW opens independently.
4. Memory settings switch reads, writes, and renders its default-off state.

### Commands after implementation

Run the narrow tests that cover the changed Rust modules and Web UI behavior,
then at minimum run:

```text
pnpm run type-check:web
cargo check --workspace
```

Add the focused frontend and Rust test commands to the implementation handoff
only after their exact test locations are finalized.

## 11. Rollout and Compatibility

The feature is forward-only. New BTW sessions use structured relationships and
durable session storage; historical transient BTW sessions do not have data to
restore. First-generation persisted BTW-shaped records receive no compatibility
handling: the implementation removes their legacy hide/recognition branches and
does not migrate their tag/custom-metadata relationship into the new structured
format. Any resulting ordinary-session loading is incidental and has no
compatibility test, parent placement, or BTW auxiliary-panel guarantee.

The memory setting defaults to closed for both new users and existing config
files. This permits persistent BTW task recovery without widening the set of
transcripts that can contribute to global memory.

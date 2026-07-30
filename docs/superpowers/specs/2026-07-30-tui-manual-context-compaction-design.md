# TUI Manual Context Compaction Design

**Status:** Approved for implementation by the 2026-07-30 request to complete PR2 and combine it with the existing TUI session/status work.

## Goal

Expose BitFun's existing manual context-compaction capability through both Embedded and Shared TUI without duplicating compression policy or weakening session ownership, cancellation, persistence, or audit semantics.

The user entry points are `/compact` and the OpenCode-compatible alias `/summarize`. No BitFun-specific synonym or new shortcut is introduced.

## Current State

- Core already performs manual compaction as a persisted `ManualCompaction` maintenance turn and emits context-compression plus dialog-turn lifecycle events.
- Desktop already calls `ConversationCoordinator::compact_session_manually`.
- The CLI action registry has no manual-compaction action.
- `AgentRuntime`, runtime ports, and private Shared TUI IPC do not expose manual compaction.
- The Shared TUI protocol is version 7 and its documented closed operation set does not include compaction.
- Manual compaction currently waits for the model operation inline and is not registered in the same active-turn, settlement, and cancellation lifecycle used by TUI dialog turns.

## Competitor Compatibility

OpenCode uses `/compact` as the primary command and `/summarize` as an alias. Codex also exposes `/compact` as a dedicated runtime operation instead of sending it as an ordinary model prompt.

BitFun will match those command names and dedicated-operation semantics. It will retain BitFun's current idle-only admission rule rather than introducing Codex-style queuing. OpenCode's `Ctrl+X C` shortcut is intentionally deferred because BitFun's keymap currently supports a single key chord; inventing a different shortcut or adding leader sequences would be unrelated scope.

## Considered Approaches

### 1. Send `/compact` as a normal prompt

Rejected. This would make compression depend on model interpretation, pollute model-visible context, and bypass the existing maintenance-turn audit path.

### 2. Call Core directly from the CLI

Rejected. Embedded and Shared TUI would diverge, the CLI would depend on the concrete coordinator, and cancellation/disconnect ownership would remain incomplete.

### 3. Add one narrow runtime-owned compaction capability

Selected. A typed runtime port starts the existing Core maintenance operation, and the private Shared IPC projects exactly that capability to its only consumer, the first-party TUI.

## Architecture

```text
/compact | /summarize
        |
        v
CLI Action Registry
        |
        v
CliAgentRuntimeClient
   |                 |
   | Embedded        | Shared
   v                 v
AgentRuntime     Runtime IPC v8
   \                 /
    v               v
AgentSessionCompactionPort
        |
        v
ConversationCoordinator
        |
        v
ExecutionEngine compaction plan -> cancellation gate -> atomic commit tail
        |
        v
Authoritative events and persisted ManualCompaction turn
```

### Runtime contract

`bitfun-runtime-ports` adds:

```rust
pub struct AgentSessionCompactionRequest {
    pub session_id: String,
    pub turn_id: String,
}

pub struct AgentSessionCompactionResult {
    pub session_id: String,
    pub turn_id: String,
}

#[async_trait::async_trait]
pub trait AgentSessionCompactionPort: Send + Sync {
    async fn start_session_compaction(
        &self,
        request: AgentSessionCompactionRequest,
    ) -> PortResult<AgentSessionCompactionResult>;
}
```

The caller supplies a stable turn ID. This lets Shared IPC record the provisional active turn before executing the side effect, preserving disconnect cancellation and outcome-unknown handling.

`AgentRuntime` stores the optional port, exposes `start_session_compaction`, and returns a typed `NotAvailable` error when a product assembly does not register it.

### Core lifecycle

The coordinator splits manual compaction into start and completion:

1. Validate exact session/turn identities and require an idle, context-loaded session.
2. Atomically admit the persisted maintenance turn with the caller-provided turn ID through the same mutation lock used by ordinary dialog turns.
3. Read the authoritative context only after the maintenance turn owns the Session, so a racing dialog turn cannot be omitted.
4. Register active-session execution, exact turn settlement, a cancellation token, and a manual-compaction commit gate.
5. Emit `DialogTurnStarted` and return the accepted turn identity immediately.
6. Run compression in an owned task.
7. Persist exactly one terminal status and return the in-memory session to idle only when the owned task settles.

The existing synchronous Desktop compatibility method starts the same task and awaits its private completion receiver. It does not create a second execution path.

### Cancellation and commit safety

Manual compaction has two phases:

- **Planning:** cancellation wins through an atomic planning/cancelled transition and cancels the model future. The maintenance turn is persisted as cancelled.
- **Committing:** the compaction plan has already been accepted for context replacement. Cancellation no longer clears session state; the commit tail finishes and reports completion.

The transition uses a small atomic gate shared by the coordinator and execution engine. A compare-and-swap prevents cancellation and commit from both winning. The gate is registered only for manual compaction and removed when the task settles.

This avoids a state where the session appears idle while context replacement or persistence continues.

The maintenance Turn is model-invisible but transcript-visible. Transcript reads project canonical persisted Turn records rather than reconstructing the maintenance entry from model context, retaining exact compression/tool identity and the `applied` fact after restart. If context commit succeeds but terminal Turn or idle-state persistence fails, the finalizer emits one explicit failed dialog terminal and returns an error that states the compaction was already applied; it never leaves Shared TUI waiting on a missing terminal event.

### Shared IPC

The private protocol moves from version 7 to version 8 and adds:

```rust
RuntimeIpcOperation::CompactSession {
    request: AgentSessionCompactionRequest,
}
```

Rules:

- current controller only;
- current session must be idle;
- side-effecting;
- supplied turn ID becomes the provisional active turn;
- success reuses `RuntimeIpcOperationResult::TurnAccepted`;
- disconnect and explicit cancel reuse the existing turn-cancellation operation.

No Server, Relay, Peer, ACP, SDK Host, or public wire protocol is changed.

### TUI behavior

- `/compact` is the primary entry; `/summarize` resolves to the same action.
- Both are shown only in chat/startup surfaces where the existing action projection permits session operations.
- Extra arguments are rejected with `Usage: /compact`.
- The action is idle-only in both Embedded and Shared modes.
- Acceptance shows immediate status while authoritative events drive processing state.
- Compression events are projected into the existing `ContextCompression` tool-card presentation, so live execution and restored transcript use the same visual vocabulary.
- Completion shows token reduction and source facts already carried by the event; failure and cancellation reuse existing turn terminal handling.

## Error Handling

- Missing runtime port: typed not-available error, surfaced in the TUI status line.
- Busy/error session: Core and IPC both fail closed; the action registry prevents the normal busy invocation path.
- Duplicate or invalid turn ID: rejected before starting a second maintenance turn.
- Shared request timeout after side-effect admission: existing outcome-unknown disconnect handling applies because the provisional turn ID is known.
- Event stream failure: existing CLI active-turn cancellation and embedded handoff guidance applies.
- Cancellation after commit wins: operation completes; no false cancelled terminal state is emitted.
- Post-commit terminal persistence failure: the Session returns to idle in memory, emits one failed terminal event, and reports that context replacement was already applied.

## Scope Exclusions

- Relay and remote protocol changes
- Extension or customization behavior
- Compression prompt, algorithm, automatic threshold, or artifact-schema changes
- Web UI behavior changes
- Public Server/API/SDK Host exposure
- Generic maintenance-command framework
- Busy-session queueing
- New keyboard shortcut or leader-sequence support

## Verification

- Runtime-port DTO and runtime forwarding tests
- Atomic cancellation/commit gate tests
- Core accepted-turn and terminal-settlement tests using existing coordinator fixtures where practical
- Atomic dialog/maintenance admission, post-commit failure finalization, and persisted transcript restoration tests
- IPC v8 serialization, rules, provisional active-turn, and disconnect-cancellation tests
- CLI action alias/availability/argument tests
- Embedded/Shared runtime-client equivalence tests
- TUI context-compression projection tests
- `cargo test -p bitfun-runtime-ports`
- `cargo test -p bitfun-agent-runtime`
- `cargo test -p bitfun-agent-runtime-ipc`
- `cargo test -p bitfun-cli`
- `cargo check -p bitfun-core --features product-full`
- `node scripts/check-core-boundaries.mjs`
- `git diff --check`

## Delivery

This work is combined with the existing TUI session aliases and truthful context-status changes in one PR. The final branch is rebased on current `gcwing/main`, reviewed as one diff, and squashed to one commit before pushing to `limityan/BitFun`.

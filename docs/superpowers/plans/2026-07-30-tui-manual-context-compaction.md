# TUI Manual Context Compaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add OpenCode-compatible `/compact` and `/summarize` commands to Embedded and Shared TUI through one runtime-owned, cancellable, audited manual-compaction lifecycle.

**Architecture:** Add one stable session-compaction port to `bitfun-runtime-ports`, inject it into `AgentRuntime`, and implement it in Core by starting an owned maintenance task with a caller-supplied turn ID. Project that port through private Shared TUI IPC v8 and convert existing compression events into the existing TUI `ContextCompression` tool card.

**Tech Stack:** Rust, Tokio, Serde, Ratatui, BitFun Agent Runtime ports, private local IPC.

## Global Constraints

- Primary command is `/compact`; the only compatibility alias is `/summarize`.
- Preserve idle-only admission and do not introduce queueing.
- Do not add a keyboard shortcut or multi-key keymap support.
- Do not change compression prompts, algorithms, automatic thresholds, or artifact schemas.
- Do not change Relay, Server, Peer, ACP, SDK Host, Web UI, extensions, or customization behavior.
- Shared IPC remains private to the first-party TUI and moves from protocol version 7 to 8.
- Caller-generated turn IDs must preserve disconnect cancellation and outcome-unknown behavior.
- Idle admission must be atomic with ordinary dialog-turn admission, and the compaction snapshot must be captured only after the maintenance turn owns the Session.
- Planning may be cancelled; once the atomic commit gate wins, context commit must finish without exposing a false idle state.
- The maintenance Turn is visible in the authoritative transcript but remains excluded from model context; live and restored tool payloads retain the same compression identity and `applied` state.
- Keep the combined PR below 8k changed lines and squash to one final commit.

---

### Task 1: Add the narrow Runtime contract and facade

**Files:**
- Modify: `src/crates/contracts/runtime-ports/src/lib.rs`
- Modify: `src/crates/execution/agent-runtime/src/runtime.rs`
- Modify: `src/crates/execution/agent-runtime/src/sdk.rs`

**Interfaces:**
- Produces: `AgentSessionCompactionRequest`, `AgentSessionCompactionResult`, `AgentSessionCompactionPort`.
- Produces: `AgentRuntimeBuilder::with_session_compaction_port` and `AgentRuntime::start_session_compaction`.

- [x] **Step 1: Write failing Runtime tests**

Add a recording provider and tests proving that the runtime forwards exact session/turn identities and returns typed `NotAvailable` when the port is absent:

```rust
#[derive(Default)]
struct RecordingCompactionPort {
    requests: Mutex<Vec<AgentSessionCompactionRequest>>,
}

#[async_trait::async_trait]
impl AgentSessionCompactionPort for RecordingCompactionPort {
    async fn start_session_compaction(
        &self,
        request: AgentSessionCompactionRequest,
    ) -> PortResult<AgentSessionCompactionResult> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(AgentSessionCompactionResult {
            session_id: request.session_id,
            turn_id: request.turn_id,
        })
    }
}
```

- [x] **Step 2: Verify RED**

Run: `cargo test -p bitfun-agent-runtime session_compaction -- --nocapture`

Expected: compilation fails because the request, result, port, builder method, and runtime method do not exist.

- [x] **Step 3: Implement the minimal contract and forwarding path**

Add serializable camelCase DTOs and the narrow async trait in `runtime-ports`; add the optional port field, builder injection, debug projection, and forwarding method in `AgentRuntime`; re-export only these stable types through `sdk`.

- [x] **Step 4: Verify GREEN**

Run:

```powershell
cargo test -p bitfun-runtime-ports
cargo test -p bitfun-agent-runtime session_compaction -- --nocapture
```

Expected: both commands pass.

### Task 2: Move manual compaction into the owned Core turn lifecycle

**Files:**
- Modify: `src/crates/assembly/core/src/agentic/coordination/coordinator.rs`
- Modify: `src/crates/assembly/core/src/agentic/execution/execution_engine.rs`
- Modify: `src/crates/assembly/core/src/service_agent_runtime.rs`

**Interfaces:**
- Consumes: `AgentSessionCompactionPort` and its request/result DTOs.
- Produces: one accepted maintenance task registered with active-session execution, settlement, cancellation, and an atomic planning/commit gate.
- Preserves: `ConversationCoordinator::compact_session_manually(String) -> BitFunResult<()>` for Desktop.

- [x] **Step 1: Write failing gate and assembly tests**

Add tests proving:

```rust
let gate = ManualCompactionCommitGate::planning();
assert!(gate.try_cancel());
assert!(!gate.try_begin_commit());

let gate = ManualCompactionCommitGate::planning();
assert!(gate.try_begin_commit());
assert!(!gate.try_cancel());
```

Add a source/assembly contract test proving all Core TUI-capable runtime builders register `with_session_compaction_port`.

- [x] **Step 2: Verify RED**

Run: `cargo test -p bitfun-core manual_compaction --features product-full -- --nocapture`

Expected: fails because the commit gate and runtime-port implementation do not exist.

- [x] **Step 3: Implement the start/task split**

Refactor the current synchronous body into:

```rust
async fn start_manual_compaction_task(
    &self,
    session_id: String,
    requested_turn_id: Option<String>,
) -> BitFunResult<ManualCompactionTask>;

pub async fn compact_session_manually(&self, session_id: String) -> BitFunResult<()>;
```

`ManualCompactionTask` contains the accepted turn ID and a private oneshot completion receiver. The runtime port calls the start function with `Some(request.turn_id)` and drops the receiver; the Desktop compatibility method awaits it.

Register before spawn:

- `register_session_execution` lease;
- `turn_settlements.register_accepted` registration;
- `CancellationToken` in `ExecutionEngine`;
- `ManualCompactionCommitGate` in a coordinator map keyed by turn ID.

The owned task persists completed, failed, or cancelled state exactly once, removes the gate and cancel token, and only then releases settlement/active-execution guards. Maintenance and ordinary dialog turns share one mutation-locked idle admission path; context is captured after that admission so a racing user turn cannot be omitted from the committed replacement.

- [x] **Step 4: Make planning cancellation-aware**

Pass the cancellation token and commit gate to `compact_session_context`. Wrap only `build_planned_compression_result` in cancellation selection. After planning, atomically call `try_begin_commit`; if cancellation already won, return `BitFunError::Cancelled`. Once commit wins, finish the existing replacement/persistence/event tail without another cancellation branch.

Teach `cancel_dialog_turn` to consult the manual gate before changing state: planning cancellation follows the existing cancellation path; commit-winning turns ignore the late cancellation request and retain processing state until completion.

- [x] **Step 5: Inject the port and verify GREEN**

Register the coordinator as `AgentSessionCompactionPort` in the shared Core runtime builder paths, then run:

```powershell
cargo test -p bitfun-core manual_compaction --features product-full -- --nocapture
cargo test -p bitfun-agent-runtime session_compaction -- --nocapture
```

Expected: tests pass with one terminal owner and no duplicate state transition.

The completion finalizer treats post-commit turn/session persistence failures as an explicit failed terminal result while retaining an idle in-memory Session. Persisted transcript projection includes the maintenance Turn and its exact tool payload without restoring that Turn into model-visible context.

### Task 3: Extend private Shared TUI IPC to protocol v8

**Files:**
- Modify: `src/crates/adapters/agent-runtime-ipc/src/protocol.rs`
- Modify: `src/crates/adapters/agent-runtime-ipc/src/operation.rs`
- Modify: `src/crates/adapters/agent-runtime-ipc/src/server.rs`
- Modify: `src/crates/adapters/agent-runtime-ipc/src/tests/protocol_contracts.rs`
- Modify: `src/crates/adapters/agent-runtime-ipc/src/tests/shared_controller.rs`
- Modify: `src/apps/cli/src/shared_runtime.rs`

**Interfaces:**
- Consumes: `AgentSessionCompactionRequest` and `AgentRuntime::start_session_compaction`.
- Produces: `RuntimeIpcOperation::CompactSession`, returning the existing `TurnAccepted` result.

- [x] **Step 1: Write failing protocol/rules/controller tests**

Test JSON round-trip and exact operation rules:

```rust
let operation = RuntimeIpcOperation::CompactSession {
    request: AgentSessionCompactionRequest {
        session_id: "session-1".into(),
        turn_id: "turn-compact-1".into(),
    },
};
let rules = operation.rules();
assert_eq!(rules.session_requirement, RuntimeIpcSessionRequirement::CurrentController);
assert!(rules.requires_idle);
assert!(rules.side_effecting);
```

Extend the shared-controller fixture to prove the supplied turn ID becomes the connection's active turn and disconnect triggers cancellation.

- [x] **Step 2: Verify RED**

Run: `cargo test -p bitfun-agent-runtime-ipc compact -- --nocapture`

Expected: compilation fails because `CompactSession` is not defined and protocol remains v7.

- [x] **Step 3: Implement protocol v8 operation**

Add the enum variant, current-controller/idle/side-effecting rules, `session_id` projection, provisional-turn extraction shared with `SubmitTurn`, handler dispatch to `AgentRuntime`, and `TurnAccepted` result. Increment `PROTOCOL_VERSION` to 8 and update explicit protocol assertions.

- [x] **Step 4: Verify GREEN**

Run: `cargo test -p bitfun-agent-runtime-ipc compact -- --nocapture`

Expected: protocol, controller, disconnect, and cancellation tests pass.

### Task 4: Add exact competitor-compatible TUI commands and feedback

**Files:**
- Modify: `src/apps/cli/src/actions.rs`
- Modify: `src/apps/cli/src/agent/runtime_client.rs`
- Modify: `src/apps/cli/src/modes/chat/commands.rs`
- Modify: `src/apps/cli/src/modes/chat/run.rs`
- Modify: `src/apps/cli/src/chat_state.rs`
- Modify: `src/apps/cli/src/modes/chat/tests.rs`

**Interfaces:**
- Consumes: runtime/IPC compaction start operation and authoritative `AgenticEvent` compression events.
- Produces: `ActionHandler::CompactSession`, aliases `/compact` and `/summarize`, and live `ContextCompression` tool-card projection.

- [x] **Step 1: Write failing action and argument tests**

Prove both aliases resolve to one idle-only action in Embedded and Shared modes, no invented alias exists, and non-empty arguments return `Usage: /compact` without starting runtime work.

- [x] **Step 2: Write failing runtime-client parity tests**

Add a focused test/source contract proving Embedded calls `runtime.start_session_compaction(request)` and Shared sends `RuntimeIpcOperation::CompactSession { request }`, both with a caller-generated stable turn ID.

- [x] **Step 3: Write failing compression projection tests**

Add a pure projection helper and tests proving:

- `ContextCompressionStarted` creates a running `ContextCompression` tool card;
- `ContextCompressionCompleted` records tokens before/after, summary source, duration, and success;
- `ContextCompressionFailed` records failure;
- unrelated sessions/turns do not mutate current TUI state.

- [x] **Step 4: Verify RED**

Run: `cargo test -p bitfun-cli compact -- --nocapture`

Expected: tests fail because the action, runtime client, and event projection do not exist.

- [x] **Step 5: Implement the minimal TUI slice**

Add the exact action aliases, call the runtime client through the existing synchronous dispatch boundary, and set an immediate accepted/error status. Convert compression events to existing `ToolEventData` values and feed `ChatState::handle_tool_event`; do not add another compaction UI model.

- [x] **Step 6: Verify GREEN**

Run:

```powershell
cargo test -p bitfun-cli compact -- --nocapture
cargo test -p bitfun-cli
```

Expected: all CLI tests pass in both runtime projections.

### Task 5: Align architecture constraints and validate the combined PR

**Files:**
- Modify: `src/crates/adapters/agent-runtime-ipc/AGENTS.md`
- Modify: `docs/architecture/agent-runtime-deployment-design.md`
- Modify: `docs/architecture/cli-product-line-design.md`
- Modify: `docs/superpowers/plans/2026-07-30-tui-manual-context-compaction.md`

- [x] **Step 1: Update the closed operation contract**

Document protocol v8, the single TUI consumer, current-controller/idle requirements, caller-provided turn identity, disconnect cancellation, and explicit non-goals. Do not describe the private wire as a public SDK or server protocol.

- [x] **Step 2: Mark completed plan steps and self-review the plan/spec**

Check for placeholders, contradictory command names, protocol version drift, and scope leakage.

- [x] **Step 3: Run required verification**

```powershell
cargo test -p bitfun-runtime-ports
cargo test -p bitfun-agent-runtime
cargo test -p bitfun-agent-runtime-ipc
cargo test -p bitfun-cli
cargo check -p bitfun-core --features product-full
node scripts/check-core-boundaries.mjs
git diff --check
```

Expected: every command passes. If a broader pre-existing failure remains, capture exact evidence and ensure focused changed-path tests pass.

- [x] **Step 4: Audit size and scope**

Run:

```powershell
git diff --stat gcwing/main...HEAD
git diff --numstat gcwing/main...HEAD
git status -sb
```

Expected: only PR1+PR2 files are present and total changed lines remain below 8k.

- [x] **Step 5: Independent adversarial review and repair**

Ask an isolated reviewer to inspect the combined diff for ownership leaks, cancellation/commit races, Shared IPC controller gaps, false terminal states, command incompatibility, transcript divergence, and unnecessary scope. Fix every actionable finding and rerun affected checks.

Review repairs covered atomic dialog/maintenance admission, post-commit terminal finalization, persisted maintenance transcript restoration, and live/restored compression identity plus `applied` parity.

- [x] **Step 6: Squash, push fork, and open Draft PR**

Create one final conventional commit, push only to `origin` (`limityan/BitFun`), and open a Draft PR against `GCWing/BitFun:main` with design, impact, risk, and validation details.

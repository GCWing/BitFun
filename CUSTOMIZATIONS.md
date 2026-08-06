# BitFun Customization Reference

> This document describes the customization snapshot in this pull request.
> Base: upstream `main` @ `e640aa40`.
> It is written for maintainers who are not familiar with the internal
> background of this fork. Every term is explained the first time it is used.

---

## Table of Contents

1. [Overview](#1-overview)
2. [Architecture](#2-architecture)
3. [Domain 1: ACP Channel — Talking to External Agents](#3-domain-1-acp-channel--talking-to-external-agents)
4. [Domain 2: Session & SessionControl](#4-domain-2-session--sessioncontrol)
5. [Domain 3: Warden Guard System](#5-domain-3-warden-guard-system)
6. [Domain 4: RBAC Subagent Roles](#6-domain-4-rbac-subagent-roles)
7. [Domain 5: Engine & Context Injection](#7-domain-5-engine--context-injection)
8. [Domain 6: Legion & Task & Plan Toolchain](#8-domain-6-legion--task--plan-toolchain)
9. [Domain 7: CodeBuddy Provider Adapter](#9-domain-7-codebuddy-provider-adapter)
10. [Domain 8: Web UI](#10-domain-8-web-ui)
11. [Summary](#11-summary)

---

## 1. Overview

**What this PR is.** A reference snapshot of customizations built on top of
BitFun. It focuses on **multi-agent collaboration** and **connecting BitFun
to external AI agents** (such as CodeBuddy, Claude Code, and OpenCode) through
the ACP protocol.

**What "ACP" means.** ACP (Agent Client Protocol) is a protocol that lets one
agent process talk to another agent process over a local pipe or socket. BitFun
uses ACP as a bridge so that an external agent (installed as a CLI on the same
machine) can be treated as a real "session" inside BitFun.

**Scope of the snapshot.**
- 370 files changed relative to upstream `main` (`e640aa40`).
- No build artifacts, no internal records, no documentation besides this file.
- Everything is the **final state** of the work; there are no intermediate
  steps or scratch files.

**High-level feature list.**

| Feature | What it does | Where (key files) |
|---|---|---|
| ACP channel | Session-level direct connection to external ACP agents | `src/crates/assembly/core/src/agentic/tools/implementations/acp_tools.rs` |
| SessionControl/SessionMessage tools | Create, talk to, compact, delete sessions (including ACP ones) | `.../tools/implementations/session_control_tool.rs`, `session_message_tool.rs` |
| Warden guard system | Governance: detects repeated failures, challenges the agent, records violations | `.../agentic/warden/` |
| RBAC subagent roles | Role templates that control which tools a subagent may use | `.../tools/restrictions.rs` |
| Engine & context injection | Per-round refresh of runtime facts; once-per-generation user context | `.../execution/execution_engine.rs` |
| Legion/Task/Plan tools | Orchestration: legion topology, task dual lifecycle, plan tool family | `.../tools/implementations/legion_control_tool.rs`, `task/`, `plan_*.rs` |
| CodeBuddy provider | OpenAI-compatible adapter for the CodeBuddy cloud API | `src/crates/adapters/ai-adapters/src/providers/codebuddy/` |
| Web UI | Flow-chat display, legion pages, model switching | `src/web-ui/src/` |

---

## 2. Architecture

This fork keeps the same overall layering as upstream BitFun. The
customizations add one new capability on top: **an ACP integration layer that
lets the existing agent runtime drive real external processes**.

```
┌────────────────────────────────────────────────────────────┐
│                       Web UI (TypeScript)                  │
│   Flow-chat rendering · legion pages · model switcher      │
└───────────────▲────────────────────────────────────────────┘
                │ events / API
┌───────────────┴────────────────────────────────────────────┐
│                       Desktop app (Rust)                   │
│   AcpClientPort (bridge to external agent processes)       │
│   AcpSessionLifecycle (create/release/cancel on events)    │
│   WardenModelJudgementPort (LLM judgement for Warden)     │
└───────────────▲────────────────────────────────────────────┘
                │ runtime-ports contracts
┌───────────────┴────────────────────────────────────────────┐
│                    Agent runtime (Rust)                    │
│   Tool layer: AcpControl/AcpMessage/AcpHistory/SessionCtl  │
│   Coordinator: session role registry, delivery decisions   │
│   Execution engine: message assembly, context injection    │
│   Warden: poke scheduler, punishment executor              │
└───────────────▲────────────────────────────────────────────┘
                │ ACP protocol
┌───────────────┴────────────────────────────────────────────┐
│              External ACP agents (CLI processes)           │
│   CodeBuddy · Claude Code · OpenCode · ...                 │
└────────────────────────────────────────────────────────────┘
```

**Key relationships.**

- The **tool layer** (`acp_tools.rs`, `session_control_tool.rs`,
  `session_message_tool.rs`) is the entry point: the model calls these tools,
  and they route the call either to a local subagent or to an external ACP
  process.
- The **desktop layer** owns the actual ACP client service. The
  `AcpClientPort` contract (in `runtime-ports`) is the interface the tool
  layer calls; the desktop implementation connects to real processes.
- The **Warden** sits on the side: it observes tool-call failures and can
  inject "pokes" (challenge messages) into the conversation.
- **RBAC** sits between the coordinator and the tool pipeline: every session
  has a role, and every role has a set of allowed tools/operations.

---

## 3. Domain 1: ACP Channel — Talking to External Agents

### 3.1 What and why

Upstream BitFun has ACP *client* infrastructure but no first-class way for a
model to **drive** an external ACP agent as a session: create it, talk to it,
read its history, cancel it. This fork adds a complete **tool family** plus a
**lifecycle bridge** so an external agent behaves like a real BitFun session.

### 3.2 Tools

| Tool | Action | Key file |
|---|---|---|
| `AcpControlTool` | `create` / `list` / `delete` / `cancel` real external ACP sessions | `acp_tools.rs:344` |
| `AcpMessageTool` | send a message to an external ACP session | `acp_tools.rs:540` |
| `AcpHistoryTool` | read the persisted transcript of an ACP session | `acp_tools.rs:673` |

All three are registered in the tool registry like any other tool, so RBAC
and the tool pipeline apply to them uniformly.

### 3.3 Direct delivery ("no middleman")

When the model sends a message to a session whose id starts with `acp__`, the
message goes **directly** to the external process through the ACP port
(`session_message_tool.rs`). There is no local model round-trip, so there is
zero local inference cost.

- Timeout for direct delivery: **1800 s** (`ACP_DIRECT_TIMEOUT_SECONDS`,
  `session_message_tool.rs:83`).
- When the external agent replies, BitFun streams the reply back as
  `TextChunk` events, and stores a **notification** (not the full text) in the
  conversation. The full reply is retrievable via `SessionHistory`.
  (`acp_direct_response_notice`, `session_message_tool.rs:1061`)

### 3.4 Lifecycle bridge

`AcpSessionLifecycleSubscriber` (`src/apps/desktop/src/runtime/acp_session_lifecycle.rs`)
listens to session events and mirrors them onto the external process:

| BitFun event | Action on external process |
|---|---|
| `SessionCreated` (agent_type = `acp__*`) | start/attach the external client |
| `SessionDeleted` | release the external process |
| `DialogTurnCancelled` | cancel the in-flight external turn |

At startup, an **orphan scan** reclaims external sessions left over from a
previous run.

### 3.5 Persistence

Direct-delivery turns are persisted as standard `DialogTurnData` files, so
`SessionHistory` can render the full external reply even after a restart.
Idempotency is handled by scanning all existing turn indexes for the same
`turn_id` before writing a new one (see `session_message_tool.rs`,
`persist_acp_direct_delivery_turn`).

---

## 4. Domain 2: Session & SessionControl

### 4.1 Compact action

`SessionControl` gained a `compact` action (`session_control_tool.rs:132`).
This lets the model trigger context compression on any subagent session that
is currently **Idle** — previously compression could only be triggered
externally. It reuses the same `AgentSessionCompactionPort` as automatic
compression, so manual and automatic compression share one code path.

### 4.2 List improvements

- `list` now supports a compact, one-line-per-session output
  (`sessionId | agentType | status | short name`) for readability
  (`session_control_tool.rs:261`).
- Sessions can carry a **short name** for display.
- `model_id` is supported when creating sessions.

### 4.3 Ghost-session fixes

Two recurring bugs in multi-agent setups are addressed:

1. **Ghost deletion** — an ACP flow session whose metadata has no
   `created_by` used to be impossible to delete (authorization failed).
   Now `ghost_acp_delete_authorized` allows deletion when the session is an
   ACP flow session with no creator, and otherwise falls back to
   owner/ancestor semantics (`session_control_tool.rs`).
2. **Ghost delivery** — a hidden subagent session (e.g. Idle for >1 h and
   unloaded from memory) used to reject message delivery with
   "Session not found". The lookup now uses `include_hidden` where
   appropriate and restores internal sessions for delivery.

### 4.4 Tombstone registry (anti-resurrection)

Deleted session ids are recorded in a tombstone registry
(`deleted-session-ids.json`, max 2000 entries). Before finalizing a turn,
the coordinator checks the registry so a deleted session can never be
"resurrected" with stale metadata.

---

## 5. Domain 3: Warden Guard System

### 5.1 What and why

A "Warden" is a governance component that watches for **repeated failures**
in the agent loop and reacts. This is a full new subsystem: a poke scheduler,
a punishment executor, and (in the desktop layer) an LLM-backed judgement
port.

### 5.2 Components

| Component | File | Purpose |
|---|---|---|
| `WardenPokeScheduler` | `warden/mod.rs:63` | Randomized scheduling of "pokes" (average every ~6.5 turns, configurable) |
| `WardenRuntime` | `warden/runtime.rs` | Orchestrates poke decisions and violation recording |
| `PunishmentExecutor` | `warden/punishment_executor.rs` | Applies consequences for confirmed violations |
| `WardenModelJudgementPort` | `src/apps/desktop/src/runtime/warden_model_judgement_port.rs` | Lets the Warden ask a model whether a failure is a real violation (avoids false positives) |
| `SKILL.md` | `warden/SKILL.md` | Defines the Warden's behaviour contract for the agent |

### 5.3 Behaviour

- Failures are classified by **scene fingerprint** so that "repeated
  failures" means repeated failures of the *same* kind — a first failure of a
  new kind does not count toward the streak.
- A `goal` linkage switch lets the Warden follow goal/reference files when
  deciding whether a poke is warranted.
- Pokes are delivered as user-role `internal_reminder` messages, so they are
  visible to the model without becoming part of the user's own message
  history.

---

## 6. Domain 4: RBAC Subagent Roles

### 6.1 What and why

RBAC (Role-Based Access Control) here means: every session has a **role**, and
each role has a set of allowed tool *names* and allowed *operation classes*
(read-only, write-file, execute-code, communicate). The goal is that a
subagent cannot accidentally call a tool that mutates files when it was only
supposed to research.

### 6.2 Roles

| Role | Allowed operation classes |
|---|---|
| Commander (main orchestrator) | ReadOnly, Communicate |
| Executor | ReadOnly, WriteFile, ExecuteCode |
| Reviewer | ReadOnly, WriteFile, ExecuteCode |
| Warden | ReadOnly, WriteFile, Communicate, ExecuteCode |
| GeneralPurpose (research/exploration subagent) | ReadOnly, WriteFile, ExecuteCode, Communicate |

Key files: `src/crates/assembly/core/src/agentic/tools/restrictions.rs`
(role templates, e.g. `general_purpose_tool_restrictions` at `:195`),
`src/crates/assembly/core/tests/rbac_master_switch.rs` (guard tests).

### 6.3 Role pinning

When a session is created with a `subagent` marker, its role is **pinned** to
Executor (or the appropriate template) instead of inheriting the creator's
role. Previously a subagent created from a Commander session could inherit
Commander — which forbids most tools — making the subagent useless. The fix
ensures subagent-marked sessions always get a usable template, and restores
the same pinning when a session is reloaded from disk.

### 6.4 Tests

`rbac_master_switch.rs` and `rbac_poke_integration.rs` assert the pinning
behaviour and that ReadOnly tools work for Executor/GeneralPurpose.

---

## 7. Domain 5: Engine & Context Injection

### 7.1 What and why

Sending a message to a model costs tokens. This fork reduces token waste and
keeps the provider-side prompt cache stable by controlling *where* and *how
often* dynamic reminder text is injected.

### 7.2 Static vs dynamic groups

- **Static group** (skills list, agent list, deferred tool list, user
  context): placed right after the system message — the "prefix cache"
  foundation; must not change position.
- **Dynamic group** (runtime facts: time, context usage): placed at the very
  end of the message list, because it changes every round and would
  invalidate the prefix cache if placed mid-stream.

Key function: `build_ai_messages_for_send` (`execution_engine.rs:1795`).

### 7.3 Runtime facts refresh policy

`refresh_runtime_facts_for_round` (`execution_engine.rs:1511`) now takes an
`inject_runtime_facts` flag:

- Injected on the **first user round** (`round_index == 0`).
- Injected after a **context recovery** round.
- **Not** injected on tool rounds (they already carry a full history).

This keeps the model informed without repeating the same facts on every turn.

### 7.4 User context once per generation

`round_dynamic_reminders` (`execution_engine.rs:1541`) tracks a
"generation" counter of the prompt cache. User context is injected **once per
cache generation** — i.e. after a new conversation or after a compression —
and then omitted on subsequent rounds of the same generation. The generation
is cleared when the session is deleted. Contract tests cover both behaviours
(`execution_engine.rs:6088`).

### 7.5 Context usage display

Flow-chat now persists the exact last-request token usage and restores it
after session hydration, so the UI shows the right usage numbers after a
reload (see `src/web-ui/src/flow_chat/utils/tokenUsageDisplay.ts` and the
associated store changes).

---

## 8. Domain 6: Legion & Task & Plan Toolchain

### 8.1 Legion (agent topology)

`LegionControlTool` (`.../tools/implementations/legion_control_tool.rs`)
deploys a "legion" topology: a set of named agent roles with a maximum of 20
nodes, persisted as agent sessions. It is registered together with the other
product tools in `tool-provider-groups`.

### 8.2 Task dual lifecycle

`Task` now supports **two lifecycles**:

- **Foreground**: run a subagent and wait for its result (as upstream).
- **Background**: spawn a subagent that keeps running after the tool returns;
  the result is delivered later through the coordination layer, and can be
  retrieved via `SessionHistory`.

`run_in_background` selects the background mode
(`.../tools/implementations/task/execution.rs`).

### 8.3 Plan tool family

`CreatePlan`, `PlanList`, `PlanRead`, `PlanUpdate` are registered as a tool
family (`plan_list_tool.rs`, `plan_read_tool.rs`, `plan_update_tool.rs`). They
read/write plan files in the workspace and are exposed to the agent through
the standard tool pipeline, so RBAC and the readonly manifest apply.

### 8.4 Goal dual trigger

The `goal` feature (automatic long-horizon tracking) now triggers when
**either** of two conditions holds: the main conversation has been silent for
10 minutes, **or** all conversations in the workspace are silent
(`.../goal_mode/mod.rs`).

---

## 9. Domain 7: CodeBuddy Provider Adapter

### 9.1 What and why

CodeBuddy (Tencent's coding agent) offers a cloud API that is
OpenAI-compatible at `https://copilot.tencent.com/v2/chat/completions`.
Because it is OpenAI-shaped, BitFun can talk to it with almost no new
transport code — the fork adds a small adapter layer.

### 9.2 What was added

| Piece | File |
|---|---|
| Provider enum value `CodeBuddy` | `src/crates/adapters/ai-adapters/src/client/format.rs` |
| Message converter (BitFun messages → CodeBuddy messages) | `.../providers/codebuddy/message_converter.rs` |
| Request builder | `.../providers/codebuddy/request.rs` |
| Streaming handler (SSE parsing) | `.../stream/stream_handler/codebuddy.rs` |
| Provider catalog entry | `src/shared/ai-provider-catalog/providers.json` |

### 9.3 Empty `finish_reason` protection

The CodeBuddy stream sends an empty string as `finish_reason` on some frames.
Old code treated any `finish_reason` as "the turn is done", which aborted tool
calls early. The fix: only treat non-empty `finish_reason` as a completion
signal (`src/crates/execution/agent-stream/src/lib.rs` and
`src/crates/adapters/ai-adapters/src/client/response_aggregator.rs`). This is
guarded by contract tests so it cannot regress.

### 9.4 UI

The model settings UI gained a searchable provider picker and the ability to
select a global default model (`src/web-ui/src/infrastructure/config/...`).

---

## 10. Domain 8: Web UI

### 10.1 Flow-chat

- Turn completion notices and footer layout (`turnCompletionNotice.ts`,
  `FlowChatStore.ts`).
- Context usage display persisted across hydration (see §7.5).
- Subagent projection view (`SubagentProjectionView.tsx`).
- `handleTextChunk` creates an ACP session placeholder when a text chunk
  arrives before the session registration, so early stream output is not
  silently dropped (`flow-chat-manager/EventHandlerModule.ts`).

### 10.2 Legion pages

- `CreateLegionPage`, `LegionCard`, `BeeColonyMonitor`, `AgentsScene`
  provide a visual view of agent topology
  (`src/web-ui/src/app/scenes/agents/`, `src/web-ui/src/app/layout/`).
- `LegionPresetAPI` talks to the backend preset registry.

### 10.3 Model switching

- Searchable provider/model list and global default model selection
  (`AIModelConfig.tsx`, `builtinProviderCatalog.ts`).

---

## 11. Summary

This PR is a **reference snapshot** of customization work on BitFun for
multi-agent collaboration and external-agent integration:

1. **ACP channel** — drive real external agents as first-class sessions
   (create/talk/history/cancel), with direct delivery, lifecycle mirroring,
   persistence, and orphan reclamation.
2. **Session tooling** — compact action, better listing, ghost-session
   fixes, tombstone anti-resurrection.
3. **Warden** — a full governance subsystem with scene-aware failure
   detection and LLM-assisted judgement.
4. **RBAC** — role templates and role pinning so subagents get usable,
   safe tool sets.
5. **Engine** — stable prompt-cache prefix, per-round runtime facts,
   once-per-generation user context.
6. **Orchestration** — legion topology, task dual lifecycle, plan tool
   family, goal dual trigger.
7. **CodeBuddy** — an OpenAI-compatible provider adapter with an empty
   `finish_reason` fix.
8. **Web UI** — flow-chat display, legion pages, model switching.

Everything in this snapshot is the final state; no intermediate commits,
scratch files, or internal records are included.

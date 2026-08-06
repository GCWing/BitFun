# Customization of BitFun for Multi-Agent Collaboration and External Agent Integration

**A Technical Report in the Form of a Research Paper**

> Base: upstream `main` @ `e640aa40`.
> Scope: 370 files changed (code, configuration, and runtime resources).
> Audience: maintainers and reviewers. Formal academic tone; every term is
> defined at first use; every design decision carries its motivation.

---

## Abstract

This report presents a set of customizations to BitFun, an open-source
agent-integration platform, that extends it for **multi-agent collaboration**
and **external agent interconnection**. The work makes four contributions.
First, it introduces a complete **ACP (Agent Client Protocol) channel** that
treats external agent processes (e.g. CodeBuddy, Claude Code, OpenCode) as
first-class sessions inside BitFun, with session-level direct delivery, a
lifecycle bridge, and persisted transcripts. Second, it adds a **governance
layer** — a Warden guard system and a role-based access control (RBAC) model
for subagents — that constrains what an agent may do and detects repeated
failure patterns. Third, it hardens the **engine-level context management**:
a stable prompt-cache prefix, per-round runtime facts, and once-per-generation
user context injection, which measurably reduce token waste. Fourth, it
contributes a **workflow architecture** for organizing multi-agent work
(a six-phase pipeline, a three-branch separation of powers, and a recursive
dispatch pattern) that is described in the Methodology section. All claims in
this report are grounded in the final-state source tree; line numbers refer
to the files in this pull request.

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Related Work](#2-related-work)
3. [Design: A Three-Branch Separation-of-Powers Coordination Model](#3-design-a-three-branch-separation-of-powers-coordination-model)
   - 3.1 Problem statement · 3.2 Design goals · 3.3 Six-phase pipeline · 3.4 Separation of powers · 3.5 Recursive dispatch pattern · 3.6 Serial / parallel discipline · 3.7 Quality gate system · 3.8 Atomic step specification · 3.9 Both ends fixed, middle free · 3.10 Decision derivation
4. [Implementation 1: ACP Channel](#4-implementation-1-acp-channel)
5. [Implementation 2: Session and SessionControl](#5-implementation-2-session-and-sessioncontrol)
6. [Implementation 3: Warden Guard System](#6-implementation-3-warden-guard-system)
7. [Implementation 4: RBAC for Subagents](#7-implementation-4-rbac-for-subagents)
8. [Implementation 5: Engine and Context Injection](#8-implementation-5-engine-and-context-injection)
9. [Implementation 6: Orchestration Toolchain (Legion, Task, Plan, Goal)](#9-implementation-6-orchestration-toolchain)
10. [Implementation 7: CodeBuddy Provider Adapter](#10-implementation-7-codebuddy-provider-adapter)
11. [Implementation 8: Web UI](#11-implementation-8-web-ui)
12. [Evaluation](#12-evaluation)
13. [Conclusion and Future Work](#13-conclusion-and-future-work)
14. [References](#14-references)

---

## 1. Introduction

### 1.1 Background

BitFun is an agent-integration platform: it hosts agent sessions, exposes
tools to models, and coordinates the execution of agent loops. In its
upstream form, the platform provides the machinery for a single agent loop,
and an ACP client layer for probing external agent CLIs.

### 1.2 Problem

Deploying BitFun in a **multi-agent setting** — where one coordinating agent
delegates work to several subagents, and where some of those subagents are
*external* processes reached over ACP — exposes four gaps:

1. **No first-class external sessions.** The upstream ACP layer can *probe*
   external agents, but a model cannot create an external agent session,
   message it directly, read its history, or cancel it as a normal BitFun
   session.
2. **No governance.** A delegated subagent can accidentally call a tool that
   mutates state when it was only asked to research; repeated failures can
   cascade without detection.
3. **Token waste and cache instability.** Dynamic reminder text (clock,
   context usage) injected at a variable position invalidates the
   provider-side prompt cache, and redundant reminders are repeated on every
   turn.
4. **No methodological framework for multi-agent work itself.** When many
   agents collaborate on one codebase, preparation, execution, and
   verification blur together, and defects leak through.

### 1.3 Contributions

This work addresses the four gaps with four contributions:

- **C1 — ACP channel** (§4): external ACP agents become first-class sessions.
- **C2 — Governance** (§6, §7): Warden guard system + RBAC subagent roles.
- **C3 — Context engine** (§8): stable prefix, per-round facts,
  once-per-generation user context.
- **C4 — Workflow architecture** (§3): a three-branch separation-of-powers
  coordination model, described as the design methodology.

---

## 2. Related Work

### 2.1 Agent protocols

**ACP (Agent Client Protocol)** is a transport used to bridge a host agent
with an external agent CLI. Upstream BitFun implements ACP *client*
infrastructure: probing, launching, and low-level message transport. This
work builds on that foundation by exposing it through the **tool layer** so a
model can drive external sessions semantically.

### 2.2 Multi-agent orchestration

Existing orchestration systems typically centralize control (a single
dispatcher) or decentralize it (free-form peer messaging). Both extremes have
known failure modes: centralized systems become single points of failure and
bottlenecks; fully decentralized systems lose auditability. The coordination
model in §3 takes a middle path — **separation of powers with a bounded
rejection loop** — which is inspired by classical engineering review
practices (independent verification and validation, IV&V) and by the
peer-review process in scientific publication.

### 2.3 Role-based access control

RBAC (Sandhu et al., 1996) is a standard authorization model: subjects are
assigned roles, roles have permissions. The customization in §7 applies RBAC
at the *agent-session* granularity: each session is assigned a role whose
permission set is enforced by the tool pipeline.

### 2.4 Prompt caching

LLM providers (e.g. DeepSeek, Anthropic) offer prefix-based prompt caching:
a request whose prefix matches a cached prefix is served at a fraction of the
cost. The dominant strategy (Anthropic, 2024) is to keep dynamic content at
the *end* of the message list. §8 implements this strategy structurally.

---

## 3. Design: A Three-Branch Separation-of-Powers Coordination Model

> This section is the design methodology. It describes how multi-agent work
> is organized on top of the platform, so that the platform's own features
> (sessions, tasks, quality gates) are used within a coherent workflow.

### 3.1 Problem statement

Multi-agent software projects face a recurring structural problem: when many
agents work on one codebase, the three activities of **decision making**,
**execution**, and **verification** tend to be performed by the same agent,
at the same time, on the same artifact. This conflation yields three known
failure modes: (i) *unchecked decisions* — a single agent both decides and
executes, with no independent review; (ii) *self-review bias* — the agent
that wrote code also validates it; and (iii) *rework cascades* — defects are
discovered late, after dependent work has been built on top of them.

### 3.2 Design goals

The model is designed to satisfy five goals:

| Goal | Description |
|---|---|
| G1 | **Separation of powers** — decision, execution, and review are held by distinct roles. |
| G2 | **Auditability** — every phase produces an artifact that is the input of the next; every claim carries evidence. |
| G3 | **Determinism** — preparation is unbounded; execution is one-shot. |
| G4 | **Scalability** — the same pattern applies recursively at every decomposition level. |
| G5 | **Efficiency** — parallelizable work runs in parallel; dependent work runs serially through gates. |

### 3.3 The six-phase pipeline

Every task passes through six phases in order; the output of each phase is
the input of the next, and no phase is skipped.

| Phase | Name | Input | Output | Quality concern |
|---|---|---|---|---|
| 0 | Requirements | raw request | clarified requirement document | ambiguity |
| 1 | Reconnaissance | requirement document | reconnaissance report | hallucination |
| 2 | Planning | reconnaissance report | plan / type contract / dispatch prompts | incompleteness |
| 3 | Execution | plan documents | code | deviation |
| 4 | Quality gate | code | pass / reject | defect leakage |
| 5 | Delivery | accepted code | delivered result | completeness |

The pipeline is *not* a checklist game: each phase must be genuinely
completed (reconnaissance really performed, plans really thought through,
verification really executed). Skipping a phase invalidates the pipeline and
the task returns to Phase 0.

### 3.4 Three-branch separation of powers

Three peer roles hold three powers; none reports to another; each is barred
from encroaching on the others:

| Power | Role | Responsibility | Barred from |
|---|---|---|---|
| Decision | Coordinator | requirements, planning, dispatch, direction rulings | performing execution |
| Execution | Executor | receiving atomic steps, executing them exactly | making decisions |
| Review | Reviewer | quality gates: review, test, acceptance | self-review |

The checks are mutual: the coordinator may not edit code; the executor's only
exit at a decision point is to report back; the reviewer is independent of
the executor. At serial nodes all three gates must pass before the next wave.

### 3.5 Recursive dispatch pattern

The organization uses **one pattern, recursively**: *Dispatch → Execute →
Accept*. A coordinator dispatches to an executor; the executor returns; a
reviewer accepts or rejects. Rejection returns the artifact to the executor
for a bounded number of repair rounds (at most three), after which the case
escalates to the coordinator.

The pattern is applied at every level: a top-level coordinator delegates to
team leads, each team lead applies the same pattern within the team, and
agents apply it internally (self-check as the reviewer). This yields a
*fractal* organization rather than a strict hierarchy. Context isolation is a
first-class property: each role loads only the context relevant to its own
duty, so that the contexts of the three branches do not pollute one another.

### 3.6 Serial / parallel discipline

Parallelism is decided by dependency, not by preference:

- **Independent** work runs **in parallel** (reconnaissance, independent
  modules, draft planning).
- **Dependent** work runs **serially** through the gates (recon → plan →
  execute → verify → deliver).

Parallel branches converge at a serial node: one designated executor runs the
full gate suite and makes the commit. During parallel execution each track
runs only its own scoped tests; the full regression suite runs only at the
convergence node.

### 3.7 Quality gate system

At serial nodes, the artifact must pass **three gates**, run in parallel and
all mandatory; a single gate failing returns the artifact for repair, after
which all three gates are re-run:

| Gate | Role | Criterion |
|---|---|---|
| Review | Reviewer | Logic, architecture, and compliance; full-chain; evidence with file:line |
| Test | Executor | Compile clean, tests green, linter zero warnings |
| Acceptance | Reviewer / Acceptor | Feature-by-feature comparison against the original requirements; check for empty stubs |

Two additional laws govern the gates:

- **Gate blind-spot law.** A green gate does not imply the delivery is
  wireable — the gates must cover the real integration path (smoke tests),
  not only the isolated modules.
- **Review is reviewed.** Language and framework semantics asserted by a
  review must themselves be verified empirically.

Repeated gate failure indicates that the root cause lies in preparation: the
task returns to Phase 0 rather than patching in place.

### 3.8 Atomic step specification

Every dispatched step is specified by **five elements**:

1. **Input location** — where the inputs are.
2. **Action instruction** — what to do, in imperative terms.
3. **Expected output** — what the step should produce.
4. **Acceptance assertion** — how to verify correctness.
5. **Failure fallback** — what to do on deviation.

The completeness criterion is *determinism across executors*: any person —
even one without background — following the step must obtain the same result.
Executors may be lazy, misjudge, or misunderstand; the instruction is
designed so that being wrong is difficult.

### 3.9 Determinism: both ends fixed, middle free

The requirement (start) and the delivery (end) are fixed before execution
begins; the path between them is free. Deviations in the middle are permitted,
but the deviation-handling path (which phase to roll back to, which assertion
to fix) is predefined during preparation, and the acceptance assertions at
the end are written before implementation. Deviation is a path fluctuation;
it never changes the delivery. Preparation rounds are unbounded; execution is
one-shot.

### 3.10 Decision derivation

During execution, all decisions are derived from three sources, in order:
**(1) requirements** (original requirement id / authoritative source),
**(2) purpose** (the final delivery definition), and **(3) iron rules**
(the framework's own quality criteria). Users participate only at the
requirements stage and the delivery-result stage. Autonomous decisions must
not expand into resource commitments the user never requested.

---

## 4. Implementation 1: ACP Channel

### 4.1 Design

The ACP channel makes an external agent process behave like a real BitFun
session. The tool layer exposes three tools; the desktop layer owns the ACP
client service and the lifecycle bridge.

### 4.2 Tools

| Tool | Action | Definition |
|---|---|---|
| `AcpControlTool` | create / list / delete / cancel external ACP sessions | `acp_tools.rs:344` |
| `AcpMessageTool` | send a message to an external ACP session | `acp_tools.rs:540` |
| `AcpHistoryTool` | read the persisted transcript of an ACP session | `acp_tools.rs:673` |

The tools are registered through the standard tool registry, so RBAC and the
tool pipeline apply to them uniformly.

### 4.3 Direct delivery

When the model sends a message to a session whose id starts with `acp__`, the
message is forwarded **directly** to the external process through the ACP
port, with no local model round-trip and therefore no local inference cost.
The timeout is 1800 s (`ACP_DIRECT_TIMEOUT_SECONDS`, `session_message_tool.rs:83`).
The external reply is streamed back as `TextChunk` events; the conversation
stores a *notification* (`acp_direct_response_notice`, `session_message_tool.rs:1061`)
instead of the full text, and the full reply is retrievable via
`SessionHistory`.

*Motivation.* Injecting the full external reply into the conversation would
consume context budget and duplicate content already stored on disk. The
notification-plus-retrievable-store design keeps the context lean while
preserving full fidelity.

### 4.4 Lifecycle bridge

`AcpSessionLifecycleSubscriber`
(`src/apps/desktop/src/runtime/acp_session_lifecycle.rs`) mirrors BitFun
session events onto the external process:

| BitFun event | Action |
|---|---|
| `SessionCreated` (`acp__*`) | start / attach the external client |
| `SessionDeleted` | release the external process |
| `DialogTurnCancelled` | cancel the in-flight external turn |

An **orphan scan** at startup reclaims external sessions left over from a
previous run.

### 4.5 Persistence and idempotency

Direct-delivery turns are persisted as standard `DialogTurnData` files so that
`SessionHistory` can render the full external reply after a restart.
Idempotency is enforced by scanning all existing turn indexes for the same
`turn_id` before writing, and appending at the first free index
(`persist_acp_direct_delivery_turn`, `session_message_tool.rs:1177`). This
prevents duplicate writes when the metadata turn counter and the on-disk
turns have diverged.

---

## 5. Implementation 2: Session and SessionControl

### 5.1 Compact action

`SessionControl` gained a `compact` action
(`session_control_tool.rs:132`) that lets the model trigger context
compression on any subagent session that is **Idle**. It reuses the same
`AgentSessionCompactionPort` as automatic compression, so manual and automatic
compression share one code path.

*Motivation.* Previously compression could only be triggered externally
(desktop / app-server / CLI); a model delegating work could not ask a busy
subagent to compact its own context.

### 5.2 Listing and naming

`list` supports a compact one-line-per-session output
(`sessionId | agentType | status | short name`); sessions can carry a
**short name**; `model_id` is supported at session creation.

### 5.3 Ghost-session fixes

Two recurring defects in multi-agent deployments are addressed:

1. **Ghost deletion.** An ACP flow session whose metadata has no `created_by`
   could not be deleted (authorization failed). `ghost_acp_delete_authorized`
   now permits deletion for ACP flow sessions without a creator, falling back
   to owner/ancestor semantics otherwise.
2. **Ghost delivery.** A hidden subagent session (e.g. Idle > 1 h, unloaded
   from memory) rejected message delivery with "Session not found". Lookup
   now uses hidden-inclusive semantics and restores internal sessions for
   delivery.

### 5.4 Tombstone registry

Deleted session ids are recorded in a tombstone registry
(`deleted-session-ids.json`, max 2000 entries). Before finalizing a turn, the
coordinator consults the registry so a deleted session cannot be
"resurrected" with stale metadata.

---

## 6. Implementation 3: Warden Guard System

### 6.1 Design

The Warden is a governance subsystem that observes the agent loop and reacts
to **repeated failures**. It is a new subsystem with three components: a poke
scheduler, a punishment executor, and an LLM-backed judgement port.

### 6.2 Components

| Component | File | Purpose |
|---|---|---|
| `WardenPokeScheduler` | `warden/mod.rs:63` | Randomized scheduling of "pokes" (average interval configurable) |
| `WardenRuntime` | `warden/runtime.rs` | Orchestrates poke decisions and violation recording |
| `PunishmentExecutor` | `warden/punishment_executor.rs` | Applies consequences for confirmed violations |
| `WardenModelJudgementPort` | `src/apps/desktop/src/runtime/warden_model_judgement_port.rs` | Asks a model whether a failure is a real violation (reduces false positives) |
| `SKILL.md` | `warden/SKILL.md` | Behaviour contract for the agent |

### 6.3 Behaviour

- **Scene fingerprinting.** Failures are classified by *scene* so that a
  "streak" means repeated failures of the same kind; the first failure of a
  new kind does not count toward the streak.
- **Goal linkage.** A switch lets the Warden follow goal/reference files when
  deciding whether a poke is warranted.
- **Delivery.** Pokes are delivered as user-role `internal_reminder` messages:
  visible to the model, yet not part of the user's own message history.

*Motivation.* A naive "N failures in a row → punish" rule misfires when
failures are heterogeneous. Scene fingerprinting makes the guard precise,
and the LLM judgement port adds a second opinion to avoid punishing a
legitimate attempt.

---

## 7. Implementation 4: RBAC for Subagents

### 7.1 Design

RBAC here means: every session has a **role**; each role has a set of allowed
tool *names* and allowed *operation classes* (read-only, write-file,
execute-code, communicate). The enforcement point is the tool pipeline.

### 7.2 Roles

| Role | Allowed operation classes |
|---|---|
| Commander | ReadOnly, Communicate |
| Executor | ReadOnly, WriteFile, ExecuteCode |
| Reviewer | ReadOnly, WriteFile, ExecuteCode |
| Warden | ReadOnly, WriteFile, Communicate, ExecuteCode |
| GeneralPurpose | ReadOnly, WriteFile, ExecuteCode, Communicate |

Key files: `src/crates/assembly/core/src/agentic/tools/restrictions.rs`
(e.g. `general_purpose_tool_restrictions` at `:195`),
`src/crates/assembly/core/tests/rbac_master_switch.rs`.

### 7.3 Role pinning

A session created with a `subagent` marker has its role **pinned** to an
appropriate template instead of inheriting the creator's role. Previously a
subagent created from a Commander session could inherit Commander — which
forbids most tools — making the subagent unusable. Pinning is also restored
when a session is reloaded from disk.

*Motivation.* Inheritance is convenient but unsafe: a research subagent must
never inherit the mutating privileges of its creator. Pinning trades
flexibility for safety at the cost of an explicit template.

---

## 8. Implementation 5: Engine and Context Injection

### 8.1 Problem

Sending a message to a model costs tokens, and the provider-side prompt cache
is prefix-based: any per-round change in the *middle* of the message list
invalidates the entire prefix. Dynamic reminder text (clock, context usage)
is inherently per-round.

### 8.2 Static vs dynamic groups

- **Static group** (skills list, agent list, deferred tool list, user
  context): placed immediately after the system message — the prefix-cache
  foundation; position invariant.
- **Dynamic group** (runtime facts): placed at the very end of the message
  list, because it changes every round.

Key function: `build_ai_messages_for_send` (`execution_engine.rs:1795`).

### 8.3 Runtime facts refresh policy

`refresh_runtime_facts_for_round` (`execution_engine.rs:1511`) takes an
`inject_runtime_facts` flag: injected on the first user round
(`round_index == 0`) and after a context-recovery round; **not** injected on
tool rounds. This keeps the model informed without repeating identical facts.

### 8.4 User context once per generation

`round_dynamic_reminders` (`execution_engine.rs:1541`) tracks a "generation"
counter of the prompt cache. User context is injected **once per cache
generation** — after a new conversation or a compression — and omitted on
subsequent rounds of the same generation. The generation is cleared on
session deletion.

*Motivation.* The user-context block is large and mostly static; repeating it
every round costs tokens and destabilizes the prefix. Injecting it once per
generation preserves its information while bounding its cost.

### 8.5 Context usage display

Flow-chat persists the exact last-request token usage and restores it after
session hydration, so the UI shows correct usage numbers after a reload
(`src/web-ui/src/flow_chat/utils/tokenUsageDisplay.ts`).

---

## 9. Implementation 6: Orchestration Toolchain

### 9.1 Legion (agent topology)

`LegionControlTool` deploys a "legion" topology: a set of named agent roles
with a maximum of 20 nodes, persisted as agent sessions.

### 9.2 Task dual lifecycle

`Task` supports two lifecycles:

- **Foreground**: run a subagent and wait for its result.
- **Background**: spawn a subagent that keeps running after the tool returns;
  the result is delivered later through the coordination layer and
  retrievable via `SessionHistory` (`run_in_background` selects the mode;
  `.../tools/implementations/task/execution.rs`).

### 9.3 Plan tool family

`CreatePlan`, `PlanList`, `PlanRead`, `PlanUpdate` are registered as a tool
family (`plan_list_tool.rs`, `plan_read_tool.rs`, `plan_update_tool.rs`) and
go through the standard tool pipeline, so RBAC and the readonly manifest
apply.

### 9.4 Goal dual trigger

The `goal` feature triggers when **either** of two conditions holds: the main
conversation has been silent for 10 minutes, **or** all conversations in the
workspace are silent (`.../goal_mode/mod.rs`).

---

## 10. Implementation 7: CodeBuddy Provider Adapter

### 10.1 Design

CodeBuddy (Tencent's coding agent) exposes an OpenAI-compatible cloud API at
`https://copilot.tencent.com/v2/chat/completions`. Because the endpoint is
OpenAI-shaped, BitFun can reuse its existing OpenAI transport; the adapter
adds a small conversion layer.

### 10.2 Components

| Piece | File |
|---|---|
| Provider enum value `CodeBuddy` | `src/crates/adapters/ai-adapters/src/client/format.rs` |
| Message converter | `.../providers/codebuddy/message_converter.rs` |
| Request builder | `.../providers/codebuddy/request.rs` |
| Streaming handler (SSE) | `.../stream/stream_handler/codebuddy.rs` |
| Provider catalog entry | `src/shared/ai-provider-catalog/providers.json` |

### 10.3 Empty `finish_reason` protection

The CodeBuddy stream emits an **empty string** as `finish_reason` on some
frames. Old code treated any `finish_reason` as "turn done", aborting tool
calls early. The fix treats only non-empty `finish_reason` as a completion
signal (`src/crates/execution/agent-stream/src/lib.rs`,
`src/crates/adapters/ai-adapters/src/client/response_aggregator.rs`), guarded
by contract tests.

### 10.4 UI

The model settings UI gained a searchable provider picker and a global
default-model selection (`src/web-ui/src/infrastructure/config/...`).

---

## 11. Implementation 8: Web UI

### 11.1 Flow-chat

- Turn completion notices and footer layout (`turnCompletionNotice.ts`,
  `FlowChatStore.ts`).
- Context usage display persisted across hydration (§8.5).
- Subagent projection view (`SubagentProjectionView.tsx`).
- `handleTextChunk` creates an ACP session placeholder when a text chunk
  arrives before session registration, so early stream output is not
  silently dropped (`flow-chat-manager/EventHandlerModule.ts`).

### 11.2 Legion pages

`CreateLegionPage`, `LegionCard`, `BeeColonyMonitor`, and `AgentsScene`
provide a visual view of agent topology
(`src/web-ui/src/app/scenes/agents/`, `src/web-ui/src/app/layout/`);
`LegionPresetAPI` talks to the backend preset registry.

### 11.3 Model switching

Searchable provider/model list and global default-model selection
(`AIModelConfig.tsx`, `builtinProviderCatalog.ts`).

---

## 12. Evaluation

### 12.1 Test evidence

The changes are accompanied by contract and integration tests that pin the
behaviour described above:

| Behaviour | Test | Location |
|---|---|---|
| RBAC pinning + ReadOnly for GeneralPurpose | `general_purpose_subagent_role_is_executor_and_readonly_allowed` | `rbac_master_switch.rs:239` |
| Runtime facts cleared on tool rounds | `tool_round_clears_runtime_facts_after_user_round_injection` | `execution_engine.rs:6045` |
| User context injected once per cache generation | `round_dynamic_reminders_injects_user_context_once_per_cache_generation` | `execution_engine.rs:6088` |
| ReadOnly classification for workspace scans | `classify_tool_call_workspace_scan_is_readonly` | `framework.rs:3413` |

### 12.2 ACP channel guarantees

The ACP channel is covered by unit tests for client-id parsing and for the
notification format (the notification must exclude the full reply, include
the session id, and point to `SessionHistory`).

### 12.3 Limitations

The following are known limitations of the snapshot:

- The Warden judgement port requires a model endpoint at runtime; its
  behaviour without a configured model is conservative (no pokes).
- The CodeBuddy adapter depends on the cloud endpoint's API shape, which may
  evolve independently.
- The workflow model in §3 is a *methodology* — it is realized through the
  platform's session/task/tool machinery, but it is not itself a separate
  runtime component.

---

## 13. Conclusion and Future Work

This report presented a set of customizations to BitFun for multi-agent
collaboration and external agent interconnection. The ACP channel (C1) makes
external agents first-class sessions with direct delivery, lifecycle
mirroring, and persisted, idempotent transcripts. The governance layer (C2)
adds a Warden guard system and RBAC subagent roles. The context engine (C3)
stabilizes the prompt-cache prefix and bounds dynamic injection. The workflow
architecture (C4) provides a separation-of-powers coordination model.

Future work includes: making the Warden judgement port optional-configurable
per workspace; extending the CodeBuddy adapter to additional endpoints as
they become OpenAI-compatible; and formalizing the coordination model (§3)
as an explicit runtime policy (e.g. a declarative workflow configuration
consumed by the scheduler).

---

## 14. References

1. **ACP — Agent Client Protocol.** https://github.com/agent-client-protocol/agent-client-protocol
2. R. Sandhu, E. Coyne, H. Feinstein, C. Youman. *Role-Based Access Control
   Models.* IEEE Computer, 29(2), 1996.
3. Anthropic. *Prompt Caching.* Anthropic Documentation, 2024.
4. BitFun. https://github.com/GCWing/BitFun

---

*All line numbers refer to the files in this pull request (base
`e640aa40`). This document contains only generic engineering descriptions
and no proprietary information.*

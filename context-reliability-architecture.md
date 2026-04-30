# BitFun Context Reliability Architecture

> Updated proposal: 2026-04-27
>
> This document replaces the earlier "Context Distraction 与 Context Poisoning 优化方案" draft. The new framing is not "make compression smarter" but "make long-running local agent work reliable, auditable, and recoverable across compression, pruning, weak models, and subagent handoffs."

## 1. Executive Summary

BitFun already has a useful foundation:

- L0 Microcompact clears old compactable tool results.
- L1 ContextCompressor performs model summary plus structured fallback compression.
- L2 emergency truncation prevents provider context overflow.
- `MessageSemanticKind` distinguishes actual user input, internal reminders, compression boundaries, and compression summaries.
- `TaskTool` supports `timeout_seconds`, and Deep Review has reviewer/judge policy, file splitting, and read-only reviewer enforcement.
- `deep-review-design.md` is treated as an implemented baseline: Deep Review has Strategy Engine behavior, Architecture and Frontend reviewers, predictive timeout, dynamic concurrency policy, partial result capture, retry budget, strategy directive / model plumbing, and Judge overlap handling.

The remaining risk is not just token pressure. The harder product problem is context reliability:

1. The agent may forget the original task, scope, user constraints, touched files, or failed checks after compression.
2. User text or tool output can look like higher-priority instructions.
3. Tool results can be pruned correctly while the model still loses the operational fact that a command failed or a file changed.
4. Subagents can isolate context but still return outputs that are hard for the parent agent to merge safely.
5. Weak models amplify all of the above: they summarize worse, follow pointers less reliably, and recover less gracefully from partial state.

The revised proposal is to build a **Context Reliability Architecture** around four product promises:

- **Trusted:** every context item has a clear source and priority.
- **Auditable:** important tool facts survive pruning and compression.
- **Recoverable:** risky edits and long workflows have resumable state and rollback boundaries.
- **Adaptive:** stronger and weaker models receive different levels of structure, automation, and user confirmation.

### 1.1 Relationship to the Runtime Budget Plan

This document should remain independent from `agent-runtime-budget-governance-design.md`.

The boundary is:

- **This document is the context reliability foundation.** It owns context trust, Evidence Ledger, Compaction Contract, snapshot/recovery, Work Packet shape/projection boundaries, adaptive context profiles, weak-model policy, and Context Health. It does not own Deep Review-specific reviewer roles, prompt directives, or retry behavior.
- **The runtime budget plan is the control plane for failure prevention and recovery.** It owns `PartialRecoveryKind`, `ProviderFinishReason`, `ToolArgumentStatus`, `ContextMutationKind` events, `SubagentScheduler`, gateway request limiting, output spill, and the Large File Write Protocol.
- **Integration is event-based, not by merging responsibilities.** Runtime events from truncation recovery, scheduler state transitions, artifact creation, spill, and large file writes should become ledger facts when the ledger exists. The context architecture should then use those facts for compaction, health scoring, and user-facing recovery summaries.

This split avoids a misleading mega-design where compression, scheduler capacity, provider retry, artifact storage, and trust boundaries appear to be one module. They are related, but they should remain separately testable and separately releasable.

### 1.2 Relationship to Implemented Deep Review

`deep-review-design.md` is the Deep Review product and strategy baseline. This document should not re-plan reviewer roles, prompt ownership, predictive timeouts, partial result capture, retry budget, or dynamic reviewer concurrency as if they were missing.

Instead, this architecture consumes the facts produced by the implemented Deep Review flow:

- launch manifest / strategy directive / reviewer role / scope
- `model_id` and reviewer-specific prompt directive
- predictive active timeout and retry budget
- partial reviewer evidence and timeout status
- reviewer queue / retry / completion state once runtime scheduler events exist
- reviewer report artifacts and final judge output

The context architecture turns those facts into trust metadata, Evidence Ledger entries, Compaction Contract fields, and Context Health signals. It does not decide actual reviewer execution order, gateway capacity, or retry ownership.

### 1.3 Convergence and No-Duplicate Rule

Similar mechanisms must converge into the existing owner. This architecture should prefer projections over new modules: if Deep Review, runtime budget governance, and context management describe the same status, retry, artifact, budget, or task contract, only one layer owns behavior and the other layers consume typed facts.

| Similar area | Source of truth | Context architecture action | Must not do |
| --- | --- | --- | --- |
| Deep Review launch manifest / Work Packet | implemented Deep Review canonical manifest | project manifest facts into Work Packet and Compaction Contract fields | duplicate reviewer roles, model ids, prompt directives, or retry budgets |
| Review Evidence Pack / large source snapshot | Deep Review source resolver + runtime artifact storage | record pack refs, hashes, source kind, source fingerprint, slice ids, and stale status in ledger/contract | let each reviewer reconstruct the same complete source evidence independently |
| scheduler states and subagent capacity | runtime `SubagentScheduler` + gateway limiter | record state projections in Evidence Ledger and Context Health | implement queueing, permits, retry backoff, or effective concurrency in context code |
| raw `partial_timeout` and partial recovery state | runtime normalized scheduler state | keep raw status as diagnostics and expose `completed_with_partial` / `timed_out` / `retry_waiting` as model-visible facts | let UI, Judge, or ledger treat raw Deep Review strings as the primary state |
| artifact refs, spill files, large-write manifests | runtime artifact / session storage | preserve refs, hashes, sensitivity flags, status, and next action in ledger/contract | create a parallel context artifact store or inject full large outputs back into context |
| context compaction and budget mutation | `ContextMutationKind` event + Compaction Contract | summarize mutation facts and preserve user intent / trusted evidence | infer mutations from prompt text or encode context mutation as partial output recovery |
| budget thresholds and model profile limits | runtime budget module and model/provider profile | consume budget outcomes as health signals and compaction inputs | hard-code separate budget thresholds inside context profiles |
| evidence facts | trusted tool/runtime events | project facts into ledger summaries and compaction contracts | promote model-generated summaries to authoritative facts |

## 2. Competitive Reference: What Matters

The relevant comparables are general coding-agent runtimes:

### 2.1 Codex

Codex emphasizes independent task execution, AGENTS.md as scoped project instructions, and verifiable evidence through terminal logs and test outputs. OpenAI's own guidance also warns against turning AGENTS.md into a giant manual: a short AGENTS.md should act as a map to deeper repository docs, not as the full knowledge base.

The Codex agent loop also treats context window management as a runtime responsibility and describes automatic compaction once a token threshold is exceeded.

Useful lesson for BitFun:

- Keep always-injected instructions short.
- Treat repository docs as the system of record.
- Preserve evidence, not just prose summaries.
- Run longer tasks as independently reviewable units.
- Let automatic compaction start as an observable runtime behavior, then gate default enforcement on measured reliability.

### 2.2 Claude Code

Claude Code uses CLAUDE.md, auto memory, subagents, and compaction. Its public subagent guidance emphasizes independent context windows, concise final summaries back to the parent, tool restrictions, and foreground/background execution. The Agent SDK guidance similarly frames subagents as separate instances for context isolation, parallelization, specialized instructions, and tool restrictions.

Useful lesson for BitFun:

- Compaction needs a contract: it must preserve operational facts, not only conversation meaning.
- Subagents are most useful when they isolate focused work and return concise, structured results.
- Memory is context, not enforcement; hard safety and priority rules should be represented programmatically where possible.
- Subagent contracts should describe scope and permissions, while runtime scheduling should own concurrency, queueing, retry, and timeout semantics.

### 2.3 OpenCode

OpenCode exposes configurable compaction (`auto`, `prune`, `reserved`), snapshots, agent permissions, hidden subagents, and task permissions. Its agent model distinguishes primary agents from subagents and can hide internal subagents while still allowing programmatic invocation through the Task tool when permissions allow.

Useful lesson for BitFun:

- Context pruning should be configurable and observable.
- Snapshot/recovery is part of user trust, especially when agents edit files.
- Per-agent permissions and subagent invocation permissions reduce orchestration risk.
- Hidden/internal subagents should be programmatically invokable without cluttering the user-facing agent list.
- Permission policy and runtime capacity policy are separate controls; BitFun should not encode gateway pressure as prompt instructions.

## 3. Product Positioning

The old positioning was close to:

> "BitFun has context compression to support long conversations."

The revised positioning should be:

> "BitFun is a local Agent Runtime for long-running, auditable, multi-agent work. It preserves task intent, execution evidence, and recovery boundaries even when context is compressed, pruned, or delegated."

This matters because compression is table stakes. The product differentiation is whether BitFun can safely run real engineering workflows over time:

- Deep Review can coordinate several reviewers without losing evidence.
- Agentic Mode can edit, test, retry, and still know exactly what changed.
- Computer Use can summarize screen/action history without dumping every screenshot into context.
- Cowork and Plan modes can remain conversational without inheriting overly aggressive long-task pruning.

## 4. Design Principles

1. **Programmatic facts beat model summaries.**
   If a tool can record "file X was edited" or "test Y failed with exit code 1", do not ask the model to remember it.

2. **Summaries are hints, not authority.**
   Model-generated compression summaries should never override trusted instructions, tool facts, or user-visible state.

3. **Weak models need more structure, not more autonomy.**
   Weak models should receive simpler schemas, smaller scopes, stronger early-stop rules, and more user confirmation.

4. **Subagents need contracts.**
   Isolation helps only if the parent gives a precise work packet and receives a structured result.

5. **Context policy should follow task shape.**
   Long execution tasks and multi-turn conversations fail differently. They should not use exactly the same context strategy.

## 5. Key Measures

### 5.1 Repository System of Record

#### Proposal

Keep always-injected project guidance short. Use top-level and nearest AGENTS files as routing maps, with deeper architecture, verification, and module guidance stored in docs and loaded when relevant.

For BitFun:

- Keep `AGENTS.md` as a short architecture and verification index.
- Use nearest `AGENTS.md` / `AGENTS-CN.md` for touched directories.
- Treat detailed docs such as Deep Review strategy docs as on-demand references.
- Add mechanical checks later for stale pointers and missing linked docs.

#### Benefits

- Reduces initial context pressure.
- Makes project knowledge easier to maintain and review.
- Avoids burying task-specific context under a large instruction blob.
- Aligns with Codex-style AGENTS.md scoping and "map, not manual" guidance.

#### Negative Impacts

- More pointers means more chances for the agent to miss a required doc.
- Weak models may not follow "read this if relevant" instructions reliably.
- Users may think rules disappeared if they are no longer always visible.

#### Weak-Model Experience

Weak models need programmatic doc injection for high-risk scopes. For example:

- Editing `src/web-ui` should automatically include the web-ui agent doc summary.
- Deep Review should automatically include the review-team execution contract.
- Desktop/Tauri API changes should include structured command conventions.

#### Risk Mitigation

- Add a "required context resolver" rather than relying purely on the model.
- Keep critical constraints duplicated as concise capsule fields.
- Log which instruction sources were included in each run.

#### Product Impact

This shifts BitFun toward a repository-aware runtime. It also reduces the product risk that users solve context problems by writing ever-larger instruction files.

### 5.2 Context Trust Boundary

#### Proposal

Assign a trust level to each model-visible context item:

| Trust level | Examples | Can override lower levels? |
| --- | --- | --- |
| `system` | Built-in system/developer policy | Yes |
| `workspace_instruction` | AGENTS.md, memory files, AI rules | Yes, within scoped priority |
| `user_input` | Raw user prompt and attached context | No |
| `tool_observation` | Read/Bash/Git/WebFetch results | No |
| `model_summary` | Compression summary | No |
| `external_artifact` | Imported docs, screenshots, webpages | No |

User text and tool output can mention system-looking tags, but those tags must never upgrade their authority.

#### Benefits

- Reduces prompt injection through user input, code comments, docs, and tool output.
- Gives compression logic a safe ordering rule.
- Allows UI warnings without blocking normal legitimate discussion.
- Helps weak models by making priority explicit and mechanical.

#### Negative Impacts

- False positives can annoy users, especially when discussing prompt markup literally.
- Overly aggressive blocking can break debugging and prompt-authoring workflows.
- Additional metadata increases implementation complexity.

#### Weak-Model Experience

Weak models are more vulnerable to instruction-looking text in tool results. Trust metadata benefits weak models more than strong models because it reduces the need to infer source authority.

#### Risk Mitigation

- Start with escaping, tagging, and warnings instead of hard blocking.
- Block only clear privilege-escalation patterns.
- Keep raw user text accessible for display while sending escaped/annotated text to the model.
- Include tests where a user legitimately discusses `<system_reminder>` and where a malicious tool result contains fake instructions.

#### Product Impact

This is a safety foundation for BitFun as a local runtime. It supports an enterprise-friendly story: local execution with explicit context provenance and priority.

### 5.3 Evidence Ledger

#### Proposal

Record compact, structured facts for important tool events. The ledger survives pruning and feeds the compression contract.

Minimum fields:

```text
event_id
turn_id
tool_name
target_kind
target
status
exit_code_or_error_kind
touched_files
artifact_path
summary
created_at
```

Initial event categories:

- file read/search results
- file write/edit/delete operations
- shell commands and exit codes
- git status/diff/test commands
- context compression events
- subagent start/completion/timeout/cancel
- model output recovery events such as `PartialRecoveryKind`
- input-side context mutation events such as microcompact, model compression, fallback compression, and emergency truncation
- subagent scheduler state transitions such as queued, waiting for capacity, running, retry waiting, completed with partial, failed, and cancelled
- implemented Deep Review Strategy Engine facts such as reviewer role, scope, strategy level, `model_id`, prompt directive, predictive timeout, retry budget, raw `partial_timeout`, normalized scheduler state, and judge decision
- Review Evidence Pack events such as pack id, source kind, source provider, source locator hash, source fingerprint, source collection count, file count, diff line count, pack hash, artifact slice ids, cache hit, and stale status
- session artifact events for subagent results, spill files, and large file write manifests

#### Benefits

- Tool outputs can be pruned without losing the fact that an action happened.
- The agent can preserve modified files and test commands across compression.
- Final responses and review reports can cite operational evidence.
- Repeated work decreases because the agent can see recent failed commands and touched files.

#### Negative Impacts

- Ledger can become a new source of noise if every minor action is included.
- Incorrect ledger entries are dangerous because they look authoritative.
- More storage and retention policy work is required.

#### Weak-Model Experience

This is one of the highest-value measures for weak models. Weak models often forget that a file was already changed or that a command already failed. A short ledger prevents loops and repeated exploration.

#### Risk Mitigation

- Generate ledger facts from tool layer code, not model prose.
- Keep model-visible ledger to a small recent slice:
  - recently touched files
  - latest verification commands
  - latest blocking failures
  - active subagent statuses
- Keep full ledger in session storage for UI/audit, but only inject a summary into model context.
- Treat runtime event producers as untrusted unless they come from core/tool-layer code. Model prose may explain an event, but it must not create authoritative ledger facts by itself.
- Normalize Deep Review raw statuses before projection: `partial_timeout` with evidence should appear as `completed_with_partial`, while timeout without useful evidence should appear as `timed_out`. Keep raw status only as diagnostics.

#### Product Impact

This is the core of BitFun's "auditable local agent" positioning. It also turns compression from lossy summarization into a state-aware runtime behavior.

### 5.4 Snapshot and Recovery

#### Proposal

Introduce recovery boundaries for high-risk operations.

Two levels:

1. **Light checkpoint**
   - Track dirty status, touched files, diff hash, current branch, and latest ledger event.
   - Cheap and always available.

2. **Strong checkpoint**
   - Capture enough state to rollback or create a user-visible recovery point before high-risk edits.
   - Used for auto-fix, batch edits, generated rewrites, and long-running Computer Use flows.

#### Benefits

- Users can trust longer autonomous tasks.
- Failed or weak-model edits become less scary.
- The system can explain what happened before an error.
- Deep Review remediation and Agentic Mode can safely attempt bounded changes.

#### Negative Impacts

- Strong snapshots can be slow or disk-heavy in large repositories.
- Snapshots can create a false sense of safety for external side effects.
- Rollback semantics can be complex when user edits happen concurrently.

#### Weak-Model Experience

Weak models should trigger stronger recovery boundaries earlier. They should also have stricter edit-size thresholds and more user confirmations before broad changes.

#### Risk Mitigation

- Start with light checkpoints only.
- Use strong checkpoints only when the repo is clean enough or when the user explicitly approves.
- Clearly label what is recoverable: local files and git state, not external APIs, databases, or remote side effects.
- Never rollback user edits without explicit approval.

#### Product Impact

Snapshot/recovery moves BitFun toward a controlled execution environment rather than a plain chat assistant that happens to edit files.

### 5.5 Compaction Contract

#### Proposal

Replace free-form compression expectations with a fixed contract. Every model-generated or fallback compression summary must preserve these fields when available:

```text
current_goal
active_scope
hard_constraints
decisions
touched_files
verification_commands
blocking_failures
open_questions
subagent_statuses
budget_state
artifact_refs
deep_review_manifest_summary
review_evidence_pack_summary
next_step
```

Facts such as touched files and verification commands should be populated from the Evidence Ledger whenever possible.
`budget_state` and `artifact_refs` are optional fields populated from the runtime budget plan when a request triggered budget governance, scheduler queueing, spill-to-file, or large file write artifacts. They should stay compact: include state, path/hash/reference, and next action, not raw large content.
`deep_review_manifest_summary` is optional and should be populated only when the active task is a Deep Review. It preserves reviewer roles, scope, strategy level, model slot/model id, timeout/retry budget, and normalized reviewer status without injecting full reviewer reports.
`review_evidence_pack_summary` is optional and should be populated for large source reviews such as PR URLs, local ranges, working tree diffs, patch artifacts, or explicit file snapshots. It preserves pack id, source kind, source provider, source fingerprint, pack hash, slice ids, stale status, and source collection count without injecting raw patch content.

#### Benefits

- Reduces context drift after compression.
- Makes compression quality easier to test.
- Preserves exactly the facts that coding agents most often lose: files, commands, failures, and next step.
- Aligns with Claude-style guidance to preserve modified files and test commands.

#### Negative Impacts

- The contract itself consumes tokens.
- If the model fills unknown fields by guessing, the contract can become misleading.
- Too many fields can make weak models produce lower-quality summaries.

#### Weak-Model Experience

Weak models should receive a shorter contract. Suggested weak-model contract:

```text
Goal:
Scope:
Touched files:
Last tests:
Blocking issue:
Next step:
```

Do not ask weak models to produce nuanced decision histories unless the task requires it.

#### Risk Mitigation

- Populate factual fields programmatically.
- Allow empty fields rather than invented content.
- Validate contract length and truncate low-priority fields first.
- Add tests for compression summaries preserving touched files and test commands.
- Do not inject full spill files, large write chunks, or full reviewer artifacts into the compaction summary. Inject only summaries and references.
- Preserve Deep Review manifest facts from the implemented Strategy Engine, but never duplicate the full prompt block or full reviewer output.
- Preserve Review Evidence Pack refs and stale status, but never duplicate complete source evidence or per-file patch content in the compaction contract.

#### Product Impact

This is the most direct way to make long BitFun sessions feel stable after compaction.

### 5.6 Subagent Work Packet

#### Proposal

Make subagent dispatch structured. Because `deep-review-design.md` is treated as implemented, the first Work Packet should be a compatibility projection of the existing Deep Review launch manifest, not a second schema built from scratch. A Work Packet defines what a subagent may do and what it must return.

Minimum packet:

```text
packet_id
parent_session_id
goal
scope
allowed_tools
forbidden_actions
input_artifacts
review_evidence_pack_id
evidence_slice
timeout_seconds
queue_timeout_seconds
run_timeout_seconds
stream_idle_timeout_seconds
output_budget
output_schema
expected_parent_state_patch
```

Minimum result:

```text
packet_id
status
summary
findings_or_changes
evidence
touched_files
verification
artifact_refs
open_risks
```

#### Benefits

- Parent context stays smaller.
- Subagents receive clearer scope and permissions.
- Judge/orchestrator logic can merge results more reliably.
- Deep Review can better handle partial, timed-out, or cancelled reviewers.

#### Negative Impacts

- Work packets can reduce flexibility for open-ended research.
- Schema validation and fallback handling add code paths.
- Poorly designed packets can hide important context from subagents.

#### Weak-Model Experience

Weak models benefit from clear packets but struggle with large schemas. Use minimal schemas for weak models and require short outputs.

#### Risk Mitigation

- Start by projecting the implemented Deep Review manifest into this shape.
- Keep initial schemas small.
- Allow `partial` status.
- Parent agent must validate required fields and ask for repair only once before falling back to a plain summary.
- Do not make Work Packet responsible for actual execution order or gateway pressure. The runtime budget plan's `SubagentScheduler` and gateway request limiter own queueing, permits, retry, and effective concurrency.
- Do not duplicate `reviewTeamService.ts` / backend role definitions. The Work Packet projection must consume the canonical Deep Review manifest and preserve `model_id`, `prompt_directive`, reviewer role, scope, timeout, and retry policy.
- Do not let each reviewer reconstruct the same complete source evidence. When a Review Evidence Pack exists, the Work Packet should pass artifact slice refs and stale policy, not prompt the reviewer to refetch PR files, rerun full local ranges, or rebuild the whole patch again.

#### Product Impact

This turns BitFun's multi-agent story from prompt-level parallelism into runtime-level orchestration. It is especially important for Code Review Team and future Team Mode workflows.

### 5.7 Adaptive Context Policy

#### Proposal

Use two first-class context profiles:

| Profile | Modes | Default policy |
| --- | --- | --- |
| `long_task` | Agentic, Deep Review, Deep Research, Computer Use, Team Mode | active ledger, aggressive tool pruning, compaction contract, subagent isolation |
| `conversation` | Cowork, Plan, general Q&A | preserve recent user intent, conservative pruning, fewer automatic subagents |

Later, these profiles can branch by model capability.

#### Benefits

- Matches context strategy to failure mode.
- Keeps conversation modes from feeling over-managed.
- Gives long-running modes enough structure to survive compression.

#### Negative Impacts

- Users may not understand why modes behave differently.
- Switching profiles mid-session can be confusing.
- Profile-specific bugs can appear.

#### Weak-Model Experience

Weak models should default to lower autonomy:

- smaller scope
- more explicit confirmations
- fewer automatic subagents
- stricter loop detection
- simpler compression contract

#### Risk Mitigation

- Derive default profile from agent type.
- Make profile visible but not noisy in UI.
- Allow per-session override.
- Avoid automatic profile switching until telemetry proves it is safe.

#### Product Impact

This makes BitFun's agent modes real runtime modes, not just prompt skins.

### 5.8 Context Health Score

#### Proposal

Track internal health signals:

- token usage ratio
- compacted turn count
- pruned tool output count
- ledger freshness
- repeated tool signature count
- consecutive failed commands
- subagent timeout/cancel count
- scheduler queued/retry count
- partial recovery count by kind
- Deep Review reviewer status counts, including normalized `completed_with_partial`, `timed_out`, `retry_waiting`, and raw diagnostic status when present
- Deep Review retry budget used vs allowed
- artifact/spill pressure
- large file write incomplete count
- unresolved open questions
- compression circuit breaker state

Use this score internally first. Only show simple user-facing states:

- Healthy
- Context pressure rising
- Recommend compacting
- Recommend splitting work
- Needs user decision

#### Benefits

- Prevents compaction loops and repeated failed tool calls.
- Gives the runtime a reason to ask the user before continuing.
- Helps tune weak-model behavior over time.

#### Negative Impacts

- A visible score can create user anxiety.
- Bad thresholds can interrupt good workflows.
- Health calculation can become a bag of arbitrary heuristics.

#### Weak-Model Experience

Weak models should hit early-stop and user-decision thresholds sooner. Strong models can continue longer before escalation.

#### Risk Mitigation

- Keep health score internal in P0/P1.
- Log telemetry locally before exposing UI.
- Start with a small set of high-signal metrics:
  - repeated identical tool calls
  - consecutive failed commands
  - subagent timeout count
  - compression failures
- Use action-oriented UI states instead of raw scores until product telemetry proves users benefit from more detail.

#### Product Impact

Health score supports a future "why did BitFun pause?" explanation and can become a key differentiator for long-task reliability.

### 5.9 Runtime Budget Integration Boundary

The runtime budget plan adds several producers of structured facts:

- output truncation and partial recovery
- context mutation caused by budget governance
- scheduler queue/retry/run/completion state
- gateway overload classification and effective concurrency
- subagent/reviewer artifacts
- Deep Review Strategy Engine facts from the implemented launch manifest
- Review Evidence Pack facts from Deep Review source resolver / provider preflight
- spill-to-file artifacts
- large file write manifests and incomplete-write recovery

This architecture should consume those facts through Evidence Ledger and Context Health, but it should not duplicate their execution policies.

| Runtime area | This document consumes | Runtime budget plan owns |
| --- | --- | --- |
| `PartialRecoveryKind` | ledger facts and recovery summaries | classification, retry behavior, UI recovery event |
| `ContextMutationKind` | compaction history, health scoring, contract fields | budget trigger, mutation event emission, user progress state |
| `SubagentScheduler` | active subagent statuses for compaction and health | queue state machine, gateway permits, retry classifier, cancellation cleanup |
| Deep Review Strategy Engine | reviewer role/scope/model/strategy facts, normalized partial state, retry budget summary | role selection, strategy directives, predictive timeout, Deep Review-specific policy |
| Review Evidence Pack | pack refs, hash, source kind/provider/fingerprint, slice ids, stale status, source collection count | source resolution, provider evidence collection, normalized change snapshot creation, artifact storage, staleness policy |
| spill/artifact | artifact references, hash, sensitivity flag, retention hints | file creation, permissions, cleanup, inline summary policy |
| Large File Write Protocol | manifest facts and incomplete-write recovery state | chunking, hash validation, temp file, atomic commit, abort/continue |

Default dependency direction:

1. Phase 1 truncation recovery can ship before Evidence Ledger is complete, as long as it emits structured events.
2. Context P0/P1 should improve Phase 2+ budget recovery quality, especially compaction and health scoring.
3. Phase 3/4 artifact and large-write work should record ledger facts once the ledger exists, but should not block on a globally persisted ledger format.

If the documents appear to disagree, prefer this rule: **Deep Review defines review policy and roles; context architecture defines what the model may know and trust; runtime budget governance defines what the system may execute, retry, queue, spill, or block.**

## 6. Cross-Measure Risks

| Combination | Risk | Mitigation |
| --- | --- | --- |
| Evidence Ledger + Compaction Contract | Contract can become too long if it includes too many ledger facts. | Inject only latest touched files, latest tests, latest failures, and active subagent statuses. |
| Snapshot + Weak-Model Auto-fix | Weak model may attempt broad edits because rollback exists. | Add edit-size thresholds, approval gates, and scope caps. |
| Subagent Work Packet + Adaptive Policy | Over-delegation can fragment reasoning. | Default automatic delegation off except Deep Review / Deep Research. |
| Trust Boundary + UX | Too many warnings can feel hostile. | Warn only for high-confidence injection patterns; otherwise silently escape. |
| Repository System of Record + Weak Models | Weak models may not follow document pointers. | Programmatically resolve required docs for touched paths. |
| Context Health Score + Autonomy | Bad thresholds can pause too early or too late. | Start telemetry-only, then enable advisory mode, then enforcement. |
| Runtime Budget Events + Evidence Ledger | Duplicated ownership can make the same failure appear as two conflicting facts. | Runtime emits typed events; ledger records facts and projections; model summaries never redefine event meaning. |
| Work Packet + SubagentScheduler | Packet timeouts can conflict with queue/run/idle timeout semantics. | Packet may request timeout budgets; scheduler owns final queue, run, idle, retry, and parent deadline enforcement. |
| Artifact References + Compaction | Summaries can lose the fact that full evidence exists on disk. | Compaction Contract must preserve artifact refs, hash/status, and next action while excluding raw large content. |
| Deep Review Manifest + Work Packet | A second Work Packet schema can drift from implemented reviewer roles, `model_id`, or prompt directives. | Generate the Work Packet projection from the implemented Deep Review manifest; do not duplicate role metadata in context code. |
| Review Evidence Pack + Work Packet | Reviewer packets can still prompt subagents to reconstruct complete source evidence, wasting timeout on duplicate evidence gathering. | Pack refs and slice ids must be first-class packet inputs; full source reconstruction is fallback-only and diagnosable. |
| Deep Review Partial Status + Ledger | Raw `partial_timeout` can conflict with normalized scheduler states. | Store raw status as diagnostics; project model-visible status as `completed_with_partial` or `timed_out` based on evidence. |

## 7. Weak Model Policy

BitFun should not treat weak models as simply cheaper strong models. They need a different runtime posture.

### 7.1 Weak Model Defaults

- Prefer `conversation` profile unless user explicitly starts a long-task mode.
- Use shorter compaction contracts.
- Inject more programmatic facts and fewer prose summaries.
- Require user confirmation for broad edits, destructive commands, and automatic remediation.
- Cap subagent fan-out more aggressively.
- Stop earlier on non-convergence.

### 7.2 Strong Model Defaults

- Allow longer autonomous runs.
- Permit richer Work Packet schemas.
- Allow more nuanced compression summaries.
- Use higher thresholds before asking the user to intervene.

### 7.3 Model Capability Inputs

Initial model capability classification can be heuristic:

- context window size
- configured model slot (`fast`, `primary`, `reasoning`)
- known provider/model family
- user-selected "safe mode" preference
- observed loop/failure rate in the current session

Do not rely only on model name. Runtime behavior should be measured.

## 8. Revised Priority

### P0: Safety and Facts

1. Context Trust Boundary
2. Evidence Ledger minimum viable version
3. Prompt markup escaping/warning tests

Why first:

- High benefit for both strong and weak models.
- Low dependency on subjective model summarization.
- Makes later compression and pruning safer.

### P1: Compression Reliability

1. Compaction Contract
2. Ledger-backed touched-files/test-command preservation
3. Minimal Context Health telemetry
4. Read-only consumption of runtime budget events when available

Why second:

- Builds directly on ledger facts.
- Makes existing compression less lossy without changing the whole orchestration model.
- Improves Phase 2 budget recovery quality without blocking Phase 1 truncation stop-the-bleeding work.

### P2: Recovery and Deep Review Handoff

1. Light Snapshot checkpoints
2. Deep Review manifest to Work Packet compatibility projection
3. Review Evidence Pack projection for PR URL / local range / working tree / patch artifact reviews
4. Ledger projection for already-preserved partial/timed-out subagent results
5. Ledger projection for scheduler states and subagent artifacts

Why third:

- Higher complexity.
- Best validated in Deep Review, where subagent isolation and Strategy Engine behavior already exist.
- Should consume `SubagentScheduler` states from the runtime budget plan, not implement a second scheduler here.

### P3: Runtime Policy

1. Adaptive Context Profile
2. Weak-model policy gates
3. Advisory UI for context health
4. Product decision on whether short background compaction can be enabled by default in conversation mode

Why later:

- Needs telemetry from P0/P1.
- User-facing behavior must be tuned carefully.

## 9. Implementation Plan

### Task 1: Add Context Trust Metadata

**Goal:** represent trust source without changing behavior first.

**Likely files:**

- `src/crates/core/src/agentic/core/message.rs`
- `src/crates/core/src/agentic/core/prompt_markup.rs`
- `src/crates/core/src/agentic/session/session_manager.rs`
- `src/crates/core/src/agentic/session/compression/fallback/builder.rs`

**Steps:**

1. Add `ContextTrustLevel` enum with values:
   - `System`
   - `WorkspaceInstruction`
   - `UserInput`
   - `ToolObservation`
   - `ModelSummary`
   - `ExternalArtifact`
2. Add optional trust metadata to message metadata.
3. Set trust level when creating:
   - actual user input
   - internal reminders
   - compression summaries
   - tool results
4. Add tests ensuring existing serialization remains backward compatible.
5. Add tests for prompt markup in user input being treated as user text.

**Verification:**

- `cargo test -p bitfun-core prompt_markup -- --nocapture`
- `cargo test -p bitfun-core compression -- --nocapture`

### Task 2: Add Evidence Ledger Core Types

**Goal:** create a programmatic fact store independent of model summaries.

**Likely files:**

- Create `src/crates/core/src/agentic/session/evidence_ledger.rs`
- Modify `src/crates/core/src/agentic/session/mod.rs`
- Modify tool execution path in `src/crates/core/src/agentic/execution/round_executor.rs`
- Modify file/shell/git tool result handling as needed.

**Steps:**

1. Define `EvidenceLedgerEvent`.
2. Define `EvidenceLedgerSummary` for model-visible projection.
3. Add append/read APIs scoped by `session_id` and `dialog_turn_id`.
4. Capture command status, touched files, and artifact pointers where available.
5. Keep the first implementation in-memory or session-local; avoid global persistence until format stabilizes.
6. Accept optional runtime budget event facts:
   - `PartialRecoveryKind`
   - `ContextMutationKind`
   - scheduler state
   - artifact/spill/large-write manifest reference
7. Add unit tests for summarizing:
   - touched files
   - latest failed commands
   - latest verification commands
   - active scheduler states
   - artifact references without raw content

**Verification:**

- `cargo test -p bitfun-core evidence_ledger -- --nocapture`
- `cargo test -p bitfun-core agentic::execution -- --nocapture`

### Task 3: Integrate Ledger With Microcompact

**Goal:** allow pruning old tool output while preserving important operational facts.

**Likely files:**

- `src/crates/core/src/agentic/session/compression/microcompact.rs`
- `src/crates/core/src/agentic/execution/execution_engine.rs`
- `src/crates/core/src/agentic/session/evidence_ledger.rs`

**Steps:**

1. Before clearing compactable tool results, ensure a ledger event exists for that tool result.
2. Add microcompact stats for events preserved.
3. Do not clear recent failed command outputs until the ledger summary includes their error kind.
4. Add tests:
   - old successful read result is cleared and ledger keeps target path
   - failed command remains or is summarized safely
   - TodoWrite and compression summary are not pruned incorrectly

**Verification:**

- `cargo test -p bitfun-core microcompact -- --nocapture`

### Task 4: Implement Compaction Contract

**Goal:** make compression preserve fixed critical fields.

**Likely files:**

- `src/crates/core/src/agentic/session/compression/compressor.rs`
- `src/crates/core/src/agentic/session/compression/fallback/builder.rs`
- `src/crates/core/src/agentic/core/message.rs`
- `src/crates/core/src/agentic/session/evidence_ledger.rs`

**Steps:**

1. Add a `CompressionContract` struct.
2. Populate factual fields from Evidence Ledger:
   - touched files
   - verification commands
   - blocking failures
   - subagent statuses
   - budget state
   - artifact references
3. Update model compression prompt to require the contract fields.
4. Update fallback compression builder to emit the same fields without model help.
5. Add tests proving touched files and test commands survive compression.
6. Add tests proving spill/artifact references survive without injecting raw large content.
7. Add a weak-model short contract mode behind config or capability detection.

**Verification:**

- `cargo test -p bitfun-core compression -- --nocapture`
- targeted manual long-session compression smoke test

### Task 5: Add Context Health Telemetry

**Goal:** observe before changing user-facing behavior.

**Likely files:**

- `src/crates/core/src/agentic/execution/execution_engine.rs`
- `src/crates/core/src/agentic/session/compression/microcompact.rs`
- `src/crates/core/src/agentic/session/evidence_ledger.rs`
- optional frontend later: `src/web-ui/src/flow_chat/*`

**Steps:**

1. Track:
   - token usage ratio
   - microcompact count
   - full compression count
   - compression failures
   - repeated tool signatures
   - consecutive failed commands
2. Emit English-only logs.
3. Add a small internal `ContextHealthSnapshot` type.
4. Keep UI hidden for now.
5. Add tests for repeated tool signature scoring.

**Verification:**

- `cargo test -p bitfun-core context_health -- --nocapture`
- `cargo check --workspace`

### Task 6: Add Light Snapshot Checkpoints

**Goal:** record recovery boundaries before risky edits without promising full rollback.

**Likely files:**

- `src/crates/core/src/agentic/session/evidence_ledger.rs`
- `src/crates/core/src/service/git/*`
- tool implementations for Edit/Write/Delete/Bash/Git

**Steps:**

1. Add `CheckpointCreated` ledger event.
2. Capture:
   - current branch
   - dirty state summary
   - touched file list
   - diff hash when cheap
3. Create checkpoint before high-risk operations:
   - batch edits
   - auto-fix
   - destructive file operations
4. Do not implement automatic rollback in this phase.
5. Add tests for checkpoint event creation.

**Verification:**

- `cargo test -p bitfun-core checkpoint -- --nocapture`
- manual edit flow verifying ledger output

### Task 7: Project Deep Review Manifest Into Work Packet

**Goal:** standardize the already-implemented Deep Review launch manifest as a Work Packet projection without changing reviewer dispatch ownership.

**Likely files:**

- `src/crates/core/src/agentic/agents/prompts/deep_review_agent.md`
- `src/crates/core/src/agentic/deep_review_policy.rs`
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- `src/web-ui/src/shared/services/reviewTeamService.ts`
- `src/web-ui/src/flow_chat/services/DeepReviewService.ts`

**Steps:**

1. Read the implemented Deep Review launch manifest produced by the review team strategy flow.
2. Project it into a Work Packet markdown/JSON block for ledger/compaction/judge consumption.
3. Include:
   - packet id
   - role
   - assigned scope
   - allowed tools
   - timeout
   - queue/run/idle timeout requests
   - output budget
   - input artifact references
   - `model_id`
   - `prompt_directive`
   - strategy level
   - retry budget
   - required output fields
4. Update reviewer prompt only if it does not already return packet id and status.
5. Update judge prompt to treat missing packet id/status as lower confidence.
6. Ensure the packet does not promise actual parallel execution; scheduler capacity is owned by the runtime budget plan.
7. Add frontend service tests proving the Work Packet projection matches the implemented manifest.
8. Add Rust tests for Deep Review policy compatibility.

**Verification:**

- `cargo test -p bitfun-core deep_review -- --nocapture`
- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run test:run src/shared/services/reviewTeamService.test.ts`

### Task 7A: Project Review Evidence Pack Into Work Packets

**Goal:** preserve shared source evidence across compaction and prevent reviewers from repeatedly reconstructing the same PR, local range, working tree diff, patch artifact, or file snapshot.

**Likely files:**

- `src/crates/core/src/agentic/deep_review_policy.rs`
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- `src/crates/core/src/agentic/coordination/coordinator.rs`
- `src/web-ui/src/flow_chat/services/DeepReviewService.ts`

**Steps:**

1. Consume the runtime budget plan's source-agnostic `ReviewEvidencePack` artifact emitted during Deep Review launch preflight.
2. Record pack id, source kind, source provider, source locator hash, source fingerprint, source collection count, pack hash, stale status, and slice ids in Evidence Ledger.
3. Project the relevant `evidence_slice` refs into each reviewer Work Packet.
4. Preserve `review_evidence_pack_summary` in the Compaction Contract without injecting raw patch content.
5. Add tests proving that compaction retains pack refs and that reviewer packets do not request complete source reconstruction when a pack exists.

**Verification:**

- `cargo test -p bitfun-core deep_review -- --nocapture`
- ledger projection tests for pack refs, hash, and stale status

### Task 8: Ledger Projection for Partial Subagent Results

**Goal:** record already-preserved reviewer partial results as trustworthy ledger facts and normalized context state.

**Likely files:**

- `src/crates/core/src/agentic/coordination/coordinator.rs`
- `src/crates/core/src/agentic/execution/execution_engine.rs`
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- `src/web-ui/src/flow_chat/components/TaskDetailPanel/TaskDetailPanel.tsx`

**Steps:**

1. Inspect current partial recovery data from stream processing and Deep Review result objects.
2. Treat existing `partial_timeout` with evidence as `completed_with_partial` for model-visible projections.
3. Treat timeout without useful evidence as `timed_out`.
4. Record timeout, partial result, raw status, normalized status, reviewer role, scope, and artifact ref in Evidence Ledger.
5. Update Deep Review judge guidance only if it still consumes raw timeout strings instead of normalized status.
6. Map scheduler/runtime status into parent-visible result status:
   - `queued`
   - `retry_waiting`
   - `timed_out`
   - `completed_with_partial`
   - `failed`
7. Add tests for raw-to-normalized status projection and completed-with-partial paths.

**Verification:**

- `cargo test -p bitfun-core coordination -- --nocapture`
- `cargo test -p bitfun-core deep_review -- --nocapture`

### Task 9: Add Adaptive Context Profiles

**Goal:** introduce policy without exposing too much UI complexity.

**Likely files:**

- `src/crates/core/src/agentic/agents/mod.rs`
- `src/crates/core/src/agentic/execution/execution_engine.rs`
- `src/crates/core/src/agentic/session/session_config.rs` or equivalent config type
- `src/web-ui/src/shared/services/reviewTeamService.ts` only if Deep Review needs explicit policy display

**Steps:**

1. Define `ContextProfile`: `LongTask` and `Conversation`.
2. Map default profiles by agent type.
3. Use profile to choose:
   - microcompact aggressiveness
   - compression contract length
   - subagent fan-out caps
   - health threshold behavior
4. Add weak-model override mode:
   - shorter contract
   - stricter loop threshold
   - lower auto-delegation cap
5. Keep UI advisory only.

**Verification:**

- `cargo test -p bitfun-core context_profile -- --nocapture`
- `cargo check --workspace`

### Task 10: Frontend Observability and UX

**Goal:** expose reliability state without making users manage internals.

**Likely files:**

- `src/web-ui/src/flow_chat/*`
- `src/web-ui/src/component-library/components/FlowChatCards/*`
- `src/web-ui/src/locales/*`

**Steps:**

1. Add a compact context status surface only when action is needed.
2. Show simple statuses:
   - Context pressure rising
   - Compression preserved key facts
   - Reviewer timed out with partial result
   - User decision needed
   - Waiting for model capacity
   - Organizing context to continue
3. Avoid showing raw health score initially.
4. Add i18n entries.
5. Add tests for rendering timeout/partial states.
6. Add tests that ordinary conversation mode does not show context health UI for low-confidence or short background recovery.

**Verification:**

- `pnpm run lint:web`
- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run test:run`

## 10. Recommended Validation Matrix

| Scenario | Strong model expectation | Weak model expectation |
| --- | --- | --- |
| Long edit/test loop | compression preserves touched files and tests | short contract, early stop on repeated failure |
| Prompt injection in file content | tool output remains observation only | same behavior, with stronger warning |
| Deep Review timeout | partial reviewer evidence retained | partial evidence retained, lower confidence |
| Large diff review | work packets split reviewers predictably | smaller fan-out, more user confirmation |
| PR / local range / patch Deep Review evidence | one source-agnostic Review Evidence Pack survives compaction and feeds reviewer slices | smaller pack summary, no repeated complete source reconstruction |
| Implemented Deep Review Strategy Engine | manifest facts survive compaction without duplicating reviewer definitions | manifest projected to shorter Work Packet; no duplicate scheduler |
| Conversation mode | recent user intent preserved | conservative compression and fewer automatic actions |
| Runtime budget recovery | recovery/context mutation events become ledger facts | no low-confidence blocking; smaller repair scope |
| Subagent scheduler queue | queued/retry/running states preserved for judge and compaction | lower fan-out; clear user-visible waiting state |
| Large file write | manifest and artifact refs survive compaction | prefer patch/hunk or smaller chunks |

## 11. Open Questions

1. Where should the full Evidence Ledger persist long term: session JSON, `.bitfun/sessions/{id}/ledger.jsonl`, or existing event storage?
2. Should strong checkpoints use git-native mechanisms, an internal snapshot store, or both?
3. Which model capability signal is reliable enough for weak-model policy: configured model slot, provider metadata, observed runtime behavior, or a user setting?
4. Should Context Health remain action-oriented UI only, or should raw health score be exposed in an advanced diagnostics panel?
5. Should Work Packet become a generic TaskTool schema or stay a projection of the implemented Deep Review manifest until validated? Current recommendation: keep it as a projection until scheduler/artifact behavior is proven.
6. Should short background compaction be enabled by default in conversation mode after telemetry, or remain opt-in until the product has stronger confidence?
7. Should session artifact retention be governed by the Evidence Ledger store, the runtime budget artifact policy, or a shared retention service?

## 12. Cross-Document Product Decisions Pending Confirmation

The current recommendation is to keep this document and `agent-runtime-budget-governance-design.md` separate but aligned. When they touch similar problems, use the following default decisions until product confirmation changes them:

| Decision | Recommendation | Why |
| --- | --- | --- |
| Background compaction in normal conversation | Allow only short, non-blocking, telemetry-backed compaction by default; long or lossy mutation must show progress/diagnostics. | Codex-style automatic compaction is a strong direction, but BitFun's product rule forbids adding low-confidence user-visible interruptions. |
| Generic Work Packet | Project the implemented Deep Review manifest first; do not make it a universal TaskTool schema in early phases. | Claude/OpenCode show subagents are useful with permissions and isolation, but generic delegation increases blast radius before BitFun has enough runtime evidence. |
| Review Evidence Pack | Parent Deep Review preflight resolves the source and generates the pack once; context only records refs/hash/source fingerprint/slices/stale state. | Repeated PR/file/diff collection is shared evidence, not reviewer-specific reasoning. |
| Raw Context Health UI | Keep raw score internal; show only actionable states. | Users need recovery guidance, not another number to interpret. |
| Subagent parallelism | Do not promise simultaneous start for every subagent. Use runtime-bounded scheduling and show queue/retry/running states. | Gateway capacity and provider behavior are runtime facts, not prompt-level guarantees. |
| Artifact ownership | Runtime creates artifacts; Evidence Ledger records facts, refs, hashes, status, and sensitivity flags. | This keeps storage mechanics separate from model-visible facts. |

## 13. Final Recommendation

Implement this in the order of state authority:

1. Trust boundaries define what can be believed.
2. Evidence ledger defines what happened.
3. Compaction contract defines what must survive.
4. Snapshot/checkpoint defines what can be recovered.
5. Work packets define what can be delegated.
6. Adaptive policy defines how much autonomy each model and mode should get.

This order keeps the project from adding automation before the runtime can preserve facts. It also gives weak models a better user experience: fewer vague summaries, more explicit state, smaller scopes, and earlier handoff to the user when the task stops converging.

The runtime budget plan should be treated as the first major consumer of this architecture, not as a replacement for it. Its recovery, scheduler, spill, and large-write events should feed the ledger and context health once those foundations exist.

Implemented Deep Review should be treated as the first high-value producer of structured facts. Its manifest, reviewer roles, strategy directives, partial evidence, retry budget, and judge output should feed the ledger and compaction contract through projections, not through duplicated role definitions or a second scheduler.

Large-source Deep Review should also treat shared review evidence as a first-class artifact. A parent-generated source-agnostic Review Evidence Pack is the preferred source of truth for PR URLs, local ranges, working tree diffs, patch artifacts, and explicit file snapshots; context management should preserve its refs and stale status, while reviewers consume slices instead of independently reconstructing the same source evidence.

The implementation rule is therefore convergence first: before adding a context-side module for task packets, retry state, partial status, artifacts, budget thresholds, or UI health states, verify whether Deep Review or the runtime budget plan already owns the behavior. If an owner exists, context code should add a projection, adapter, or ledger mapping only. If no owner is sufficient, the proposal must name the new owner, migration path, rollback path, and the duplicated behavior it will retire.

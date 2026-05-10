# Agent Runtime Subagent Scheduling Plan

> Status: coarse-grained planning draft with Deep Review queue-liveness addendum
> Scope: BitFun Agent Runtime, Task subagents, Deep Review-internal reviewer scheduling, model-gateway capacity handling
> Non-goal: this document does not prescribe exact code edits or UI copy.

## Background

BitFun already has the foundations for bounded subagent execution:

- `src/crates/core/src/service/config/types.rs` defines `ai.subagent_max_concurrency`.
- `src/crates/core/src/agentic/coordination/coordinator.rs` acquires a semaphore before hidden subagent execution.
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs` routes `Task` calls through `coordinator.execute_subagent(...)`.
- `src/crates/core/src/agentic/deep_review_policy.rs` bounds Deep Review roles, timeouts, file splitting, and same-role reviewer count.
- `src/web-ui/src/shared/services/reviewTeamService.ts` builds the Deep Review manifest and model-facing execution policy.

The current weak point is that there are multiple concepts currently close together:

- Deep Review reviewer parallelism: how many reviewers from the same Deep Review turn may run at once.
- Generic hidden-subagent execution: how ordinary Task subagents are admitted and timed out.
- Model-gateway capacity: whether the configured provider can accept another streaming request.

Some model gateways, especially local or self-hosted vLLM deployments, may allow only a small number of concurrent requests. Deep Review can ask several reviewers to run in parallel, and each reviewer becomes a streaming model request. If the gateway rejects the burst, a reviewer may fail even though it would have succeeded after waiting in a queue.

The target direction for Deep Review is not a global subagent queue. It is a Deep Review-internal, observable reviewer scheduler with provider-capacity recovery. Generic subagent scheduling can be revisited later, but Deep Review should not use ordinary main-session or cross-session subagent activity as reviewer-cap input.

## Competitor Signals

### Codex

OpenAI describes Codex as a coding agent that can work on many tasks in parallel, with each task running independently in an isolated environment and producing evidence through terminal logs and test outputs. Codex also uses `AGENTS.md` as scoped project guidance that tells the agent how to navigate the repo and which commands to run.

Useful takeaways for BitFun:

- Parallelism is product-level, but each task should have its own execution envelope.
- Users should be able to monitor progress and inspect evidence, not just see a final answer.
- Project-local instructions and verification rules belong in the runtime contract, not only in prompt text.
- Long-running review work should not be modeled as a one-minute per-queue-item wait. User-visible progress and evidence matter more than prematurely timing out queued work.

Reference: [Introducing Codex](https://openai.com/index/introducing-codex/)

### Claude Code

Claude Code documents subagents as independent assistants with their own context window, system prompt, tool access, and permissions. Built-in subagents include read-only Explore and Plan agents, plus a general-purpose agent. Claude also exposes model selection for subagents and permission checks around subagent execution.

Claude Code auto mode is especially relevant: subagent work is checked at spawn time, during execution, and after completion. This suggests a useful separation between dispatch policy, tool permission policy, and result trust policy.

Useful takeaways for BitFun:

- A subagent is not just a model call; it is a controlled execution unit with context, model, tool, permission, and result boundaries.
- Read-only review subagents should have stronger retry semantics than write-capable subagents.
- Safety decisions can happen at multiple lifecycle points, not just before launch.
- Session-scoped subagents and separate-session agent teams are different product concepts; Deep Review reviewer caps should be scoped to one Deep Review turn, not to all user sessions.

References:

- [Claude Code subagents](https://code.claude.com/docs/en/sub-agents)
- [Claude Code permission modes](https://code.claude.com/docs/en/permission-modes)
- [Claude Code auto mode](https://www.anthropic.com/engineering/claude-code-auto-mode)

### OpenCode

OpenCode separates primary agents from subagents. Its built-in Plan primary agent is restricted for planning and analysis, while Build has broader tool access. Its Explore subagent is read-only, and its General subagent is used for parallel units of work.

Useful takeaways for BitFun:

- Agent mode and permission profile should be visible first-class runtime metadata.
- Plan/review/explore agents can be treated as low-risk, retryable work when they are read-only.
- "Parallel units of work" should still be mediated by runtime permissions and status reporting.

Reference: [OpenCode agents](https://opencode.ai/docs/agents/)

## Design Goals

1. Keep BitFun's product logic platform-agnostic. The queue and scheduling rules should live in core runtime services, with UI and desktop surfaces acting as adapters.
2. Preserve Deep Review's reviewer-team semantics while preventing gateway overload.
3. Guarantee useful forward progress when a model gateway allows at least one active request.
4. Make waiting, retrying, throttling, and partial completion visible to the user.
5. Separate prompt-level orchestration from runtime-level enforcement. Prompts can request parallelism; the runtime decides what can safely run.
6. Support weak or local models by making concurrency, retry, timeout, and partial-result handling explicit.
7. Keep Deep Review reviewer concurrency scoped to one Deep Review turn. Main-chat subagents, other sessions, and subagents inside other sessions must not reduce the Deep Review reviewer cap.
8. Treat fixed queue deadlines as outage detection, not normal progress control. If any Deep Review reviewer is actively running, queued reviewers should continue waiting; the active reviewer's own run timeout is the primary fallback.

## Proposed Runtime Model

Introduce a Deep Review reviewer scheduling concept owned by the core runtime. It should be implemented as a Deep Review adapter first. A generic `SubagentScheduler` is a deferred platform project and should not be introduced as a side effect of fixing Deep Review.

At a high level:

```text
DeepReview agent
  -> TaskTool with deep_review_run_manifest
  -> DeepReview reviewer admission adapter
  -> Hidden reviewer subagent session
  -> ExecutionEngine
  -> Provider adapter / model gateway
```

The Deep Review admission adapter owns reviewer lifecycle state and queue liveness for a single Deep Review turn. Provider adapters and future AI request limiting own gateway-level capacity. These are related, but not identical:

- Deep Review reviewer scheduling limits how many reviewers from the same Deep Review turn execute at once.
- Provider capacity recovery handles actual model-gateway saturation after an execution attempt reaches the model layer.
- Generic hidden-subagent capacity remains outside this feature unless separately approved as a platform behavior change.
- Deep Review policy limits what the DeepReview agent is allowed to spawn.

## Queue State Machine

Every scheduled subagent should have an explicit runtime state:

```text
accepted
queued
waiting_for_capacity
running
retry_waiting
completed
completed_with_partial
failed
cancelled
timed_out
```

Suggested transitions:

- `accepted -> queued`: Task call is valid and accepted by policy.
- `queued -> waiting_for_capacity`: task is ready but no slot is currently available.
- `waiting_for_capacity -> running`: Deep Review reviewer admission grants a reviewer slot, and the provider execution path is available.
- `running -> completed`: subagent returns normally.
- `running -> completed_with_partial`: subagent failed after producing useful reviewer output.
- `running -> retry_waiting`: transient gateway failure before useful output.
- `retry_waiting -> queued`: retry backoff elapsed.
- `queued/running -> cancelled`: user or parent session cancelled.
- `queued/running -> failed`: non-retryable error or retry budget exhausted.

This state machine should be surfaced through existing agentic events or new subagent scheduling events so the UI can show actual queue pressure rather than only "waiting for model response".

## Dynamic Concurrency

Keep `ai.subagent_max_concurrency` as the configured ceiling, but add an adaptive effective limit:

```text
configured_max = user or default setting
effective_max = runtime-adjusted value, clamped to [1, configured_max]
```

Recommended control policy:

- Start with `effective_max = configured_max`.
- On gateway concurrency/rate-limit overload, reduce `effective_max` quickly.
- On sustained successful completions, increase `effective_max` slowly.
- Never reduce below 1.
- Keep separate counters per gateway key when possible.

Gateway key examples:

- Explicit `concurrency_key` on model config.
- Otherwise `provider + base_url` or normalized request URL.
- Optional model-level override when a provider has per-model limits.

The minimum forward-progress invariant is:

> If the parent session is not cancelled, the queue is not empty, and the model gateway can run at least one request, at least one eligible subagent should eventually run.

## Retry Classification

Only retry errors that are likely to succeed after waiting. A coarse initial classifier is enough.

Retryable:

- HTTP 429 or provider-specific rate-limit errors.
- vLLM or gateway messages indicating concurrency saturation, overload, queue full, server busy, or capacity exceeded.
- Connection reset or stream closed before any effective output.
- Transient network failure before the subagent made progress.

Not retryable:

- Deep Review policy violation.
- Invalid subagent type or missing workspace.
- Auth, quota, billing, permission, model-not-found, or invalid API key.
- User cancellation.
- Tool permission denial.
- Write-capable subagent failure after state-changing actions.

Conditionally retryable:

- Read-only subagent failed after partial output: keep the partial output and ask the parent reviewer/judge to continue with reduced confidence.
- Write-capable subagent failed before any tool execution: retry only if the execution history proves no state-changing tools ran.

Retry attempts should use exponential backoff with jitter and a small max retry count. Deep Review reviewers can tolerate a small retry budget; general write-capable workers should be more conservative.

## Timeout Semantics

The existing `timeout_seconds` is useful, but low-concurrency queues need sharper semantics:

- `provider_queue_timeout_seconds`: maximum time to wait for transient provider capacity, usually based on `Retry-After` or a short bounded backoff.
- `local_queue_liveness_timeout_seconds`: outage detector for a Deep Review queue with no active reviewer and no executable path. This is not a per-reviewer queue item timeout.
- `run_timeout_seconds`: maximum active execution time after the subagent starts.
- `stream_idle_timeout_secs`: maximum silence between model stream chunks.
- `parent_deadline`: optional end-to-end deadline for the whole parent operation.

For Deep Review, reviewer timeout should primarily mean active review time. Queue waiting should be tracked separately so a local vLLM gateway does not cause reviewers to time out before they start.

The queue-liveness rule is:

```text
if active_deep_review_reviewer_count > 0:
  keep queued reviewers waiting; do not expire them through a short fixed queue timeout
else if queued_reviewers > 0 and no executable path is available:
  apply local_queue_liveness_timeout_seconds and surface a no-executable-path error
else:
  admit the next eligible reviewer
```

This makes the worst case bounded by the active reviewer's run timeout or user cancellation, instead of a short queue item timeout that later reviewers cannot realistically satisfy.

## Deep Review Adaptation

Deep Review should continue to ask for specialist reviewers, but the runtime should own real execution order.

Prompt and manifest changes should move from:

```text
Launch all reviewer Task calls in one message.
```

to:

```text
Schedule all required reviewers. The runtime may execute them in bounded parallel batches according to model gateway capacity. Preserve reviewer isolation and include every scheduled reviewer in the final report.
```

Execution policy additions:

- `max_parallel_reviewers`: Deep Review-level ceiling scoped to one Deep Review turn. It must not count main-chat subagents, other sessions, or subagents inside other sessions.
- `provider_queue_timeout_seconds`: optional short provider-capacity queue deadline.
- `local_queue_liveness_timeout_seconds`: optional no-executable-path watchdog. It only applies when no Deep Review reviewer is active.
- `retry_budget`: optional retry count for read-only reviewers.
- `gateway_concurrency_policy`: optional pointer to a model or gateway limit profile.

Review report behavior:

- Include queued, retried, timed-out, and failed reviewer states in `reviewers`.
- Preserve partial reviewer output when available.
- Continue to ReviewJudge after all runnable reviewers settle.
- If ReviewJudge cannot run, fall back to conservative parent validation using surviving reviewer evidence.

## UI and Observability

The UI should distinguish between model thinking, queue waiting, and retry backoff.

Suggested visible fields:

- Running subagents: `2 / 4 configured`, plus `effective limit: 2` when throttled.
- Queue: `3 waiting`, with named waiting reviewers and the active reviewer(s) currently blocking the queue.
- Retry: `1 retrying after gateway overload`.
- Per reviewer: queued time, run time, retry count, final status, and final error reason when failed.
- Deep Review summary: "Review continued with queued execution because the model gateway limited concurrency."
- No-executable-path failure: "No Deep Review reviewer is running and no reviewer can acquire an execution path. Check provider settings or retry with a lower reviewer parallelism setting."

Logs should stay English-only and avoid noisy per-token logging. Important Deep Review admission logs:

- Permit acquired/released.
- Effective concurrency changed.
- Retry classified and scheduled.
- Retry budget exhausted.
- Partial output preserved.

## Configuration Shape

Coarse proposal:

```json
{
  "ai": {
    "subagent_max_concurrency": 4,
    "gateway_concurrency": {
      "default": {
        "max_concurrent_requests": 4,
        "adaptive": true,
        "min_concurrent_requests": 1
      },
      "vllm-local": {
        "match": {
          "provider": "openai",
          "base_url_contains": "localhost"
        },
        "max_concurrent_requests": 2,
        "adaptive": true
      }
    },
    "review_teams": {
      "default": {
        "max_parallel_reviewers": 3,
        "reviewer_file_split_threshold": 20,
        "max_same_role_instances": 2,
        "reviewer_timeout_seconds": 600,
        "provider_queue_timeout_seconds": 120,
        "local_queue_liveness_timeout_seconds": 900,
        "reviewer_retry_budget": 1
      }
    }
  }
}
```

Exact names can change during implementation. The important part is the split between:

- Deep Review-internal reviewer policy,
- provider/model request capacity,
- global subagent execution capacity as a deferred platform concern.

## Architecture Boundaries

Core runtime:

- Owns Deep Review reviewer admission state, retry classification, queue liveness, and event emission.
- Keeps Deep Review queue behavior behind Deep Review manifest/context gates.
- May later own model-gateway request permits because streaming request duration is known at the AI client boundary, but that is a separate provider-capacity project.

Web UI:

- Displays queue and retry states.
- Edits user-facing policy fields.
- Does not decide scheduler semantics.
- Does not count main-session or cross-session ordinary subagents as Deep Review reviewer capacity.

Desktop / transport adapters:

- Expose config and events.
- Do not own product scheduling rules.

Provider adapters:

- Surface raw provider diagnostics.
- Mark whether a failure happened before effective output.
- Keep streaming permits until the stream finishes, fails, or is cancelled.

## Implementation Phases

### Phase 0: Clarify existing behavior

- Document `ai.subagent_max_concurrency` in settings and Deep Review docs.
- Clarify that Review Team `max_parallel_reviewers` is scoped to one Deep Review turn.
- Remove or downgrade UI/product language that treats current main-session subagent activity as a hard Deep Review capacity blocker.
- Keep default behavior compatible.

### Phase 1: Deep Review queue liveness

- Split local reviewer queue semantics from provider transient queue semantics.
- Keep queued reviewers waiting while any reviewer in the same Deep Review turn is running.
- Apply no-executable-path timeout only when no reviewer is active and the queue cannot acquire an execution path.
- Emit queue, running, no-executable-path timeout, cancellation, and skip events with stable Deep Review reasons.
- Update UI to show waiting reviewers, active blocking reviewers, elapsed queue time, and actionable failure reasons.

### Phase 2: Provider transient capacity recovery

- Add provider error classification for concurrency and rate-limit failures.
- Add short provider-capacity wait based on `Retry-After` or bounded backoff.
- Attempt one provider re-run for read-only Deep Review reviewers when no useful output was produced.
- Keep this scoped to Deep Review reviewer execution.

### Phase 3: Retry and adaptive concurrency

- Retry read-only Deep Review reviewers after retriable capacity errors.
- Reduce effective concurrency on overload.
- Slowly increase it after successful windows.
- Preserve partial outputs when retries are unsafe or exhausted.

### Phase 4: Review Team policy and report integration

- Add separate Deep Review provider queue timeout, local queue liveness watchdog, retry budget, and max parallel reviewer policy.
- Change Deep Review prompt/manifest rules from "all in one parallel message" to "runtime-bounded scheduling".
- Ensure ReviewJudge receives complete reviewer state, including queued/retried/failed metadata.

### Phase 5: Advanced runtime quality

- Consider per-gateway fairness only after Deep Review queue liveness and provider recovery are stable.
- Add Deep Review queue metrics to diagnostics/export.
- Add persisted recovery for long-running review sessions if the app restarts.
- Consider trust metadata for scheduler-produced summaries and partial reviewer output.

## Risks and Open Questions

- Retrying write-capable subagents is risky unless tool history proves no state-changing action occurred.
- Adaptive concurrency can oscillate if provider errors are noisy; start conservative.
- Queue timeout must not hide real gateway outage. If every retry hits auth/quota/model errors, surface settings/diagnostics instead of looping.
- A no-executable-path timeout can be wrong if active reviewer guards leak or session state is stale. Tests must prove guards release on success, failure, cancellation, provider-capacity retry, and timeout.
- Deep Review output may become slower on local gateways. The UI should frame this as controlled queued execution, not as a degraded or broken review.
- Persisted scheduler state is useful but should not be a Phase 1 requirement unless long-running desktop review recovery is in scope.

## Success Criteria

- A vLLM gateway with max concurrency 2 can complete a Deep Review with 4-5 reviewers by queueing rather than failing the burst.
- Users can see which reviewers are running, waiting, retrying, or completed.
- Queued reviewers do not expire through a short fixed queue timeout while another reviewer in the same Deep Review turn is active.
- Main-chat subagents, other sessions, and other sessions' subagents do not reduce the Deep Review reviewer cap.
- A no-executable-path queue timeout surfaces only when there is no active Deep Review reviewer and no executable path is available.
- Review reports include accurate reviewer status and preserve partial evidence.
- Non-retryable provider errors still surface promptly with raw diagnostics.
- Existing fast cloud-provider workflows still run with high parallelism when configured capacity allows it.

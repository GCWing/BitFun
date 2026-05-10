# Deep Review Strategy Engine & Architecture Reviewer Design

## Overview

This document proposes a set of improvements to BitFun's deep review system to address timeout, rate-limit, and coverage issues when reviewing large code changes with slow or rate-limited models.

The proposal has three parts:
1. **Strategy Engine** (方案2): Programmatic strategy selection, predictive timeouts, dynamic concurrency control, and partial result capture.
2. **Architecture Reviewer**: A new core reviewer role focused on structural/architectural concerns, always-on across all strategy levels.
3. **Frontend Reviewer**: A new core reviewer role focused on frontend-specific concerns (React, i18n, accessibility, state management).

## Implementation Status

### Status Reconciliation

This section reflects the current implementation shape after the latest Deep Review strategy-engine commits. Some Phase 2 work is now present as backend policy helpers, manifest parsing, prompt rules, schema/UI surfaces, or tool-level safety nets, but not all items have reached full deterministic scheduler semantics. In particular, "implemented" below means the code has a usable runtime surface; items that still rely on the DeepReview orchestrator prompt, or that are only renderable when the final report carries a structured signal, are called out separately.

### Completed

- Added `ReviewArchitecture` as an always-on core reviewer role.
- Added `ReviewFrontend` as a frontend-focused reviewer role in the review team model.
- Added dedicated prompts for Architecture and Frontend reviewers.
- Updated DeepReview orchestration prompt to dispatch Architecture and Frontend reviewers and to provide role-specific strategy amplification.
- Updated existing reviewer prompts to reduce overlap:
  - Business Logic no longer owns UI state or layer-boundary analysis.
  - Performance no longer owns React render optimization.
  - Security focuses on exploitable trust-boundary risks, while structural boundaries move to Architecture.
- Updated Judge prompt with cross-reviewer overlap handling for Architecture/Business Logic, Architecture/Security, Frontend/Performance, and Frontend/Business Logic overlaps.
- Updated review team service, tests, and UI metadata for the new roles.
- Updated Settings > Review i18n so Architecture and Frontend reviewer names render in the active language.
- Updated the Agents page Code Review Team card to avoid clipped reviewer tags and present a compact role-summary layout.
- Added the initial predictive-timeout loop: the launch manifest records target file and diff line stats, derives effective reviewer/judge timeouts from strategy and target size, and the core Task tool honors that manifest policy when launching DeepReview subagents.
- Converted Frontend Reviewer dispatch from always-present execution to conditional execution based on changed frontend or frontend-backend contract files.
- Made backend-provided reviewer definitions the runtime source for frontend team resolution and review-agent visibility, with frontend fallback metadata only as a degraded-mode safety net.
- Added a backend `ChangeRiskFactors` structure and `auto_select_strategy()` helper as a pure policy computation. Runtime manifests now record frontend and backend-compatible recommendations, user override, final strategy, mismatch state, and mismatch severity; backend scoring remains advisory/mismatch-warning metadata and does not override the team or user-selected strategy.
- Added `DeepReviewConcurrencyPolicy` parsing and TaskTool local-cap capacity handling. Reviewer launches now bounded-wait for local reviewer capacity, expire as `CapacitySkipped`, lower a turn-local effective cap after local capacity skips, recover cautiously after successful reviewer observations, and fold capacity skips into final report reliability signals; this is not yet a staggered backend scheduler.
- Added adaptive capacity queue foundation primitives, a backend queue-state event contract, a compact frontend queue notice, local/manual queue-control surface, backend-bound queue pause/continue/cancel/optional-skip commands for local-cap and provider-capacity waits, launch-time active-session concurrency warning, and provider transient-capacity observation. Explicit provider rate-limit/concurrency/retry-after/temporary-overload reviewer failures now wait briefly, reattempt once, surface final `capacity_skipped` when needed, lower the turn-local effective cap, and feed report reliability signals; user-facing effective-cap override controls and staggered backend dispatch are not enabled.
- Added retry budget tracking, reviewer timeout retry guidance, TaskTool structured retry admission, bounded retry-scope prompt injection, explicit action-bar retry for structured unresolved slices, and guarded `auto_retry` admission behind Review Team opt-in. The visible retry ceiling uses the effective manifest policy when available; backend-owned automatic redispatch scheduling with reduced scope is not implemented.
- Added partial-timeout result preservation through a coordinator grace period when a timed-out subagent returns a usable final message before the grace window closes.
- Added per-session incremental review cache primitives, session metadata storage fields, TaskTool cache-hit read logic, completed-reviewer write-through, packet-key alignment, and hit/miss reliability signals.
- Added a shared review-subagent tooling contract so custom review agents have explicit required tools (`GetFileDiff`, `Read`) and invalid tooling is reported as `invalid_tooling` rather than silently disappearing.
- Added packet metadata fallback in `submit_code_review`, so missing reviewer `packet_id` values can be inferred from the run manifest when possible and marked as lower-confidence metadata when not.
- Added adaptive context-profile and model-capability policy support for long-running review work and weak-model handling.
- Added a reviewer applicability registry so conditional reviewer decisions are data-driven instead of hardcoded in review-team assembly.
- Added heuristic prompt-byte budget metadata: the launch manifest records per-mode prompt byte thresholds, estimated max reviewer prompt bytes, full-scope summary-first decisions, and token-budget warnings without clipping `assigned_scope` files.
- Added a compact pre-review launch summary in the Deep Review consent dialog, covering file count, risk areas, selected strategy, optional-reviewer count, summary-first state, and skipped-reviewer warnings without restoring dense lineup/cost cards.
- Added automated locale completeness and Agents page team-card layout resilience tests for review-team roles.

### Remaining / Future Work

The detailed execution order and per-round exit checks are tracked in `docs/deep-review-phase2-addendum.md`. Keep this section as the design-level summary, and update the addendum first when implementation status changes.

- **Change risk runtime authority**: Backend-compatible scoring is now represented as advisory/mismatch-warning manifest metadata, while the final launch strategy remains the configured team strategy or explicit user override. `max_cyclomatic_complexity_delta` is still marked `not_measured`; do not make backend scoring authoritative until a measured complexity signal exists and product explicitly wants auto-selection to override user choice.
- **Backend queue/stagger scheduler**: `DeepReviewConcurrencyPolicy` exists and TaskTool bounded-waits for local reviewer-cap saturation before expiring to `CapacitySkipped`; queue state has a backend event contract and can be controlled from the action bar for local-cap, provider-capacity, and launch-batch-overlap waits. Turn-local effective learning exists for local-cap skips and explicit provider transient-capacity reviewer failures, but backend `staggerSeconds`, full batch lifecycle management, and user-facing override controls are not implemented. Any future queue extension must keep using visible controls, stay timeout-separated, and must not silently consume the user's normal session concurrency.
- **Automatic retry dispatch**: Retry budget, guidance, structured retry admission, explicit manual retry action, guarded `auto_retry` admission, and bounded retry-scope prompt injection exist, but the backend does not automatically redispatch a timed-out reviewer with reduced scope or downgraded strategy.
- **Project-level incremental review cache**: Per-session cache read/write support is implemented and keyed by `packet_id`; cross-session/project-level persistence remains product-decision-required and deferred. Current cached reviewer outputs live only with session metadata and are deleted with that session metadata.
- **Shared context cache**: Frontend plan generation, prompt rules, local duplicate `Read`/`GetFileDiff` measurement, and aggregate debug diagnostics exist, but backend result reuse is not programmatically enforced.
- **Token budget enforcement**: File splitting, max-file style limits, heuristic prompt-byte estimates, and full-scope `largeDiffSummaryFirst` decisions are present in manifest policy. Hard prompt-byte clipping and byte-accurate enforcement remain deferred, and any summary-first path must keep unreviewed files visible in coverage notes/reliability signals.
- **Cost-aware review depth**: Quick/default strategies still need a product-level depth contract that makes `quick` a high-risk gate, `normal` a risk-expanded review, and `deep` the explicit full-depth path. This should reduce slow-model time and token use without hiding changed files from coverage metadata.
- **Shared evidence pack**: Duplicate-tool diagnostics can show repeated `Read`/`GetFileDiff` work, but reviewers still rediscover common facts. A source-agnostic evidence pack should precompute changed files, hunk hints, domain/risk tags, packet ids, and cheap contract hints once per run so subagents spend more tokens on judgment than discovery.
- **Pre-review summary UI**: Compact launch-dialog summary is implemented. A separate dense pre-review report remains deferred unless product later needs it.
- **Work packet batched scheduling**: Frontend work packet data structure and prompt rules are complete; extra reviewers are placed into their own launch batch when configured, and TaskTool now prevents later `launchBatch` reviewers from running while an earlier batch is still active by queueing them with `launch_batch_blocked`. Full backend-owned `staggerSeconds` scheduling remains deferred.
- **Conditional reviewer extensibility**: Path-domain classification and reviewer applicability rules now support the current Frontend Reviewer; future conditional reviewer families should extend the registry and add focused tests.
- **Custom reviewer quality boundary**: `GetFileDiff` + `Read` is the minimum valid review-agent tool contract; missing recommended tools such as `Grep`, `Glob`, and `LS` should remain visible as degraded review quality, not invalid configuration.
- **Compression contract integration**: `CompressionContract` structure and `From<EvidenceLedgerSummary>` conversion is complete; the compressor prompt already injects contract content; no additional implementation needed.
- Extend operational metrics beyond report reliability signals if runtime dashboards or telemetry are added later.
- Define retention/privacy boundaries for cached reviewer outputs, partial outputs, and evidence-ledger artifacts.

## Current State Analysis

### Architecture

BitFun's deep review is a **prompt-driven 5-phase orchestrator** (`DeepReview` agent) coordinating 4 always-on specialist reviewers, an optional frontend-focused reviewer, and 1 sequential judge:

```
Phase 1: Scope identification
Phase 2: Parallel dispatch (BusinessLogic, Performance, Security, Architecture + Frontend when applicable + extras)
Phase 3: Quality gate (ReviewJudge validates/merges findings)
Phase 4: Report synthesis (submit_code_review)
Phase 5: Optional remediation (Edit/Write/Bash)
```

Key components:
- **Policy layer**: `deep_review_policy.rs` - execution policy, budget tracker, file splitting
- **Task tool enforcement**: `task_tool.rs` - readonly/review/budget/timeout enforcement
- **Coordinator**: `coordinator.rs` - subagent execution with dynamic timeout adjustment
- **Frontend**: `reviewTeamService.ts` + `DeepReviewService.ts` - manifest building, session management

### Current Strengths

1. Defense-in-depth policy enforcement (programmatic, not just prompt)
2. Clean separation: orchestrator (write) vs reviewers (read-only)
3. Configurable strategy levels with per-member overrides
4. File splitting for large changes (threshold: 20 files, max 3 instances/role)
5. Continuation/recovery for interrupted reviews
6. Budget tracking with TTL-based pruning

### Current Weaknesses

| Dimension | Problem | Impact |
|-----------|---------|--------|
| **Orchestration determinism** | 5-phase workflow exists only in prompt text; LLM may skip phases or serialize reviewers | Weak models increase total runtime 3x |
| **Timeout strategy** | Predictive timeout is present, but partial capture only preserves output that arrives during the grace period | Some timed-out work can still be lost |
| **Dynamic concurrency** | TaskTool bounded-waits for local reviewer-cap saturation, short-waits provider transient capacity, reattempts once, learns a turn-local effective cap after capacity skips, and queues later launch batches while earlier batches are active. There is still no full backend batch/stagger scheduler | Weak orchestrator models can still skip an unstarted earlier batch; broader automatic waiting would be confusing unless backend-bound controls are visible |
| **Error fallback** | Retry budget, guidance, structured retry admission, explicit manual retry action, guarded `auto_retry` admission, and bounded retry-scope prompt injection exist, but backend-owned automatic reduced-scope redispatch is not implemented | Retry launch behavior still depends on the orchestrator model or user action |
| **Context management** | Shared context cache is prompt-only with local duplicate Read/GetFileDiff measurement and aggregate debug diagnostics; backend result reuse is not enforced | Reviewers may duplicate IO and token usage until real-run measurements justify an interception/cache plan |
| **Strategy selection** | Frontend recommendation, backend-compatible recommendation, user override, final strategy, mismatch state, and mismatch severity are recorded as launch metadata; runtime launch still follows configured/user-selected strategy | Users may still over- or under-review, but the product now has non-blocking metadata to explain the tradeoff without silently changing token/concurrency cost |
| **Review cost by strategy** | Quick and normal modes have budget metadata, but role prompts can still perform broad discovery unless a scope-depth contract is explicit | Slow models and large diffs can consume excessive time/tokens before reaching high-risk findings |
| **Repeated evidence discovery** | Duplicate `Read`/`GetFileDiff` calls are measured, but reviewers do not yet receive a shared evidence pack with hunk/risk/contract hints | Parallel subagents can spend their first turns reading the same files and git facts instead of reasoning |

### Scenario Breakdown

| Scenario | Files | Lines | Current Runtime Behavior | Remaining Concern |
|----------|-------|-------|------------------|---------|
| A: Small change | < 5 | < 200 | 4 always-on reviewers, optional frontend only when applicable | Can still be over-provisioned if the user chooses a deeper strategy |
| B: Medium change | 5-20 | 200-1000 | 4 always-on reviewers with predictive timeout and local-cap backpressure | Logic-heavy reviewers may still return partial output on slow models |
| C: Large change | 20-50 | 1000+ | File split can create multiple reviewer packets plus judge; local reviewer-cap waiting is bounded | Cost-aware high-risk-first scope and shared evidence packs are still needed before adding heavier scheduler behavior |
| D: Any + slow model | Any | Any | Predictive timeout, partial capture, and structured retry admission exist | Backend-owned retry redispatch is still prompt-guided/deferred |
| E: Any + rate limit | Any | Any | Local cap pressure is bounded and visible; explicit provider transient-capacity reviewer failures short-wait, retry once, then become `capacity_skipped` if still constrained and lower the turn-local effective cap | Backend batch scheduling and broader provider/adaptive queueing are not implemented |

## Competitive Landscape

| Tool | Architecture | Parallelism | Large Change | Adaptive | Budget |
|------|-------------|-------------|--------------|----------|--------|
| **BitFun (current)** | 4 always-on specialists + optional Frontend + judge | Prompt-guided parallelism with local-cap backpressure | File split (threshold 20) + summary-first metadata | Advisory strategy metadata; provider/adaptive queue deferred | Per-session cache + heuristic prompt-byte metadata |
| GitHub Copilot | Single model, single pass | N/A | Skip if > ~3000 lines | None | Service-managed |
| CodeRabbit | Single model, multi-pass | Sequential passes | Chunk + summary-first | Heuristic (PR size) | Implicit (chunking) |
| Amazon CodeGuru | Detector ensemble | Parallel detectors | File-level + incremental | Static (language) | Per-detector |
| Google AutoCommenter | Single model, chunked | Sequential chunks | Chunk by file | Confidence threshold | Per-chunk |

**Key insights from research:**
- No major AI tool has dedicated architecture reviewer as separate agent
- Research (Hong et al., 2024) suggests 3-5 agents with non-overlapping scopes is optimal
- Risk-based automatic strategy selection can reduce review time ~30% (Munaiah et al., 2017)
- Budget-aware dynamic reallocation improves efficiency (Wang et al., 2024)

## Part 1: Strategy Engine (方案2)

### 1.1 Change Risk Auto-Classification

**Goal**: Automatically recommend strategy level based on change characteristics.

**Implementation**:

Add a new method to `DeepReviewExecutionPolicy`:

```rust
/// Risk factors used for automatic strategy selection
pub struct ChangeRiskFactors {
    pub file_count: usize,
    pub total_lines_changed: usize,
    pub files_in_security_paths: usize,  // e.g. auth/, crypto/, api/
    pub max_cyclomatic_complexity_delta: usize,
    pub cross_crate_changes: usize,  // files in different crates
}

impl DeepReviewExecutionPolicy {
    /// Auto-select strategy level based on change risk.
    /// Returns recommended level and a human-readable rationale.
    pub fn auto_select_strategy(&self, risk: &ChangeRiskFactors) -> (DeepReviewStrategyLevel, String) {
        let score = risk.file_count
            + risk.total_lines_changed / 100
            + risk.files_in_security_paths * 3
            + risk.cross_crate_changes * 2;

        match score {
            0..=5 => (DeepReviewStrategyLevel::Quick,
                      format!("Small change ({} files, {} lines). Quick scan sufficient.",
                              risk.file_count, risk.total_lines_changed)),
            6..=20 => (DeepReviewStrategyLevel::Normal,
                       format!("Medium change ({} files, {} lines). Standard review recommended.",
                               risk.file_count, risk.total_lines_changed)),
            _ => (DeepReviewStrategyLevel::Deep,
                  format!("Large/high-risk change ({} files, {} lines, {} security files). Deep review recommended.",
                          risk.file_count, risk.total_lines_changed, risk.files_in_security_paths)),
        }
    }
}
```

**Risk factor computation**:
- `file_count` and `total_lines_changed`: from `GetFileDiff` or `Git diff --stat`
- `files_in_security_paths`: configurable list of path patterns (e.g. `**/auth/**`, `**/crypto/**`)
- `max_cyclomatic_complexity_delta`: computed by a lightweight AST pass or heuristic
- `cross_crate_changes`: count files across different `Cargo.toml` workspaces

**Frontend integration**:
- `reviewTeamService.ts` computes risk factors before building the manifest
- UI shows recommended strategy with rationale; user can override
- Override is persisted per-project

### 1.2 Predictive Timeout

**Goal**: Set per-reviewer timeout based on change size and strategy, not static defaults.

**Current state**:
```rust
const DEFAULT_REVIEWER_TIMEOUT_SECONDS: u64 = 600;  // 10 minutes
const DEFAULT_JUDGE_TIMEOUT_SECONDS: u64 = 600;     // 10 minutes
```

**Proposed state**:
```rust
/// Base timeout per strategy (seconds)
const BASE_TIMEOUT_QUICK: u64 = 180;
const BASE_TIMEOUT_NORMAL: u64 = 300;
const BASE_TIMEOUT_DEEP: u64 = 600;

/// Per-file overhead (seconds)
const TIMEOUT_PER_FILE: u64 = 15;
const TIMEOUT_PER_100_LINES: u64 = 30;

impl DeepReviewExecutionPolicy {
    pub fn predictive_timeout(
        &self,
        role: DeepReviewSubagentRole,
        strategy: DeepReviewStrategyLevel,
        file_count: usize,
        line_count: usize,
    ) -> u64 {
        let base = match strategy {
            DeepReviewStrategyLevel::Quick => BASE_TIMEOUT_QUICK,
            DeepReviewStrategyLevel::Normal => BASE_TIMEOUT_NORMAL,
            DeepReviewStrategyLevel::Deep => BASE_TIMEOUT_DEEP,
        };

        let file_overhead = file_count as u64 * TIMEOUT_PER_FILE;
        let line_overhead = (line_count as u64 / 100) * TIMEOUT_PER_100_LINES;

        let raw = base + file_overhead + line_overhead;

        // Judge needs more time when there are more reviewer reports
        let judge_multiplier = match role {
            DeepReviewSubagentRole::Judge => {
                let reviewer_count = CORE_REVIEWER_AGENT_TYPES.len() + self.extra_subagent_ids.len();
                1 + (reviewer_count as u64 - 1) / 3  // +1 for every 3 reviewers
            }
            DeepReviewSubagentRole::Reviewer => 1,
        };

        let predicted = raw * judge_multiplier;
        predicted.min(MAX_TIMEOUT_SECONDS)
    }
}
```

**Example predictions**:

| Change | Strategy | Files | Lines | Reviewer Timeout | Judge Timeout |
|--------|----------|-------|-------|-----------------|---------------|
| 3 files, 150 lines | Quick | 3 | 150 | 180 + 45 + 30 = 255s | 255s |
| 15 files, 800 lines | Normal | 15 | 800 | 300 + 225 + 240 = 765s -> 600s (capped) | 600s |
| 30 files, 2000 lines | Deep | 30 | 2000 | 600 + 450 + 600 = 1650s -> 1200s (capped) | 1200s x 1 = 1200s |
| 30 files, 2000 lines + 2 extras | Deep | 30 | 2000 | 1650s -> 1200s | 1650s x 2 = 2400s -> 1800s (capped) |

**Integration**:
- Frontend computes `file_count` and `line_count` from diff before building manifest
- Passes them in the prompt block or as config fields
- `effective_timeout_seconds` is updated to call `predictive_timeout` when risk factors are available

### 1.3 Dynamic Concurrency Control

**Goal**: Prevent rate limit violations by controlling how many reviewers launch in parallel.

**Original problem state**: Reviewers were launched by prompt instruction with true parallelism. With core roles, file splitting, optional Frontend, and extras, this can create many parallel LLM calls.

**Current runtime boundary**: TaskTool now bounded-waits for local reviewer-cap saturation, emits queue state, can expire over-cap reviewer work as `CapacitySkipped`, short-waits explicit provider transient-capacity reviewer failures before one reattempt and final `capacity_skipped` with turn-local effective-cap learning, and queues later launch batches while earlier batches remain active. It still does not own deterministic backend stagger scheduling, complete batch DAG dispatch, or a broader provider/adaptive queue scheduler.

**Proposed state**:

Add `DeepReviewConcurrencyPolicy`:

```rust
pub struct DeepReviewConcurrencyPolicy {
    /// Maximum parallel reviewer instances at once
    pub max_parallel_instances: usize,
    /// Whether to stagger launches (wait N seconds between batches)
    pub stagger_seconds: u64,
    /// Whether to batch extras separately from core reviewers
    pub batch_extras_separately: bool,
}

impl Default for DeepReviewConcurrencyPolicy {
    fn default() -> Self {
        Self {
            max_parallel_instances: 4,
            stagger_seconds: 0,
            batch_extras_separately: true,
        }
    }
}
```

**Launch strategy**:

```
Batch 1 (immediate): Core 4 reviewers (BL, Perf, Sec, Arch)
  - If file splitting: up to 3 instances per role, but total <= max_parallel_instances
  - Example: max_parallel_instances=4, 4 core roles -> 1 instance each (no splitting)
  - Example: max_parallel_instances=8, 4 core roles -> 2 instances each

Wait for Batch 1 to complete or timeout

Batch 2 (if needed): Conditional Frontend reviewer and extra reviewers
  - Only if Frontend is applicable or extras are configured, and Batch 1 completed
  - Respects max_parallel_instances
```

**Rate limit awareness**:

The frontend can query the current model's rate limit status (from a lightweight endpoint or cached state) and adjust `max_parallel_instances`:

```typescript
function computeConcurrencyPolicy(
  modelSlot: string,
  rateLimitStatus: RateLimitStatus | null,
): DeepReviewConcurrencyPolicy {
  const baseMax = 4;
  if (!rateLimitStatus || rateLimitStatus.remaining > baseMax * 2) {
    return { max_parallel_instances: baseMax, stagger_seconds: 0, batch_extras_separately: true };
  }
  if (rateLimitStatus.remaining > baseMax) {
    return { max_parallel_instances: baseMax, stagger_seconds: 5, batch_extras_separately: true };
  }
  // Rate limit is tight: reduce parallelism and add stagger
  return {
    max_parallel_instances: Math.max(2, rateLimitStatus.remaining),
    stagger_seconds: 10,
    batch_extras_separately: true,
  };
}
```

**Integration with file splitting**:

When `max_parallel_instances` is tight, file splitting should be reduced or disabled:

```rust
pub fn effective_max_same_role_instances(
    &self,
    file_count: usize,
    concurrency_policy: &DeepReviewConcurrencyPolicy,
) -> usize {
    let role_count = CORE_REVIEWER_AGENT_TYPES.len() + self.extra_subagent_ids.len();
    let max_per_role = concurrency_policy.max_parallel_instances / role_count;
    max_per_role.max(1).min(self.max_same_role_instances)
}
```

**Adaptive capacity queue follow-up**:

The current runtime intentionally stops at bounded local-cap waiting plus turn-local effective-cap learning for local-cap skips and explicit provider transient-capacity failures. Broader provider/adaptive scheduling is a future extension outside the current runtime boundary. If added, it must be owned by the subagent runtime rather than by the DeepReview prompt alone:

- Treat configured `max_parallel_instances` as a hard maximum and maintain a lower runtime `effective_parallel_instances` when provider or local capacity errors are observed.
- Queue only explicit transient capacity errors: provider rate limit, provider concurrency limit, explicit `Retry-After`, local subagent cap saturation, or temporary overload. Authentication, billing/quota exhaustion, invalid model, policy violation, user cancellation, invalid tooling, and validation errors must fail fast.
- Separate queue time from execution timeout. A reviewer in `QueuedForCapacity` or `PausedByUser` has not started its reviewer `timeout_seconds`; the timeout starts only after the reviewer enters `Running`.
- Surface the queue as a compact user-facing state, not as hidden waiting. Local-cap backend-driven queue notices now support backend-bound pause, continue, cancel, and optional-extra skipping. Cap adjustment and broader provider/adaptive queue controls remain future work.
- Preserve normal session responsiveness. Deep Review reviewer queueing must not silently consume all available subagent capacity. For broader provider/adaptive queueing, the UI should recommend pausing Deep Review or lowering strategy, and provide backend-bound controls before promising manual continuation.

### 1.4 Partial Result Capture

**Goal**: When a reviewer times out, preserve its last output instead of losing all work.

**Current state**: Coordinator uses `tokio::time::timeout` which returns `Err(Timeout)` with no partial data.

**Proposed state**:

Modify the coordinator's `execute_subagent` to capture the last model response before timeout:

```rust
// In coordinator.rs, around the timeout wrapping logic:
let result = if let Some(secs) = timeout_seconds.filter(|&s| s > 0) {
    let timeout_future = tokio::time::timeout(
        Duration::from_secs(secs),
        self.run_subagent_loop(...)
    );
    match timeout_future.await {
        Ok(result) => result,
        Err(_) => {
            // Timeout fired - try to capture partial results
            let partial = self.try_capture_partial_results(&session_id).await;
            match partial {
                Some(partial_result) => {
                    // Return partial result with a timeout marker
                    Ok(SubagentResult {
                        response: format!("{}", partial_result),
                        status: SubagentStatus::PartialTimeout,
                        ..partial_result
                    })
                }
                None => Err(BitFunError::Timeout(timeout_error_message)),
            }
        }
    }
} else {
    self.run_subagent_loop(...).await
};
```

**Partial result capture mechanism**:

The subagent's dialog turns are stored in the session store. `try_capture_partial_results` would:
1. Read the subagent session's dialog turns
2. Find the last assistant message (model output)
3. Extract any findings already written in the expected format
4. Return them as a partial result

**Frontend handling**:

`deepReviewContinuation.ts` already detects `timed_out` status. It should be updated to handle `PartialTimeout`:

```typescript
// In collectReviewerProgress:
if (toolResult.status === 'partial_timeout' ||
    (toolResult.error && /partial timeout/i.test(toolResult.error))) {
  status = 'partial_timeout';
}

// In buildDeepReviewContinuationPrompt:
// Include partial findings with reduced confidence
```

**Judge handling**:

The judge prompt already instructs it to handle partial results. The `ReviewJudge` should treat `partial_timeout` findings with lower confidence than `completed` findings.

### 1.5 Retry Budget

**Goal**: Allow each reviewer role one retry with reduced scope when it times out or fails.

**Current state**: `DeepReviewTurnBudget` tracks:
- `reviewer_calls`: max calls per turn
- `judge_calls`: max 1 per turn

**Proposed state**:

Add retry tracking:

```rust
#[derive(Debug, Clone)]
struct DeepReviewTurnBudget {
    reviewer_calls: usize,
    judge_calls: usize,
    retries_used: HashMap<String, usize>,  // role -> retry count
    max_retries_per_role: usize,            // default: 1
    updated_at: Instant,
}
```

**Retry logic**:

When a reviewer times out or fails:
1. Orchestrator checks `retries_used[role] < max_retries_per_role`
2. If yes, re-dispatch with:
   - Same target but reduced scope (only files not yet reviewed)
   - Reduced timeout (original timeout / 2)
   - Strategy downgraded one level (deep -> normal, normal -> quick)
3. Include structured `retry_coverage` so TaskTool can confirm the source packet, source status, covered files, and smaller retry scope
4. Increment `retries_used[role]`

**Current runtime boundary**: TaskTool enforces the structured retry admission gate for retry reviewer Tasks. It rejects missing coverage, non-retryable source status, broad scope, non-lowered timeout, and exhausted retry budget, then prepends the accepted retry scope to the reviewer prompt. It does not infer coverage from free-form partial output and does not launch backend-owned automatic redispatch.

**Integration with continuation**:

The continuation system already handles re-running failed reviewers. The retry budget adds a cap to prevent infinite loops.

### 1.6 Summary: Strategy Engine Changes

| Component | File | Change |
|-----------|------|--------|
| Risk classification | `deep_review_policy.rs` | Add `ChangeRiskFactors` and `auto_select_strategy` |
| Predictive timeout | `deep_review_policy.rs` | Add `predictive_timeout` method |
| Concurrency policy | `deep_review_policy.rs` | Add `DeepReviewConcurrencyPolicy` and `effective_max_same_role_instances` |
| Budget retry tracking | `deep_review_policy.rs` | Add `retries_used` and `max_retries_per_role` to `DeepReviewTurnBudget` |
| Structured retry admission | `task_tool.rs` | Require bounded `retry_coverage`, reduced retry scope, retryable source status, lower timeout, retry budget, and bounded retry-scope prompt injection before accepting retry reviewer Tasks |
| Partial result capture | `coordinator.rs` | Add `try_capture_partial_results` and `SubagentStatus::PartialTimeout` |
| Task tool enforcement | `task_tool.rs` | Apply concurrency policy and predictive timeout |
| Frontend risk computation | `reviewTeamService.ts` | Compute risk factors from diff |
| Frontend concurrency | `reviewTeamService.ts` | Build batches based on concurrency policy |
| Frontend timeout pass-through | `DeepReviewService.ts` | Pass risk factors to backend |

## Part 2: Architecture Reviewer

### 2.1 Rationale

**Original gap analysis**: The earlier reviewer set covered correctness, performance, and security. The implemented team now adds Architecture as an always-on reviewer, so this section is historical rationale for why the role was added:

1. **Module coupling / dependency direction violations** - e.g. core crate importing from desktop app
2. **Layer violations** - e.g. service layer bypassing API abstraction
3. **API contract design** - e.g. Tauri commands not following `snake_case` + structured request pattern
4. **Abstraction integrity** - e.g. platform-specific details escaping through shared interfaces
5. **Design pattern consistency** - e.g. new features not following established patterns
6. **Structural scalability** - e.g. changes requiring cross-cutting modifications in 5+ crates

**Research findings**:
- No major AI code review tool has a dedicated architecture reviewer as a separate parallel agent
- Architecture concerns are typically folded into "maintainability" or "code quality" within a single reviewer
- Research (Hong et al., 2024) suggests 3-5 agents with non-overlapping scopes is optimal; 5+1 is still within the efficient range
- Google's code review culture treats "Design" as the most important dimension, but handled by the same reviewer

**Overlap risk with Business Logic reviewer**:
- BL reviewer: "Does this call chain produce correct results?"
- Architecture reviewer: "Should this call chain exist at all? Does it respect layer boundaries?"
- The deep strategy already asks BL to "map full call chains" - this borders on architectural analysis but from a correctness angle

### 2.2 Scope Definition

**Architecture Reviewer** (`ReviewArchitecture`):

**Covers**:
- Module boundary violations (imports that violate layer dependencies)
- API contract design (Tauri commands, tool schemas, transport messages)
- Abstraction integrity (platform-agnostic violations, bypassed interfaces)
- Structural consistency (patterns, registration conventions)
- Dependency direction (circular dependencies, wrong-direction imports)
- Cross-cutting concern impact (changes touching too many layers)

**Explicitly excludes** (to avoid overlap):
- Business rule correctness - Business Logic reviewer
- Algorithm performance - Performance reviewer
- Security vulnerabilities - Security reviewer
- Code style/formatting - not a review dimension

### 2.3 Activation Strategy: Always-On Across All Strategy Levels

**Revised recommendation: Architecture Reviewer should be always-on, not deep-only.**

Previous analysis recommended deep-only activation to minimize cost. However, further investigation reveals:

**Why architecture review matters at every strategy level**:

1. **Quick reviews still need architecture checks**: A 3-file change that adds `import { invoke } from '@tauri-apps/api'` directly in a React component violates the adapter pattern regardless of strategy level. Quick reviews that skip architecture miss exactly the kind of issue that is cheap to find but expensive to fix later.

2. **Architecture violations are cheap to detect**: Unlike business logic review (which requires reading surrounding context), architecture review primarily uses `LS`, `Glob`, and `Grep` for import analysis. The token cost is significantly lower than other reviewers - estimated at 0.6-0.8x of a typical reviewer's cost.

3. **Layer violations compound**: An architecture violation that slips through a quick review will be harder to catch later. The cost of missing it early is disproportionately high.

4. **BitFun's explicit architectural rules**: The project has documented rules ("keep product logic platform-agnostic", "do not call Tauri APIs directly from UI components") that should be checked on every review, not just deep ones.

**Cost mitigation for always-on**:

| Strategy | Architecture Reviewer Behavior | Estimated Token Cost |
|----------|-------------------------------|---------------------|
| Quick | Only check imports directly changed by the diff | ~0.3x |
| Normal | Check diff imports + one level of dependency direction | ~0.6x |
| Deep | Map full dependency graph for changed modules | ~0.8x |

At quick strategy, the architecture reviewer's cost is minimal because it only inspects import statements in changed files - no context reading, no call chain tracing.

**Updated team composition**:

| Strategy | Reviewers | Total Parallel Calls (no split) |
|----------|-----------|-------------------------------|
| Quick | BL, Perf, Sec, **Arch** | 4 |
| Normal | BL, Perf, Sec, **Arch** | 4 |
| Deep | BL, Perf, Sec, **Arch**, **Frontend** (if frontend files present) | 4-5 |

### 2.4 Implementation

**New files**:
- `src/crates/core/src/agentic/agents/prompts/review_architecture_agent.md`

**Modified files**:

1. **`deep_review_policy.rs`**:
   - Add `REVIEWER_ARCHITECTURE_AGENT_TYPE: &str = "ReviewArchitecture"`
   - Update `CORE_REVIEWER_AGENT_TYPES` from `[&str; 3]` to `[&str; 4]`
   - Budget test updates

2. **`review_specialist_agents.rs`**:
   - Add `ArchitectureReviewerAgent` using `define_readonly_subagent!`

3. **`registry.rs`**:
   - Add `ReviewArchitecture` to `default_model_id_for_builtin_agent` - `"fast"`
   - Add to `is_review_agent_entry` check

4. **`deep_review_agent.md` (orchestrator prompt)**:
   - Update "Team Shape" to include Architecture Reviewer as 4th mandatory role
   - Add role-specific strategy amplification for Architecture

5. **`reviewTeamService.ts` (frontend)**:
   - Add `'architecture'` to `ReviewTeamCoreRoleKey`
   - Add `'ReviewArchitecture'` to `ReviewRoleDirectiveKey`
   - Add architecture entry to `DEFAULT_REVIEW_TEAM_CORE_ROLES`
   - Add strategy directives for architecture in all three profiles
   - Update `buildReviewTeamPromptBlock`

6. **Localization**:
   - Add architecture reviewer strings in en-US and zh-CN

7. **Tests**:
   - `deep_review_policy.rs` tests: Update budget calculation assertions
   - `reviewTeamService.test.ts`: Update manifest building tests

### 2.5 Cost Impact (Always-On)

| Metric | Current (3+1) | With Architecture (4+1, always-on) | Delta |
|--------|---------------|-----------------------------------|-------|
| Parallel calls (no split) | 3 | 4 | +33% |
| Parallel calls (3 instances/role) | 9 | 12 | +33% |
| Token cost (quick) | 0.4-0.6x | 0.5-0.8x | +~0.2x |
| Token cost (normal) | 1x | 1.2-1.3x | +~0.25x |
| Token cost (deep) | 1.8-2.5x | 2.2-3.0x | +~0.4x |

**Mitigation**: Architecture reviewer uses `fast` model slot at all strategy levels, and primarily uses `LS`/`Glob`/`Grep` (cheaper than `Read`), so actual cost increase is lower than the raw +33% parallel call increase.

## Part 3: Frontend Reviewer

### 3.1 Rationale

**Why a dedicated Frontend Reviewer is needed**:

The BitFun frontend is a substantial portion of the codebase (~250+ TSX components, ~300+ TS files, 96 locale files) with domain-specific concerns that no current reviewer can effectively evaluate:

| Concern | Current Coverage | Gap |
|---------|-----------------|-----|
| **i18n key synchronization** | None | 96 locale files across 3 languages; missing keys in one locale is a common failure |
| **React performance patterns** | Performance reviewer mentions "expensive renders" generically | Cannot identify React-specific anti-patterns: missing memo/useCallback/useMemo, inline functions in JSX, missing virtualization |
| **Accessibility** | None | Only ~40 aria/role attributes across 250+ components - severely under-covered |
| **Zustand state management** | BL reviewer might catch circular deps as "logic issue" | Cannot recognize Zustand-specific patterns: selector granularity, store dependencies, stale closures |
| **Platform boundary (frontend)** | None | ~6 files import `@tauri-apps/api` directly instead of through adapter layer |
| **Event bus contract alignment** | None | Backend events and frontend listeners must stay in sync; contract drift is invisible |
| **CSS/theme consistency** | None | ThemeService, Monaco theme sync, component library usage patterns |

**Concrete examples that fall through the cracks**:

1. A developer adds `t('scenes.agents.newFeature')` but only adds the key to `en-US/scenes/agents.json`, forgetting `zh-CN` and `zh-TW`. No current reviewer catches this.

2. A developer creates a large list component without virtualization, or defines inline object/function references in JSX causing re-renders. The Performance Reviewer mentions "unnecessary re-renders" but lacks React-specific knowledge.

3. A new modal dialog is added without `aria-labelledby`, focus trap, or keyboard navigation. No reviewer has accessibility in its mission.

4. A Rust backend Tauri command changes its request/response types, but the corresponding TypeScript API client is not updated. No reviewer systematically checks frontend-backend API contract alignment.

### 3.2 Scope Definition

**Frontend Reviewer** (`ReviewFrontend`):

**Covers**:
- i18n completeness and key synchronization across locales
- React performance patterns (memoization, virtualization, effect dependencies)
- Accessibility (ARIA attributes, keyboard navigation, focus management)
- State management patterns (Zustand selector granularity, store dependencies)
- Frontend-backend API contract alignment (Tauri command types, event payloads)
- Platform boundary compliance (no direct `@tauri-apps/api` outside adapter layer)
- CSS/theme consistency (ThemeService usage, component library patterns)

**Explicitly excludes** (to avoid overlap):
- Business rule correctness - Business Logic reviewer
- Algorithm performance (non-React) - Performance reviewer
- Security vulnerabilities - Security reviewer
- Architecture (backend layer violations) - Architecture reviewer

### 3.3 Activation Strategy: Conditional on Frontend File Presence

The Frontend Reviewer should only activate when the change includes frontend files. This avoids wasting resources on pure-backend changes.

**Detection logic** (in the orchestrator or frontend manifest builder):

```typescript
function hasFrontendFiles(changedFiles: string[]): boolean {
  return changedFiles.some(f =>
    f.startsWith('src/web-ui/') ||
    f.startsWith('src/mobile-web/') ||
    f.endsWith('.tsx') ||
    f.endsWith('.scss') ||
    f.endsWith('.css') ||
    f.includes('/locales/')
  );
}
```

**Updated team composition**:

| Strategy | Backend-only change | Change with frontend files |
|----------|--------------------|---------------------------|
| Quick | BL, Perf, Sec, Arch | BL, Perf, Sec, Arch, **Frontend** |
| Normal | BL, Perf, Sec, Arch | BL, Perf, Sec, Arch, **Frontend** |
| Deep | BL, Perf, Sec, Arch | BL, Perf, Sec, Arch, **Frontend** |

### 3.4 Implementation

**New files**:
- `src/crates/core/src/agentic/agents/prompts/review_frontend_agent.md`

**Modified files**:

1. **`deep_review_policy.rs`**:
   - Add `REVIEWER_FRONTEND_AGENT_TYPE: &str = "ReviewFrontend"`
   - Add `FRONTEND_REVIEWER_AGENT_TYPE` to a new `CONDITIONAL_REVIEWER_AGENT_TYPES` array
   - Update budget calculation to account for conditional reviewers

2. **`review_specialist_agents.rs`**:
   - Add `FrontendReviewerAgent` using `define_readonly_subagent!`

3. **`registry.rs`**:
   - Add `ReviewFrontend` to `default_model_id_for_builtin_agent` - `"fast"`
   - Add to `is_review_agent_entry` check

4. **`deep_review_agent.md` (orchestrator prompt)**:
   - Add Frontend Reviewer as conditional role: "If the change includes frontend files (src/web-ui/, .tsx, .scss, locales/), also launch ReviewFrontend in the same parallel batch."

5. **`reviewTeamService.ts` (frontend)**:
   - Add `'frontend'` to `ReviewTeamCoreRoleKey`
   - Add `'ReviewFrontend'` to `ReviewRoleDirectiveKey`
   - Add frontend entry to `DEFAULT_REVIEW_TEAM_CORE_ROLES` with `conditional: true` flag
   - Add strategy directives for frontend in all three profiles
   - Detect frontend files in the change and conditionally include Frontend Reviewer in manifest

6. **Localization**:
   - Add frontend reviewer strings in en-US and zh-CN

7. **Tests**:
   - `deep_review_policy.rs` tests: Update budget calculation for conditional reviewers
   - `reviewTeamService.test.ts`: Test conditional inclusion logic

### 3.5 Cost Impact (Conditional)

| Metric | Backend-only change | Change with frontend files |
|--------|--------------------|---------------------------|
| Parallel calls (no split) | 4 (same as architecture-only) | 5 |
| Token cost (quick) | 0.5-0.8x | 0.6-1.0x |
| Token cost (normal) | 1.2-1.3x | 1.4-1.6x |
| Token cost (deep) | 2.2-3.0x | 2.6-3.5x |

**Mitigation**: Frontend Reviewer uses `fast` model slot at all strategy levels. At quick strategy, it primarily checks i18n keys and import patterns (cheap). At deep strategy, it does thorough React analysis (more expensive but still bounded).

## Part 4: Changes to Existing Reviewer Prompts

Adding Architecture and Frontend reviewers requires adjustments to existing reviewer prompts to eliminate overlap and clarify boundaries.

### 4.1 Business Logic Reviewer Changes

**Current mission items to modify**:

| Current Item | Issue | Change |
|---|---|---|
| "partial updates that can leave data or UI in an inconsistent state" | "UI" overlaps with Frontend Reviewer's state management scope | Change to "partial updates that can leave data in an inconsistent state" - remove "or UI" since Frontend Reviewer covers React state consistency |
| Deep strategy: "map the full call chain for each changed function" | Overlaps with Architecture Reviewer's dependency analysis | Change to "map the full call chain for each changed function to verify business rules and state transitions" - explicitly scope to correctness, not structural analysis |

**New exclusion note** (add to Review Standards section):

```markdown
## What you do NOT review

- Whether a call chain should exist or respects layer boundaries (Architecture Reviewer)
- React component state, i18n, or accessibility issues (Frontend Reviewer)
- Performance of specific algorithms (Performance Reviewer)
- Security vulnerabilities (Security Reviewer)
```

### 4.2 Performance Reviewer Changes

**Current mission items to modify**:

| Current Item | Issue | Change |
|---|---|---|
| "expensive renders or recomputations" | Overlaps with Frontend Reviewer's React performance scope | Change to "expensive computations on hot paths" - remove "renders" since Frontend Reviewer covers React rendering performance |
| "unnecessary re-renders" (in quick strategy efficiency rules) | Overlaps with Frontend Reviewer | Remove from Performance Reviewer; Frontend Reviewer handles React-specific render optimization |
| "oversized diffs / payloads / serialization" | Partially overlaps with Architecture Reviewer's API contract scope | Keep but clarify: "oversized payloads or serialization on data paths" - focus on runtime cost, not contract design |

**New exclusion note**:

```markdown
## What you do NOT review

- React rendering performance or component memoization (Frontend Reviewer)
- Whether a data path respects layer boundaries (Architecture Reviewer)
- Security vulnerabilities (Security Reviewer)
- Business rule correctness (Business Logic Reviewer)
```

### 4.3 Security Reviewer Changes

**Current mission items to modify**:

| Current Item | Issue | Change |
|---|---|---|
| "trust-boundary violations" | Overlaps with Architecture Reviewer's layer boundary checks | Clarify scope: "trust-boundary violations that create exploitable security risks" - Architecture Reviewer checks structural boundaries; Security Reviewer checks exploitable ones |
| "insecure defaults" | Partially overlaps with Architecture Reviewer's API contract checks | Keep but clarify: "insecure defaults in authentication, authorization, or data handling" - scope to security-relevant defaults |

**New exclusion note**:

```markdown
## What you do NOT review

- Structural layer violations without exploitable security impact (Architecture Reviewer)
- Frontend-specific security concerns like XSS in React components (Frontend Reviewer)
- Business rule correctness (Business Logic Reviewer)
- Algorithm performance (Performance Reviewer)
```

### 4.4 Judge Changes

The Judge prompt needs to be updated to handle 4-5 reviewer reports instead of 3:

**Current behavior**: Judge validates findings from 3 core reviewers.

**Required changes**:

1. **Update efficiency rules**: The judge already has strategy-aware efficiency rules. Update the deep strategy directive to explicitly mention cross-validation between Architecture and Business Logic findings (since these roles have the most potential overlap).

2. **Add overlap detection guidance**:

```markdown
## Overlap detection

When multiple reviewers report findings about the same code location:
- If Architecture Reviewer flags a layer violation and Security Reviewer flags a trust-boundary issue at the same location, keep both but note the architectural root cause may address both.
- If Business Logic Reviewer flags a call chain issue and Architecture Reviewer flags the same chain as a dependency violation, the Architecture finding is the root cause; downgrade the BL finding to a symptom.
- If Frontend Reviewer flags a React performance issue and Performance Reviewer flags a general performance issue at the same component, merge into a single finding with both perspectives.
```

3. **Update partial timeout handling**: With more reviewers, the probability of partial timeouts increases. The judge should be instructed to handle partial results from 4-5 reviewers gracefully.

### 4.5 Orchestrator Prompt Changes

**Current "Team Shape" section**:

```
Team shape (mandatory):
- Business Logic Reviewer (ReviewBusinessLogic)
- Performance Reviewer (ReviewPerformance)
- Security Reviewer (ReviewSecurity)
- Review Quality Inspector (ReviewJudge)
```

**Updated "Team Shape" section**:

```
Team shape (mandatory):
- Business Logic Reviewer (ReviewBusinessLogic)
- Performance Reviewer (ReviewPerformance)
- Security Reviewer (ReviewSecurity)
- Architecture Reviewer (ReviewArchitecture)
- [Conditional] Frontend Reviewer (ReviewFrontend) - include only when the change contains frontend files (src/web-ui/, .tsx, .scss, locales/)
- Review Quality Inspector (ReviewJudge)
```

**Updated role-specific strategy amplification** - add entries for Architecture and Frontend:

```
- **ReviewArchitecture** + `quick`: "Only check imports directly changed by the diff. Flag violations of documented layer boundaries."
- **ReviewArchitecture** + `normal`: "Check the diff's imports plus one level of dependency direction. Verify API contract consistency."
- **ReviewArchitecture** + `deep`: "Map the full dependency graph for changed modules. Check for structural anti-patterns, circular dependencies, and cross-cutting concerns."
- **ReviewFrontend** + `quick`: "Only check i18n key completeness and direct platform boundary violations in changed frontend files."
- **ReviewFrontend** + `normal`: "Check i18n, React performance patterns, and accessibility in changed components. Verify frontend-backend API contract alignment."
- **ReviewFrontend** + `deep`: "Thorough React analysis: effect dependencies, memoization, virtualization. Full accessibility audit. State management pattern review. Cross-layer contract verification."
```

### 4.6 Frontend Strategy Directives

Add to `REVIEW_STRATEGY_PROFILES` in `reviewTeamService.ts`:

```typescript
// Architecture Reviewer directives
ReviewArchitecture: {
  quick: 'Only check imports directly changed by the diff. Flag violations of documented layer boundaries.',
  normal: "Check the diff's imports plus one level of dependency direction. Verify API contract consistency.",
  deep: 'Map the full dependency graph for changed modules. Check for structural anti-patterns, circular dependencies, and cross-cutting concerns.',
}

// Frontend Reviewer directives
ReviewFrontend: {
  quick: 'Only check i18n key completeness and direct platform boundary violations in changed frontend files.',
  normal: 'Check i18n, React performance patterns, and accessibility in changed components. Verify frontend-backend API contract alignment.',
  deep: 'Thorough React analysis: effect dependencies, memoization, virtualization. Full accessibility audit. State management pattern review. Cross-layer contract verification.',
}
```

### 4.7 Summary of All Prompt Changes

| File | Change Type | Description |
|------|-------------|-------------|
| `review_business_logic_agent.md` | Modify | Remove "or UI" from partial updates; scope deep strategy to correctness; add exclusion note |
| `review_performance_agent.md` | Modify | Remove "renders" from mission; remove "unnecessary re-renders" from efficiency rules; add exclusion note |
| `review_security_agent.md` | Modify | Clarify "trust-boundary violations" scope to exploitable risks; add exclusion note |
| `review_quality_gate_agent.md` | Modify | Add overlap detection guidance for Architecture/BL and Frontend/Perf; update partial timeout handling |
| `deep_review_agent.md` | Modify | Update team shape; add Architecture and Frontend to mandatory/conditional roles; add strategy amplification entries |
| `review_architecture_agent.md` | **New** | Full prompt for Architecture Reviewer |
| `review_frontend_agent.md` | **New** | Full prompt for Frontend Reviewer |

## Part 5: Implementation Priority

### Completed: Reviewer Role Expansion

1. **Architecture Reviewer (always-on)** - implemented as a core reviewer role with dedicated prompt, backend registration, frontend metadata, and strategy directives.
2. **Frontend Reviewer metadata and prompt** - implemented with dedicated prompt, backend registration, frontend metadata, settings i18n, and UI support.
3. **Existing reviewer prompt adjustments** - implemented to clarify ownership boundaries and reduce cross-role duplication.
4. **Judge overlap handling** - implemented for Architecture/Business Logic, Architecture/Security, Frontend/Performance, and Frontend/Business Logic overlap cases.
5. **UI support** - implemented for Review Team page, Settings > Review member names, Agents overview Code Review Team card, continuation/report/remediation flows, and hidden-agent filtering.
6. **Backend-provided reviewer definition** - implemented as a core `default_review_team_definition()` manifest surfaced through the desktop API and consumed by the frontend review team resolver.
7. **Dynamic hidden-agent derivation** - implemented for Agents overview by combining static non-review hidden IDs with backend-provided review-agent hidden IDs.

### Current Next Phase: Strategy Engine Closure (Highest Priority)

1. **Queue-aware backend dispatch**: Keep the current bounded local-cap wait, backend-bound local-cap queue controls, explicit provider transient-capacity skip conversion, and turn-local effective-cap learning as narrow TaskTool-owned behavior, not a full scheduler. Add automatic provider/adaptive queueing, staggered backend batches, and user-facing effective-cap override controls only in small verified rounds. The queue design must keep queue time separate from execution timeout, expose honest user controls, and avoid starving normal user session concurrency.
2. **Retry execution semantics**: Keep backend-owned redispatch prompt-guided unless automatic reduced-scope dispatch is explicitly implemented. The retry hint uses the effective manifest policy, the action bar can launch explicit structured retry slices, and TaskTool now accepts retry Tasks only with structured coverage, reduced scope, retryable source status, lower timeout, available retry budget, and bounded retry-scope prompt injection. `auto_retry` admission additionally requires Review Team opt-in and elapsed guard.
3. **Incremental cache expansion**: Keep the implemented per-session `packet_id` cache path. Cached reviewer outputs have no independent retention period beyond session metadata, and project-level reuse must wait for explicit retention, deletion, invalidation, and user-visibility rules.

### Current Next Phase: Dynamic Control And Governance (High Priority)

4. **Runtime strategy authority**: Keep backend `auto_select_strategy()` as advisory/mismatch-warning metadata. Only revisit authoritative auto-selection after measured complexity delta exists and product explicitly accepts strategy changes that can alter token/concurrency cost.
5. **Token and context budgets**: Keep heuristic prompt-byte estimates and full-scope summary-first metadata as the current boundary. Add hard clipping or byte-accurate enforcement only after it can preserve explicit coverage for every file.
6. **Operational evidence**: Keep the implemented report reliability surfaces for partial timeouts, retry guidance, cache hits/misses, skipped reviewers, token-budget tradeoffs, and TaskTool cap rejections. Keep shared-context duplicate measurement local and non-reporting; final Deep Review submission may emit aggregate debug counts for local sampling, but real runs must show that programmatic reuse is worth the runtime complexity before adding interception or cache reuse. Add external telemetry only if product diagnostics require it.
7. **Cost-aware scope depth**: Add a manifest-level depth profile before broadening runtime scheduling. `quick` should focus only on high-risk hunks and direct contract/security/config/concurrency paths, `normal` should review changed code plus one-hop high-risk context, and `deep` remains the full-depth option. Reports must label reduced-depth coverage honestly.
8. **Shared evidence pack**: Precompute compact source-agnostic evidence once per run and pass it to every reviewer. Start with metadata, hunk hints, domain/risk tags, packet ids, and cheap contract hints; keep full `Read` output reuse deferred until duplicate-call measurements prove it is worth the tool-pipeline complexity.

### Superseded Next Phase: Strategy Engine Foundation

1. **Predictive timeout refinement** - Add real diff line-count stats and keep frontend/core timeout formulas aligned.
2. **Partial result capture** - Prevents total work loss on timeout.
3. **Conditional Frontend dispatch** - Move from metadata-level inclusion to diff-aware launch behavior so frontend review only runs when frontend files are present.

### Superseded Next Phase: Dynamic Control

4. **Change risk auto-classification** - Reduces misconfiguration; ~30% time savings.
5. **Dynamic concurrency control** - Prevents rate limit violations, especially important with 4-5 reviewers.
6. **Retry budget** - Improves resilience for transient failures.

### Implementation Additions (Beyond Original Design)

The following additions emerged during implementation as natural extensions of the original design.

#### ContextHealthSnapshot

**Added in**: `execution_engine.rs`

A runtime health snapshot used by the compression and context-profile subsystems to detect degraded sessions:

```rust
struct ContextHealthSnapshot {
    token_usage_ratio: f64,              // current / context_window
    repeated_tool_signature_count: usize, // same tool+args pattern in consecutive turns
    consecutive_failed_commands: usize,   // back-to-back tool errors
}
```

**Purpose**: The Context Profile Policy (Section 1.3) needs runtime signals to decide when to downgrade concurrency or switch to a lighter compression strategy. `ContextHealthSnapshot` provides these signals from observed turn history rather than static configuration.

**Integration points**:
- `context_profile.rs` uses the snapshot to adjust `LongTask` profile concurrency limits when `repeated_tool_signature_count > 2` or `consecutive_failed_commands > 1`.
- The compression subsystem uses `token_usage_ratio` to decide between model-based and fallback compression.

#### ModelCapabilityProfile

**Added in**: `context_profile.rs`

A lightweight model capability classifier used to adapt review behavior for weaker models:

```rust
enum ModelCapabilityProfile {
    Standard, // full-featured models
    Weak,     // models with limited reasoning (detected by id heuristic)
}
```

**Detection heuristic**: Matches model id against known weak-model suffixes (`haiku`, `mini`, `flash`, etc.).

**Purpose**: Weak models require different concurrency and context strategies (lower parallel reviewer count, smaller per-reviewer context windows, reduced file-splitting). This is a runtime complement to the user-configured strategy level.

**Integration points**:
- `context_profile.rs` reduces `max_parallel_reviewers` for `Weak` models.
- `deep_review_policy.rs` can lower predictive timeout multipliers for weak models (future work).

#### Extended Review Target Path Classification

**Added in**: `reviewTargetClassifier.ts`

The original design defined a simple `hasFrontendFiles()` boolean check. The implementation extends this to a multi-domain path classification system with 15+ tag rules:

| Domain Tag | Path Patterns | Purpose |
|---|---|---|
| `frontend_ui` | `src/web-ui/src/**`, `*.tsx` | Frontend UI components |
| `frontend_style` | `*.scss`, `*.css` (in web-ui) | Frontend styling |
| `frontend_i18n` | `**/locales/**` | Internationalization files |
| `frontend_contract` | `src/apps/desktop/src/api/**` | Frontend-backend API surface |
| `desktop_contract` | `src/apps/desktop/**` | Desktop-specific integration |
| `backend_core` | `src/crates/core/**` | Core Rust logic |
| `api_layer` | `src/crates/api-layer/**` | API abstraction layer |
| `transport` | `src/crates/transport/**` | Transport adapters |

**Purpose**: Fine-grained classification enables:
1. More accurate `recommendReviewStrategyForTarget()` scoring (e.g., `contractSurfaceChanged` flag).
2. Conditional reviewer activation beyond just Frontend by extending the reviewer applicability registry (e.g., future backend-only optimizations).
3. Pre-review summary with workspace area breakdown.

**Backward compatibility**: The simple `hasFrontendFiles()` check is derived from the tags: `target.tags.includes('frontend_ui') || target.tags.includes('frontend_style') || target.tags.includes('frontend_i18n')`.

### Future Role Extensibility Improvements

7. **Locale completeness checks** - Add tests that fail when a core role is missing translations in `scenes/agents.json` or `settings/review.json`.
8. **Card layout resilience tests** - Add visual or DOM-level tests ensuring role summary cards do not clip content when role count grows.

### Advanced (Lower Priority)

13. **Shared context cache** - Programmatic reuse remains deferred; current runtime measures duplicate `Read`/`GetFileDiff` calls and emits aggregate local debug diagnostics at report submission. The next lower-risk step is a shared evidence pack, not cross-subagent full-file result caching.
14. **Incremental review caching** - Per-session packet cache is implemented; project-level follow-up reuse remains product-decision-required.

## Verification

| Change Type | Verification Command |
|-------------|---------------------|
| Rust policy changes | `cargo test -p bitfun-core deep_review -- --nocapture` |
| Rust coordinator changes | `cargo test -p bitfun-core coordination -- --nocapture` |
| Frontend service changes | `pnpm run type-check:web && pnpm --dir src/web-ui run test:run` |
| Full integration | `cargo build -p bitfun-desktop` + manual deep review test |

## Appendix A: Architecture Reviewer Prompt Draft

```markdown
# Role

You are an **independent Architecture Reviewer** for BitFun deep reviews.

{LANGUAGE_PREFERENCE}

You work in an isolated context. Treat this as a fresh review. Do not assume the main agent or other reviewers are correct.

## Mission

Inspect the requested review target and find **structural and architectural issues** such as:

- module boundary violations (imports that cross layer boundaries)
- API contract design problems (inconsistent patterns, breaking changes)
- abstraction integrity issues (platform-specific details leaking through shared interfaces)
- dependency direction violations (circular dependencies, wrong-direction imports)
- structural consistency (patterns, registration conventions not followed)
- cross-cutting concern impact (changes that require touching too many layers)

## What you do NOT review

- Business rule correctness (Business Logic reviewer handles this)
- Algorithm performance (Performance reviewer handles this)
- Security vulnerabilities (Security reviewer handles this)
- React component state, i18n, or accessibility (Frontend Reviewer handles this)
- Code style or formatting

## Tools

Use only read-only investigation:

- `GetFileDiff`
- `Read`
- `Grep`
- `Glob`
- `LS`
- `Git` with read-only operations only

Never modify files or git state.

## Review standards

- Confirm the violation before reporting. Cite the specific architectural rule or convention being violated.
- Prefer findings with concrete evidence (actual import paths, dependency chains) over speculative concerns.
- If a dependency direction is unusual but does not violate a documented rule, lower severity.

## Efficiency rules

- Start by understanding the module structure. Use LS and Glob to map the directory layout and identify layer boundaries.
- Focus on imports and cross-module references. Use Grep to trace import patterns rather than reading full files.
- Only read full files when an import pattern suggests a boundary violation.
- When you have confirmed or dismissed an architectural concern, move on. Do not re-examine the same module from different angles.
- Prefer a focused report with confirmed violations over a broad survey that risks timing out.
- If the strategy is `quick`, only check imports directly changed by the diff. Flag violations of documented layer boundaries.
- If the strategy is `normal`, check the diff's imports plus one level of dependency direction. Verify API contract consistency.
- If the strategy is `deep`, map the full dependency graph for changed modules. Check for structural anti-patterns, circular dependencies, and cross-cutting concerns.

## Output format

Return markdown only, using this exact structure:

## Reviewer
Architecture Reviewer

## Verdict
clear | issues_found

## Findings
- `[severity=<critical|high|medium|low>] [certainty=<confirmed|likely>] file:line - title`
  Architectural rule violated: ...
  Why it matters: ...
  Suggested fix direction: ...

If there are no confirmed or likely issues, write exactly:

- No architectural issues found.

## Reviewer Summary
2-4 sentences summarizing the structural health of the change.

If there is nothing meaningful to summarize, write exactly:

- Nothing to summarize.
```

## Appendix B: Frontend Reviewer Prompt Draft

```markdown
# Role

You are an **independent Frontend Reviewer** for BitFun deep reviews.

{LANGUAGE_PREFERENCE}

You work in an isolated context. Treat this as a fresh review. Do not assume the main agent or other reviewers are correct.

## Mission

Inspect the requested review target and find **frontend-specific issues** such as:

- i18n key synchronization problems (missing keys in one or more locales)
- React performance anti-patterns (missing memoization, unnecessary re-renders, missing virtualization)
- Accessibility violations (missing ARIA attributes, keyboard navigation, focus management)
- State management issues (Zustand selector granularity, store dependency problems, stale closures)
- Frontend-backend API contract drift (Tauri command type mismatches, event payload changes without frontend updates)
- Platform boundary violations in frontend (direct @tauri-apps/api imports outside the adapter layer)
- CSS/theme consistency issues (ThemeService misuse, component library pattern violations)

## What you do NOT review

- Business rule correctness (Business Logic reviewer handles this)
- Non-React algorithm performance (Performance reviewer handles this)
- Security vulnerabilities (Security reviewer handles this)
- Backend architectural issues (Architecture reviewer handles this)
- Code style or formatting

## Tools

Use only read-only investigation:

- `GetFileDiff`
- `Read`
- `Grep`
- `Glob`
- `LS`
- `Git` with read-only operations only

Never modify files or git state.

## Review standards

- Confirm the issue before reporting. Show the specific code that has the problem.
- For i18n issues: verify that a key exists in one locale but is missing in another.
- For React performance issues: explain the concrete performance impact, not just the pattern violation.
- For accessibility issues: reference WCAG guidelines where applicable.
- If a pattern is unusual but functional, lower severity.

## Efficiency rules

- Start from the diff. Identify changed frontend files (.tsx, .ts, .scss, locale JSON).
- For i18n: use Grep to find all `t('...')` calls in changed files, then check each key across all locale files.
- For React performance: check changed components for common anti-patterns (inline functions in JSX, missing keys, missing memo).
- For accessibility: check changed components for ARIA attributes, keyboard handlers, and focus management.
- For API contracts: compare changed Tauri command types with corresponding TypeScript API clients.
- When you have confirmed or dismissed a frontend concern, move on. Do not re-examine the same component from different angles.
- Prefer a focused report with confirmed issues over a broad survey that risks timing out.
- If the strategy is `quick`, only check i18n key completeness and direct platform boundary violations in changed frontend files.
- If the strategy is `normal`, check i18n, React performance patterns, and accessibility in changed components. Verify frontend-backend API contract alignment.
- If the strategy is `deep`, thorough React analysis: effect dependencies, memoization, virtualization. Full accessibility audit. State management pattern review. Cross-layer contract verification.

## Output format

Return markdown only, using this exact structure:

## Reviewer
Frontend Reviewer

## Verdict
clear | issues_found

## Findings
- `[severity=<critical|high|medium|low>] [certainty=<confirmed|likely>] file:line - title`
  Why it matters: ...
  Suggested fix: ...

If there are no confirmed or likely issues, write exactly:

- No frontend issues found.

## Reviewer Summary
2-4 sentences summarizing the frontend health of the change.

If there is nothing meaningful to summarize, write exactly:

- Nothing to summarize.
```

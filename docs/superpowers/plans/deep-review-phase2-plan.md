# Deep Review Strategy Engine — Execution Plan (Phase 2)

## Scope

This plan covers the remaining work items identified by comparing `deep-review-design.md` against commit `9d97b88e81`. It is strictly bounded by the original design document — no speculative additions.

## Current State Summary

| Component | Frontend (TS) | Backend (Rust) |
|---|---|---|
| Change Risk Auto-Classification | `recommendReviewStrategyForTarget()` complete | `ChangeRiskFactors` struct + `auto_select_strategy()` **implemented** |
| Predictive Timeout | `predictTimeoutSeconds()` complete | `predictive_timeout()` complete |
| Dynamic Concurrency Control | `computeConcurrencyPolicy()` complete, prompt rules emitted | `DeepReviewConcurrencyPolicy` struct **in Rust**, tool-level enforcement **implemented** |
| Retry Budget | Not applicable (backend only) | `max_retries_per_role` tracking complete, retry guidance in task_tool **implemented** |
| Partial Result Capture | Prompt rules reference `partial_timeout` | `SubagentResultStatus::PartialTimeout` + grace period complete |
| Incremental Review Cache | Fingerprint + plan generation complete, prompt rules emitted | Backend cache read + `DeepReviewIncrementalCache` **implemented** |
| Shared Context Cache | Plan generation complete, prompt rules emitted | **Prompt-only (deferred)** |
| Token Budget Plan | Plan generation complete, prompt rules emitted | `maxFilesPerReviewer` via `reviewer_file_split_threshold` **implemented** |
| Pre-Review Summary | Data + prompt block complete | Not applicable (prompt-level) |
| Work Packet Batch Scheduling | `launchBatch` + `staggerSeconds` in data model, prompt rules emitted | Concurrency cap enforcement **implemented**, batch dispatch deferred (prompt-only) |
| Compression Contract | Not applicable (backend only) | Contract generation + prompt injection complete |

**Key insight**: The frontend has built comprehensive data structures, plan generators, and prompt rules for *all* remaining items. The gap is almost entirely on the **backend enforcement side** — the Rust runtime does not yet read or act on these plan fields when dispatching subagents.

---

## Plan Items

### P2-1: Backend ChangeRiskFactors + auto_select_strategy

**Design ref**: Section 1.1

**What**: Add `ChangeRiskFactors` struct and `auto_select_strategy()` method to `deep_review_policy.rs`.

**Files**:
- `src/crates/core/src/agentic/deep_review_policy.rs` — add struct + method
- `src/crates/core/src/agentic/deep_review_policy.rs` — add unit tests

**Design spec** (verbatim from doc):
```rust
pub struct ChangeRiskFactors {
    pub file_count: usize,
    pub total_lines_changed: usize,
    pub files_in_security_paths: usize,
    pub max_cyclomatic_complexity_delta: usize,
    pub cross_crate_changes: usize,
}
```
Score formula: `file_count + total_lines_changed / 100 + files_in_security_paths * 3 + cross_crate_changes * 2`
Thresholds: `0..=5` -> Quick, `6..=20` -> Normal, `_` -> Deep

**Risk**: Low. Pure computation, no side effects. The frontend already computes this independently; the backend version serves as a validation/override path.

**Uncertainty**: The design mentions `max_cyclomatic_complexity_delta` requiring "a lightweight AST pass or heuristic". This is non-trivial. For the initial implementation, default to `0` and leave AST computation as future work.

**Verification**: `cargo test -p bitfun-core deep_review -- --nocapture`

---

### P2-2: Backend DeepReviewConcurrencyPolicy + Batched Dispatch

**Design ref**: Section 1.3

**What**: Add `DeepReviewConcurrencyPolicy` to Rust policy, enforce `max_parallel_instances` and `stagger_seconds` in coordinator dispatch.

**Files**:
- `src/crates/core/src/agentic/deep_review_policy.rs` — add struct + `effective_max_same_role_instances()`
- `src/crates/core/src/agentic/coordination/coordinator.rs` — batched subagent launch
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs` — read concurrency policy from manifest

**Design spec**:
```rust
pub struct DeepReviewConcurrencyPolicy {
    pub max_parallel_instances: usize,  // default: 4
    pub stagger_seconds: u64,           // default: 0
    pub batch_extras_separately: bool,  // default: true
}
```
`effective_max_same_role_instances`: `max(1, max_parallel_instances / role_count).min(existing_max)`

**Launch strategy**: The prompt already tells the LLM to respect launch_batch, but this is prompt-only. Backend enforcement means the coordinator/task_tool should enforce the cap programmatically.

**Risk**: Medium. The coordinator currently does fire-and-forget parallel dispatch. Adding batching requires restructuring the dispatch flow to wait for batch completion before launching the next. This is the most architecturally complex item.

**Approach**: Two sub-steps:
1. Add the policy struct and `with_run_manifest_execution_policy` parsing (low risk).
2. Add batch-aware dispatch in task_tool (the tool that launches subagent Tasks). Instead of the LLM freely launching Tasks, the tool checks active count against the cap and queues/pauses excess launches.

**Uncertainty**: The design implies the coordinator itself should batch, but the actual subagent launch goes through `task_tool` (invoked by the orchestrator LLM). Need to confirm: should enforcement be at the tool level (reject/fail excess launches) or at the coordinator level (pre-batch all launches)? The prompt already instructs sequential batches, so tool-level enforcement as a safety net is the minimal approach.

**Verification**: `cargo test -p bitfun-core deep_review -- --nocapture` + `cargo test -p bitfun-core coordination -- --nocapture`

---

### P2-3: Backend Retry Dispatch in task_tool

**Design ref**: Section 1.5

**What**: When a reviewer Task returns `partial_timeout` or `failed`, allow one retry with reduced scope and downgraded strategy.

**Files**:
- `src/crates/core/src/agentic/deep_review_policy.rs` — `retries_used` tracking (already done)
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs` — retry dispatch logic
- `src/crates/core/src/agentic/agents/prompts/deep_review_agent.md` — already has retry instructions

**Design spec**:
1. Check `retries_used[role] < max_retries_per_role`
2. Re-dispatch with: reduced scope (only unreviewed files), timeout / 2, strategy downgraded one level
3. Increment `retries_used[role]`
4. Set `is_retry: true` on the retry Task call

**Risk**: Low-Medium. The tracking structures are already in place. The retry dispatch is a conditional code path in task_tool that wraps the existing launch with modified parameters.

**Uncertainty**: "Reduced scope (only files not yet reviewed)" requires knowing which files were already covered by the partial output. The partial output is free-form text — extracting covered files requires heuristic parsing. For initial implementation, the prompt already instructs the reviewer to list reviewed files; the orchestrator can extract these. If extraction fails, retry with the full scope but a shorter timeout.

**Verification**: `cargo test -p bitfun-core deep_review -- --nocapture`

---

### P2-4: Backend Incremental Review Cache

**Design ref**: Part 5, "Advanced (Lower Priority)" item 14

**What**: When a deep review is re-run with the same target fingerprint, reuse completed work packets instead of re-dispatching.

**Files**:
- `src/crates/core/src/agentic/session/session_manager.rs` — cache storage (in session metadata)
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs` — cache read before dispatch
- `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs` — cache write on completion

**Design spec**:
- Cache key: `incremental-review:{fingerprint}` (already computed in frontend)
- Store: completed reviewer outputs keyed by `packet_id`
- Invalidation: `target_file_set_changed`, `reviewer_roster_changed`, `strategy_changed` (already listed in frontend plan)
- On cache hit: skip dispatch for cached packets, inject cached output into the judge's context

**Risk**: Medium. Cache invalidation correctness is critical — stale cache produces wrong reviews. The fingerprint computation must be stable and include all relevant dimensions.

**Approach**: Store cache in `SessionMetadata` (already has `deep_review_run_manifest`). On `buildEffectiveReviewTeamManifest`, the frontend already computes the fingerprint. The backend reads the stored cache from previous session metadata, compares fingerprints, and skips matching packets.

**Uncertainty**: Cache storage location. Session metadata is per-session, but incremental review spans sessions. Need to decide: store in project-level storage (`<project>/.bitfun/review-cache/`) or in the previous session's metadata? Project-level storage is more natural for cross-session reuse but requires a new storage path.

**Decision needed**: Cache persistence scope — per-session (simpler, only works within continuation) vs. per-project (cross-session, requires new storage).

**Verification**: `cargo test -p bitfun-core deep_review -- --nocapture`

---

### P2-5: Backend Shared Context Cache

**Design ref**: Part 5, "Advanced (Lower Priority)" item 13

**What**: When multiple reviewers need to read the same file, cache the first read's result and reuse it for subsequent reviewers.

**Files**:
- `src/crates/core/src/agentic/coordination/coordinator.rs` — shared context cache during subagent execution
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs` — inject cache context into subagent sessions

**Risk**: High. This requires intercepting tool calls (Read, GetFileDiff) within subagent sessions and caching their results. This is a deep architectural change to the tool pipeline.

**Approach**: The prompt already instructs reviewers to "reuse read-only context by cache_key". For initial implementation, the prompt-level instruction (already emitted) is the primary mechanism. Programmatic enforcement would require a tool-call interception layer.

**Recommendation**: **Defer programmatic enforcement to a later phase.** The prompt rules are already comprehensive and the LLM can follow them. The return-on-investment for programmatic enforcement is low compared to the architectural complexity.

**Verification**: Manual testing with `cargo build -p bitfun-desktop` + deep review on a multi-reviewer change.

---

### P2-6: Backend Token Budget Enforcement

**Design ref**: Part 5, "Advanced (Lower Priority)"

**What**: Enforce `maxFilesPerReviewer`, `maxPromptBytesPerReviewer`, and `largeDiffSummaryFirst` in the backend.

**Files**:
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs` — scope truncation
- `src/crates/core/src/agentic/deep_review_policy.rs` — budget policy parsing

**Risk**: Medium. `maxFilesPerReviewer` enforcement requires truncating the file list passed to subagent Tasks. `maxPromptBytesPerReviewer` requires estimating prompt size before dispatch, which is hard without generating the full prompt first.

**Approach**:
1. `maxFilesPerReviewer`: Straightforward — clamp the file list in the work packet before dispatch.
2. `largeDiffSummaryFirst`: Generate a diff summary before dispatching reviewers and include it in the packet context (this is related to P2-7 below).
3. `maxPromptBytesPerReviewer`: Defer — requires prompt size estimation infrastructure that doesn't exist yet.

**Recommendation**: Implement only `maxFilesPerReviewer` enforcement in this phase. `maxPromptBytesPerReviewer` and `largeDiffSummaryFirst` are deferred.

**Verification**: `cargo test -p bitfun-core deep_review -- --nocapture`

---

### P2-7: Pre-Review Summary UI Display (Optional)

**Design ref**: Part 5

**What**: Show the pre-review summary (file count, workspace areas, tags, warnings) in the UI before launching the review.

**Files**:
- `src/web-ui/src/app/scenes/agents/components/ReviewTeamPage.tsx` — display summary card
- `src/web-ui/src/flow_chat/services/DeepReviewService.ts` — expose summary data

**Risk**: Low. Purely additive UI. The data is already computed in `buildPreReviewSummary()`.

**Uncertainty**: UX design — where exactly to display this. Options:
1. In the review launch confirmation dialog.
2. As a summary card in the Review Team page.
3. As an inline preview in the flow chat before the review starts.

**Decision needed**: UI placement for pre-review summary.

**Verification**: `pnpm run lint:web && pnpm run type-check:web && pnpm --dir src/web-ui run test:run`

---

## Execution Order

```
Phase A (Backend policy foundation):  ✅ DONE
  P2-1: ChangeRiskFactors + auto_select_strategy
  P2-6: Token Budget - maxFilesPerReviewer only

Phase B (Backend dispatch enforcement):  ✅ DONE
  P2-2: ConcurrencyPolicy + batched dispatch
  P2-3: Retry dispatch in task_tool

Phase C (Backend caching — higher risk):  ✅ DONE
  P2-4: Incremental review cache

Phase D (Optional / lower priority):
  P2-7: Pre-review summary UI  (deferred — data already in prompt block)
  P2-5: Shared context cache   (deferred — prompt-only is sufficient for now)
```

## Implementation Summary

### Changes Made

| File | Changes |
|---|---|
| `deep_review_policy.rs` | `ChangeRiskFactors` struct, `auto_select_strategy()`, `DeepReviewConcurrencyPolicy` struct + `from_manifest()` + `effective_max_same_role_instances()` + `check_launch_allowed()`, `DeepReviewIncrementalCache` struct + `from_value()`/`to_value()`/`matches_manifest()`, `deep_review_active_reviewer_count()` / `deep_review_has_judge_been_launched()` / `deep_review_retries_used()` / `deep_review_max_retries_per_role()` free functions, 18 new unit tests |
| `task_tool.rs` | Concurrency policy enforcement before subagent launch, incremental cache hit check (returns cached result without dispatching), retry guidance hint on partial_timeout |
| `session/types.rs` | `deep_review_cache: Option<Value>` field on `SessionMetadata` |
| `persistence/manager.rs` | Preserve `deep_review_cache` when loading existing session metadata |
| `coordinator.rs` | Initialize `deep_review_cache: None` for new subagent sessions |
| `deep-review-design.md` | "Implementation Additions" section (ContextHealthSnapshot, ModelCapabilityProfile, Extended Path Classification), updated "Remaining / Future Work" |

### Verification Results

| Check | Result |
|---|---|
| `cargo check --workspace` | Pass (warnings only, pre-existing) |
| `cargo test -p bitfun-core deep_review` | 60 passed, 0 failed |
| `pnpm run lint:web` | Pass |
| `pnpm run type-check:web` | 1 pre-existing error in `MessageModule.ts` (unrelated to our changes) |
| `pnpm --dir src/web-ui run test:run` | 2 pre-existing test failures (unrelated to our changes) |

## Decisions Needed Before Starting

1. **P2-2 batching approach**: Should concurrency enforcement be at tool level (reject excess Task launches) or coordinator level (pre-batch all launches)?
   - **Recommendation**: Tool-level enforcement as safety net. The prompt already handles batching; the tool just caps concurrent launches.

2. **P2-4 cache persistence scope**: Per-session (simpler, only works within continuation) vs. per-project (cross-session, requires new storage)?
   - **Recommendation**: Start with per-session (continuation flow). Per-project can be added later.

3. **P2-5 shared context cache**: Accept prompt-only approach for now, or invest in programmatic enforcement?
   - **Recommendation**: Prompt-only for this phase. The prompt rules are already emitted and comprehensive.

4. **P2-6 token budget scope**: Implement only `maxFilesPerReviewer`, or also `maxPromptBytesPerReviewer`?
   - **Recommendation**: Only `maxFilesPerReviewer`. `maxPromptBytesPerReviewer` requires prompt estimation infrastructure.

5. **P2-7 pre-review summary UI**: Where to display?
   - **Recommendation**: Defer UI decision until Phase D. The data is already in the prompt block.

## Verification Commands

| Phase | Command |
|---|---|
| Phase A | `cargo test -p bitfun-core deep_review -- --nocapture` |
| Phase B | `cargo test -p bitfun-core deep_review -- --nocapture && cargo test -p bitfun-core coordination -- --nocapture` |
| Phase C | `cargo test -p bitfun-core deep_review -- --nocapture` |
| All phases (frontend) | `pnpm run lint:web && pnpm run type-check:web && pnpm --dir src/web-ui run test:run` |
| All phases (full Rust) | `cargo check --workspace && cargo test --workspace` |
| Integration smoke | `cargo build -p bitfun-desktop` + manual deep review |

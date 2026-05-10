# Deep Review Strategy Engine - Execution Plan (Phase 2)

## Scope

This plan covers the remaining work items identified by comparing `deep-review-design.md` against commit `9d97b88e81`. It is strictly bounded by the original design document - no speculative additions.

## Status Reconciliation

This document now distinguishes between three implementation levels:

- **Complete**: Runtime behavior is deterministic and covered by tests.
- **Safety net / prompt-guided**: Runtime has policy parsing, manifest data, or a protective check, but the orchestrator model still owns sequencing.
- **Deferred**: The document intentionally keeps the item out of the current implementation boundary.

The follow-up implementation plan for the remaining semantic gaps is tracked in `docs/deep-review-phase2-addendum.md`.
The addendum is the live progress ledger for the remaining rounds. After each implementation round, update the addendum status first, then reconcile this document and `docs/deep-review-design.md` only when the high-level state changes.

## Current State Summary

| Component | Frontend (TS) | Backend (Rust) |
|---|---|---|
| Change Risk Auto-Classification | `recommendReviewStrategyForTarget()` complete; manifest strategy-decision metadata records frontend/backend-compatible recommendations, user override, final strategy, mismatch state, and severity | `ChangeRiskFactors` struct + `auto_select_strategy()` **implemented as pure policy helper**; backend-compatible scoring is advisory/mismatch-warning metadata only, and measured complexity delta remains deferred |
| Predictive Timeout | `predictTimeoutSeconds()` complete | `predictive_timeout()` complete |
| Dynamic Concurrency Control | `computeConcurrencyPolicy()` complete, prompt rules emitted | `DeepReviewConcurrencyPolicy` parsing, bounded TaskTool local-cap waiting, backend-bound local-cap queue controls, turn-local effective cap learning after local capacity skips, and explicit provider transient-capacity skip conversion **implemented**; capacity-error classification, queue-state metadata/event contract, compact queue notice, local/manual queue controls, and active-session concurrency warning **implemented**; automatic provider requeue/retry execution, backend stagger scheduling, and user-facing override controls are deferred, and capacity skips are folded into final report reliability signals |
| Retry Budget | Not applicable (backend only) | `max_retries_per_role` tracking, TaskTool retry guidance, structured retry admission, and bounded retry-scope prompt injection **implemented**; backend-owned automatic redispatch is deferred, and retry guidance uses the effective manifest policy when available |
| Partial Result Capture | Prompt rules reference `partial_timeout` | `SubagentResultStatus::PartialTimeout` + coordinator grace-period capture complete, limited to final text returned inside the grace window |
| Incremental Review Cache | Fingerprint + plan generation complete, prompt rules emitted | Per-session `DeepReviewIncrementalCache`, metadata storage, TaskTool cache-hit read path, completed-reviewer write-through, packet-key alignment, and hit/miss report signals **implemented**; project-level persistence is product-decision-required and deferred |
| Shared Context Cache | Plan generation complete, prompt rules emitted | **Prompt-only with local duplicate Read/GetFileDiff measurement and aggregate debug diagnostics; result reuse deferred** |
| Token Budget Plan | Plan generation complete, prompt rules emitted; heuristic max reviewer prompt-byte estimate, per-mode byte threshold, and full-scope summary-first decision metadata **implemented** | File-split/max-file style guardrails **implemented**; hard prompt-byte clipping and byte-accurate enforcement are deferred |
| Pre-Review Summary | Data + prompt block complete; compact launch-dialog summary **implemented** | User-facing compact consent summary implemented; separate dense pre-review report remains deferred |
| Work Packet Batch Scheduling | `launchBatch` + `staggerSeconds` in data model, prompt rules emitted | Prompt-guided batching plus hard-cap safety net **implemented**; deterministic backend batch dispatcher deferred |
| Compression Contract | Not applicable (backend only) | Contract generation + prompt injection complete |

**Key insight**: The frontend has built comprehensive data structures, plan generators, and prompt rules for all remaining items. The backend now reads several of those fields and enforces hard safety nets, including structured retry admission, but deterministic scheduling, backend-owned retry redispatch, project-level cache reuse, and byte-level budget enforcement remain open.

---

## Plan Items

### P2-1: Backend ChangeRiskFactors + auto_select_strategy

**Design ref**: Section 1.1

**What**: Add `ChangeRiskFactors` struct and `auto_select_strategy()` method to `deep_review_policy.rs`.

**Files**:
- `src/crates/core/src/agentic/deep_review_policy.rs` - add struct + method
- `src/crates/core/src/agentic/deep_review_policy.rs` - add unit tests

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

**Uncertainty**: The design mentions `max_cyclomatic_complexity_delta` requiring "a lightweight AST pass or heuristic". This is non-trivial. Current launch metadata records `0` with source `not_measured`, and the field is excluded from runtime authority until a measured signal exists.

**Verification**: `cargo test -p bitfun-core deep_review -- --nocapture`

---

### P2-2: Backend DeepReviewConcurrencyPolicy + Cap Safety Net

**Design ref**: Section 1.3

**What**: Add `DeepReviewConcurrencyPolicy` to Rust policy and enforce `max_parallel_instances` before TaskTool launches Deep Review subagents. This is a cap safety net, not a deterministic backend batch scheduler.

**Files**:
- `src/crates/core/src/agentic/deep_review_policy.rs` - add struct + `effective_max_same_role_instances()`
- `src/crates/core/src/agentic/coordination/coordinator.rs` - future queue/stagger scheduler if backend-owned batching is added
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs` - read concurrency policy from manifest

**Design spec**:
```rust
pub struct DeepReviewConcurrencyPolicy {
    pub max_parallel_instances: usize,  // default: 4
    pub stagger_seconds: u64,           // default: 0
    pub batch_extras_separately: bool,  // default: true
}
```
`effective_max_same_role_instances`: `max(1, max_parallel_instances / role_count).min(existing_max)`

**Launch strategy**: The prompt tells the LLM to respect `launch_batch`, while TaskTool now bounded-waits when local reviewer capacity is saturated, queues later launch batches while an earlier reviewer batch is still active, converts expired waits to `CapacitySkipped`, and converts explicit provider transient-capacity reviewer failures to `capacity_skipped` with turn-local effective-cap learning. This is still not a complete backend batch scheduler: `staggerSeconds`, skipped/unstarted earlier-batch detection, automatic provider requeue/retry execution, and user-facing effective-cap overrides remain deferred.

**Risk**: Medium. The coordinator currently does fire-and-forget parallel dispatch. Adding batching requires restructuring the dispatch flow to wait for batch completion before launching the next. This is the most architecturally complex item.

**Approach**: Two sub-steps:
1. Add the policy struct and `with_run_manifest_execution_policy` parsing (low risk).
2. Add TaskTool cap enforcement as the first safety net, then bounded local-cap waiting once queue state and report propagation are verified. Backend-bound pause/continue/cancel/optional-skip controls are implemented for local-cap waits; provider/adaptive queueing remains an addendum follow-up.

**Adaptive queue follow-up boundary**: The implemented queue path is deliberately narrow: it waits only for local reviewer-cap saturation, separates queue time from reviewer runtime, emits queue-state events, supports backend-bound local-cap pause/continue/cancel/optional-skip controls, learns a turn-local effective cap after local capacity skips or explicit provider transient-capacity failures, and reports those skips as `concurrency_limited`. Future queueing must stay narrower than a full backend DAG scheduler unless explicitly redesigned. Automatic provider requeue/retry execution and user-facing overrides must remain visible, timeout-separated, and isolated from normal user session concurrency.

**Uncertainty**: The design implies the coordinator itself should batch, but actual subagent launch goes through `task_tool` invoked by the orchestrator LLM. Current implementation chooses tool-level local-cap waiting as the minimal smoother path. Deterministic backend batching remains a separate follow-up.

**Verification**: `cargo test -p bitfun-core deep_review -- --nocapture` + `cargo test -p bitfun-core coordination -- --nocapture`

---

### P2-3: Backend Retry Budget And Structured Retry Admission

**Design ref**: Section 1.5

**What**: Track retry budget, return retry guidance when a reviewer Task returns `partial_timeout`, and reject unsafe retry reviewer Tasks unless they include structured coverage, reduced scope, retryable source status, and lower timeout. Accepted retry Tasks also prepend a bounded retry-scope block to the reviewer prompt. Automatic backend redispatch with reduced scope and downgraded strategy is not implemented.

**Files**:
- `src/crates/core/src/agentic/deep_review_policy.rs` - `retries_used` tracking (already done)
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs` - retry guidance, budget checks, and structured retry admission
- `src/crates/core/src/agentic/agents/prompts/deep_review_agent.md` - already has retry instructions

**Design spec**:
1. Check `retries_used[role] < max_retries_per_role`
2. Re-dispatch with: reduced scope (only unreviewed files), timeout / 2, strategy downgraded one level
3. Increment `retries_used[role]`
4. Set `is_retry: true` on the retry Task call and include structured `retry_coverage`

**Risk**: Low-Medium. The tracking structures and retry admission gate are already in place. The remaining risk is model dependence: the orchestrator must read the guidance and explicitly issue a retry Task.

**Uncertainty**: "Reduced scope (only files not yet reviewed)" requires knowing which files were already covered by the partial output. TaskTool now requires explicit structured coverage before accepting a retry and injects the accepted retry scope into the reviewer prompt, but it does not infer coverage from free-form partial output or launch the retry by itself. Until backend-owned coverage extraction and redispatch exist, this remains prompt-guided retry with deterministic admission.

**Verification**: `cargo test -p bitfun-core deep_review -- --nocapture`

---

### P2-4: Backend Incremental Review Cache Primitives

**Design ref**: Part 5, "Advanced (Lower Priority)" item 14

**What**: Provide the cache data model, session metadata field, TaskTool cache-hit read path, completed-reviewer write-through, and packet-id key alignment. Cross-session/project-level cache reuse remains deferred.

**Files**:
- `src/crates/core/src/agentic/session/session_manager.rs` - cache storage (in session metadata)
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs` - cache read before dispatch
- `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs` - completed reviewer output write-through and cache hit/miss report signals

**Design spec**:
- Cache key: `incremental-review:{fingerprint}` (already computed in frontend)
- Store: completed reviewer outputs keyed by `packet_id`
- Invalidation: `target_file_set_changed`, `reviewer_roster_changed`, `strategy_changed` (already listed in frontend plan)
- On cache hit: skip dispatch for cached packets, inject cached output into the judge's context

**Risk**: Medium. Cache invalidation correctness is critical because stale cache produces wrong reviews. The implemented scope stays per-session, aligns read/write on `packet_id`, and invalidates by fingerprint; project-level persistence is intentionally deferred.

**Approach**: Store cache in `SessionMetadata` first. On `buildEffectiveReviewTeamManifest`, the frontend computes the fingerprint. The backend can read matching cache data and skip matching packets only after completed reviewer outputs are written using the same `packet_id` keys that work packets use.

**Uncertainty**: Cache storage location. Session metadata is per-session, but incremental review spans sessions. Need to decide: store in project-level storage (`<project>/.bitfun/review-cache/`) or in the previous session's metadata? Project-level storage is more natural for cross-session reuse but requires a new storage path.

**Decision needed**: Cache persistence scope: per-session (simpler, only works within continuation) vs. per-project (cross-session, requires new storage). The addendum keeps per-session as the initial closure target.

**Verification**: `cargo test -p bitfun-core deep_review -- --nocapture`

---

### P2-5: Backend Shared Context Cache

**Design ref**: Part 5, "Advanced (Lower Priority)" item 13

**What**: When multiple reviewers need to read the same file, cache the first read's result and reuse it for subsequent reviewers.

**Files**:
- `src/crates/core/src/agentic/coordination/coordinator.rs` - shared context cache during subagent execution
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs` - inject cache context into subagent sessions

**Risk**: High. This requires intercepting tool calls (Read, GetFileDiff) within subagent sessions and caching their results. This is a deep architectural change to the tool pipeline.

**Approach**: The prompt already instructs reviewers to "reuse read-only context by cache_key". For initial implementation, the prompt-level instruction (already emitted) is the primary mechanism. Local runtime measurement now records duplicate reviewer `Read`/`GetFileDiff` calls by parent turn, reviewer type, tool name, and normalized file path only. Final Deep Review submission emits aggregate debug diagnostics with counts only, not file content, diffs, or tool outputs. Programmatic enforcement would require a tool-call interception layer and should not be implemented until measurement shows material duplicate IO/token cost.

**Recommendation**: **Defer programmatic enforcement to a later phase pending measurement.** The prompt rules are already comprehensive and the LLM can follow them. The return-on-investment for programmatic enforcement must be proven against duplicate-call measurements because interception is a deep tool-pipeline change.

**Verification**: Manual testing with `cargo build -p bitfun-desktop` + deep review on a multi-reviewer change.

---

### P2-6: Backend Token Budget Enforcement Boundary

**Design ref**: Part 5, "Advanced (Lower Priority)"

**What**: Enforce the low-risk max-file/file-splitting boundary first, then add heuristic prompt-byte estimation that can trigger summary-first orientation without clipping the assigned file scope.

**Files**:
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs` - scope truncation
- `src/crates/core/src/agentic/deep_review_policy.rs` - budget policy parsing

**Risk**: Medium. `maxFilesPerReviewer` enforcement requires splitting file lists passed to subagent Tasks. `maxPromptBytesPerReviewer` cannot be byte-accurate without generating the final prompt, so the implemented boundary is a manifest heuristic that chooses summary-first orientation while preserving full file visibility.

**Approach**:
1. `maxFilesPerReviewer`: Split reviewer work packets by file group, without silently dropping files.
2. `largeDiffSummaryFirst`: Enable only when the estimated reviewer prompt bytes exceed the configured threshold, and use the pre-generated diff summary for orientation while keeping `assigned_scope.files` intact.
3. `maxPromptBytesPerReviewer`: Record the selected threshold and heuristic estimate in the manifest; defer byte-accurate hard clipping.

**Recommendation**: Treat heuristic estimate plus full-scope summary-first metadata as the production boundary for this phase. Hard prompt clipping and mandatory generated summaries remain deferred until prompt-size measurement is more precise.

**Verification**: `cargo test -p bitfun-core deep_review -- --nocapture`

---

### P2-7: Pre-Review Summary UI Display (Optional)

**Design ref**: Part 5

**What**: Show a compact pre-review summary in the launch confirmation dialog before starting the review. The shipped surface includes file count, risk areas, selected strategy, optional-reviewer count, summary-first state, and skipped-reviewer warnings.

**Files**:
- `src/web-ui/src/flow_chat/components/DeepReviewConsentDialog.tsx` - compact summary rows in the launch dialog
- `src/web-ui/src/locales/*/flow-chat.json` - localized labels for the compact summary

**Risk**: Low. Purely additive UI. The data is already computed in `buildPreReviewSummary()`.

**Decision**: Use the existing launch confirmation dialog. A Review Team page card or inline Flow Chat preview remains deferred to avoid adding another dense Deep Review surface.

**Verification**: `pnpm run lint:web && pnpm run type-check:web && pnpm --dir src/web-ui run test:run`

---

## Current Execution Order

```
Phase A (backend policy foundation): DONE
  P2-1: ChangeRiskFactors + auto_select_strategy pure helper
  P2-1b: advisory strategy-decision metadata with mismatch-warning authority
  P2-6: max-file/file-split guardrails plus heuristic prompt-byte summary-first metadata

Phase B (backend dispatch enforcement): PARTIAL
  P2-2: ConcurrencyPolicy + bounded local-cap waiting + explicit provider transient-capacity skip conversion + turn-local effective learning is done; automatic provider/adaptive queueing and staggered backend dispatch are deferred
  P2-3: Retry budget + guidance + structured retry admission + bounded retry prompt injection are done; backend-owned redispatch is deferred

Phase C (backend caching): DONE for per-session scope
  P2-4: Cache primitives, packet-id read/write path, and hit/miss reporting are done; project-level persistence is deferred

Phase D (optional/lower priority): PARTIAL
  P2-7: Compact pre-review consent summary is done; separate dense preview remains deferred
  P2-5: Shared context cache
  P2-6 follow-up: hard prompt-byte clipping and byte-accurate enforcement
```

## Historical Execution Order (Superseded Original Labels)

```
These labels are retained only to explain the original plan shape. They are not
current completion truth; use "Current Execution Order" and the addendum status
table above instead.

Phase A (Backend policy foundation): original target
  P2-1: ChangeRiskFactors + auto_select_strategy
  P2-6: Token Budget - maxFilesPerReviewer only

Phase B (Backend dispatch enforcement): original target, now partially scoped
  P2-2: ConcurrencyPolicy + original backend batching target (current runtime has bounded local-cap waiting and launch-batch overlap protection; full backend batch/stagger scheduling is deferred)
  P2-3: Original retry execution target (current runtime has structured retry admission; backend-owned redispatch is deferred)

Phase C (Backend caching - higher risk): original target
  P2-4: Incremental review cache (current runtime is per-session only; project-level persistence is product-decision-required)

Phase D (Optional / lower priority):
  P2-7: Pre-review summary UI (current runtime has compact consent summary)
  P2-5: Shared context cache (current runtime has prompt rules plus duplicate-call measurement; result reuse is deferred)
```

## Implementation Summary

### Changes Made

| File | Changes |
|---|---|
| `deep_review_policy.rs` | `ChangeRiskFactors` struct, `auto_select_strategy()`, `DeepReviewConcurrencyPolicy` struct + `from_manifest()` + `effective_max_same_role_instances()` + `check_launch_allowed()`, `DeepReviewIncrementalCache` struct + `from_value()`/`to_value()`/`matches_manifest()`, `deep_review_active_reviewer_count()` / `deep_review_has_judge_been_launched()` / `deep_review_retries_used()` / `deep_review_max_retries_per_role()` / cap-rejection tracking free functions, and shared-context duplicate measurement snapshots |
| `framework.rs` / `tool_pipeline.rs` / `code_review_tool.rs` | DeepReview reviewer `Read`/`GetFileDiff` duplicate measurement, parent-turn context propagation, and aggregate report-submission debug diagnostics without storing source/diff/tool-result content |
| `task_tool.rs` | Concurrency policy cap enforcement before subagent launch, cap-rejection runtime tracking, incremental cache hit check by resolved packet id when matching cache data is present, retry guidance hint on partial_timeout, structured retry admission, bounded retry-scope prompt injection, and DeepReview reviewer context tagging |
| `code_review_tool.rs` | Runtime reliability signal filling for cap rejections, cache hit/miss reporting, partial reviewer status, retry guidance, skipped reviewers, and token-budget tradeoffs |
| `reviewTargetClassifier.ts` | Path-domain classification plus reviewer applicability registry for conditional reviewer activation |
| `session/types.rs` | `deep_review_cache: Option<Value>` field on `SessionMetadata` |
| `persistence/manager.rs` | Preserve `deep_review_cache` when loading existing session metadata |
| `coordinator.rs` | Initialize `deep_review_cache: None` for new subagent sessions |
| `deep-review-design.md` | "Implementation Additions" section (ContextHealthSnapshot, ModelCapabilityProfile, Extended Path Classification), reconciled "Remaining / Future Work" |

### Known Semantic Gaps

| Area | Current behavior | Follow-up needed |
|---|---|---|
| Strategy authority | Manifest metadata records frontend recommendation, backend-compatible recommendation, user override, final strategy, mismatch state, and severity | Measured complexity delta only; backend scoring remains advisory and must not expand reviewer roster or override user/team strategy |
| Batched dispatch | Tool-level local-cap waiting handles reviewer saturation, and launch-batch overlap protection queues later batches while earlier batches are active. Full backend batch ordering and `staggerSeconds` remain prompt-guided | Add deterministic backend batch/stagger scheduling only if prompt-guided ordering remains unreliable |
| Retry | Budget, guidance, structured retry admission, and bounded retry-scope prompt injection are emitted/enforced; guidance uses effective manifest policy when available | Backend-owned automatic reduced-scope redispatch, or keep the current prompt-guided status wording |
| Pre-review summary UI | Compact launch summary shows file count, risk areas, selected strategy, optional-reviewer count, summary-first marker, and skipped-reviewer warnings | Separate dense pre-review report only if product later needs it |
| Incremental cache | Per-session data model, metadata field, packet-id read/write path, and hit/miss reporting exist | Project-level persistence and retention/privacy policy only |
| Token budget | File-split/max-file guardrails, heuristic prompt-byte estimates, full-scope summary-first metadata, and context-pressure warnings exist | Hard prompt-byte clipping and byte-accurate enforcement only; summary-first must keep assigned files visible |
| Shared context cache | Prompt rules plus local duplicate Read/GetFileDiff measurement and aggregate debug diagnostics | Tool-result interception only if real-run measurement shows material duplicate IO/token cost |
| Cost-aware strategy depth | Strategy metadata and token-budget hints exist, but quick/default reviewer prompts may still perform broad discovery on large changes | Add a manifest depth profile so quick means high-risk-only, normal means risk-expanded, and deep means full-depth |
| Shared evidence pack | Duplicate-tool measurement exists, but reviewers still rediscover changed files, hunk locations, risk tags, and cheap contract facts independently | Precompute compact source-agnostic evidence once per run before considering programmatic `Read`/`GetFileDiff` result reuse |
| Observability | Report reliability signals cover cache hit/miss, concurrency cap rejection, partial timeout, retry guidance, skipped reviewers, and token-budget tradeoffs | External telemetry/dashboard metrics only if needed later |

### Latest Release-Gate Verification

Post-Round 11b reconciliation re-ran the current release gate after document/code review. The live progress ledger remains `docs/deep-review-phase2-addendum.md`.

| Check | Result |
|---|---|
| `cargo test -p bitfun-core deep_review -- --nocapture` | 105 passed, 0 failed |
| `cargo check --workspace --exclude bitfun-cli` | Pass (warnings only, pre-existing) |
| `pnpm run lint:web` | Pass |
| `pnpm run type-check:web` | Pass |
| `pnpm --dir src/web-ui run test:run` | 67 files passed, 391 tests passed |
| `git diff --check` | Pass |

## Decisions Remaining After Status Reconciliation

1. **P2-1 strategy authority**: Backend-compatible scoring is now a mismatch-warning advisory signal. The final strategy remains the configured team strategy or explicit user override, and advisory mismatch metadata must not expand reviewer roster or silently change token/concurrency cost.
   - **Recommendation**: Keep this as the production boundary unless a future product decision explicitly asks for authoritative auto-selection. Add measured complexity delta before giving this policy any stronger authority.

2. **P2-2 batching approach**: Should the next step stay with tool-level local-cap waiting, or add broader provider/adaptive scheduling?
   - **Recommendation**: Keep local-cap waiting, explicit provider transient-capacity skip conversion, and turn-local effective-cap learning as the production boundary for this phase. Add automatic provider/adaptive queueing only as a narrow follow-up with visible backend-bound controls, queue-time/execution-time separation, user override bounds, and protection for normal user session concurrency.

3. **P2-4 cache persistence scope**: Per-session is implemented. Per-project reuse remains a future decision because it needs retention/privacy, invalidation, deletion, and user-visibility boundaries.
   - **Recommendation**: Keep per-session as the production boundary for this phase. Cached reviewer outputs have no independent retention period beyond session metadata; add per-project only after explicit product approval and deletion semantics.

4. **P2-5 shared context cache**: Accept prompt-only approach for now, or invest in programmatic enforcement?
   - **Recommendation**: Prompt-only plus local duplicate measurement and aggregate debug diagnostics for this phase. The prompt rules are already emitted and comprehensive; programmatic enforcement remains deferred until real-run measurement data justifies a separate interception/cache plan.

5. **P2-6 token budget scope**: The current production boundary is heuristic prompt-byte estimation plus full-scope summary-first metadata. Should future work add hard prompt clipping?
   - **Recommendation**: Keep hard clipping deferred until byte-accurate prompt measurement exists, and never hide files without explicit coverage metadata.

6. **P2-7 pre-review summary UI**: Where to display?
   - **Decision**: Use the existing launch confirmation dialog for a compact summary. Defer separate Review Team page or inline Flow Chat preview surfaces until product needs a denser pre-review view.

7. **Cost-aware review depth**: Should quick/default reduce depth automatically for large changes?
   - **Recommendation**: Yes, but only through explicit scope wording and coverage metadata. Quick should be a high-risk gate, normal should be risk-expanded, and deep should remain full-depth. Reduced-depth modes must not hide changed files or suppress severe categories such as security, persistence, auth, concurrency, data loss, migrations, cross-boundary APIs, and i18n contract drift.

8. **Shared evidence before shared tool cache**: Should the runtime cache full tool results across parallel reviewers now?
   - **Recommendation**: Not yet. First add a shared evidence pack with changed files, hunk hints, risk/domain tags, packet ids, and cheap contract hints. Keep programmatic result reuse limited to immutable diff/stat data until aggregate duplicate-call measurements prove that full tool-result interception is worth the complexity.

## Verification Commands

| Phase | Command |
|---|---|
| Phase A | `cargo test -p bitfun-core deep_review -- --nocapture` |
| Phase B | `cargo test -p bitfun-core deep_review -- --nocapture && cargo test -p bitfun-core coordination -- --nocapture` |
| Phase C | `cargo test -p bitfun-core deep_review -- --nocapture` |
| All phases (frontend) | `pnpm run lint:web && pnpm run type-check:web && pnpm --dir src/web-ui run test:run` |
| All phases (full Rust) | `cargo check --workspace && cargo test --workspace` |
| Integration smoke | `cargo build -p bitfun-desktop` + manual deep review |

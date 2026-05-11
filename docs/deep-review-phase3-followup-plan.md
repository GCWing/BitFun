# Deep Review Phase 3 Follow-up Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the remaining Deep Review follow-ups with low-noise diagnostics, visible capacity handling, bounded retry controls, and no project-level cache expansion.

**Architecture:** Keep Deep Review as a prompt-driven orchestrator with deterministic runtime guardrails. Add only narrow runtime automation where the user can see, pause, retry, or disable it. Do not introduce a full backend DAG scheduler, project-wide review cache, or hidden concurrency escalation.

**Tech Stack:** Rust core (`bitfun-core`), events (`bitfun-events`), desktop API, shared React frontend (`src/web-ui`), Vitest, Cargo tests.

---

## Confirmed Product Decisions

1. **Provider transient capacity queue is allowed** when the error is narrowly classified as rate limit, provider concurrency limit, `Retry-After`, or temporary overload.
2. **Automatic reviewer retry stays manual by default.** Add an explicit retry action for unresolved bounded slices. The user can choose "allow bounded retries without asking again"; after that, Deep Review may run small automatic retries within the configured retry budget and must never form an infinite loop.
3. **Project-level review cache remains deferred.** Current per-session cache behavior stays the production boundary.

## Non-Negotiable UX Constraints

- No hidden long wait. Provider or local capacity queueing must be visible, pauseable, cancellable, and timeout-separated from reviewer execution.
- No automatic increase of maximum concurrency. The runtime may lower effective concurrency temporarily, but increasing configured concurrency requires explicit user action.
- No noisy diagnostics. Runtime metrics must be aggregate, per-turn, and logged or surfaced at completion only unless they drive an existing compact queue notice.
- No source, diff, or model output content in diagnostics. Store counts, durations, reason categories, and reviewer ids only.
- No disruptive settings expansion. Prefer adding a small "Review capacity and retry" subsection to existing Review settings over creating a new surface.

## Current Instrumentation Audit

| Signal | Current coverage | Current risk | Phase 3 action |
|---|---|---|---|
| Duplicate `Read` / `GetFileDiff` usage | Present. `Tool::call` records only DeepReview reviewer `Read` and `GetFileDiff` calls by parent turn, reviewer type, tool name, and normalized path. `submit_code_review` logs aggregate debug diagnostics once. | Per-tool-call in-memory update is acceptable, but it does not produce a durable product decision snapshot. | Keep the current low-content measurement. Add a final aggregate diagnostics object only when a run completes. |
| Local capacity queue wait | Present. Queue state events include status, reason, queue elapsed, run elapsed, effective cap, and max queue wait. | UI events are emitted while waiting; this is needed for the live queue notice but should not become per-event logging. | Keep live events for UI. Add completion-time aggregate counters instead of logging every event. |
| Provider transient capacity failure | Present as short visible provider queueing, one bounded reattempt, final `capacity_skipped` with effective-cap learning, and report reliability folding. | It is still not a general provider/adaptive scheduler and does not batch/stagger all launches. | Keep it Deep Review-scoped and maintain aggregate diagnostics plus visible controls. |
| Concurrency-limited report signal | Present as a final `concurrency_limited` reliability signal. | It explains that concurrency limited coverage, but does not guide the user to a safer next run setting. | Add action-bar/report CTA to run slower next time or open Review settings. |
| Retry guidance | Present as assistant-facing retry guidance, TaskTool structured retry admission, explicit manual retry action, persisted bounded-auto-retry setting, and guarded `auto_retry` admission. | Backend-owned automatic redispatch scheduling is still not implemented. | Keep default manual behavior and require opt-in plus runtime guards for any backend-owned auto retry caller. |
| Queue control actions | Present for local-cap waits: pause, continue, cancel, skip optional. | Provider queue must reuse the same visible control model. | Extend queue state reason/control handling to provider capacity waits. |
| Token/runtime cost of retry | Partially controlled by `max_retries_per_role`, reduced scope, and lower timeout admission. | A future auto retry could extend review duration unexpectedly. | Add per-run retry elapsed guard, retry count display, and stop after one bounded retry per packet unless the configured budget explicitly allows more. |
| Project-level cache metrics | Not needed for current boundary. | A cache plan could accidentally imply persistence approval. | Keep project-level cache out of scope. |

## Recommended User Experience

### Capacity Problem Guidance

When Deep Review sees repeated capacity pressure or a provider transient capacity error:

- Show a compact action-bar notice: "Review capacity is constrained. Queue time does not count against reviewer runtime."
- Offer actions:
  - `Wait briefly` / `Continue queue`
  - `Pause Deep Review`
  - `Cancel queued reviewers`
  - `Skip optional extras`
  - `Run slower next time`
  - `Open Review settings`
- `Run slower next time` should lower Deep Review's configured max parallel reviewers by one, bounded to at least one. It must be explicit and reversible in settings.
- If the target session is busy, prefer "Pause Deep Review" over consuming more subagent capacity.

This is better than only telling users to edit a numeric setting because it keeps the recovery path local to the failure, while still exposing the persistent setting for users who want control.

### Retry Experience

When a reviewer ends as `partial_timeout` or `capacity_skipped` and has structured coverage:

- Show a `Retry unresolved slice` action near the report/action bar.
- The first click retries only the uncovered files with a lower timeout.
- Offer an inline checkbox or secondary action: `Allow bounded automatic retries for future Deep Reviews`.
- Persist the preference in Review settings and allow the user to turn it off.
- Auto retry must stop when any of these is true:
  - retry budget for the role or packet is exhausted;
  - retry scope is not smaller than the source packet;
  - retry source status is not `partial_timeout` or transient `capacity_skipped`;
  - overall Deep Review elapsed guard is exceeded;
  - retry fails with auth, quota, billing, invalid model, policy, invalid tooling, user cancellation, or validation error.

### Cost And Scope Experience

Large changes should not force every strategy to perform a full-depth audit. The product should make the selected strategy's review depth explicit and should prefer a fast, high-risk pass before spending time and tokens on broad exploration.

- `quick`: run a high-risk gate. Review only changed hunks, direct contract/security/config/concurrency paths, and required locale/API consistency checks. Optional reviewers and broad dependency exploration stay off unless the risk classifier marks a matching area.
- `normal`: run a risk-expanded review. Start from changed hunks, include one-hop dependencies only for high-risk domains, and keep optional specialists conditional. This should be the default balance for most product changes.
- `deep`: run full breadth and depth. Permit wider dependency tracing, more reviewer packets, and deeper role-specific exploration when the user explicitly chooses thoroughness.
- For slow models or very large diffs, offer a compact `Fast high-risk scan first` path and a later `Deepen selected areas` follow-up instead of silently widening the first run.
- The launch summary should describe the depth as `High-risk only`, `Risk-expanded`, or `Full-depth` rather than showing dense token estimates.

This approach reduces negative user impact because the default path still finds severe issues first, while users can deliberately pay for deeper coverage when needed.

## Data Model Additions

### Per-Turn Runtime Diagnostics

Add a small aggregate diagnostics structure under the Deep Review runtime path. It should be in-memory during the turn and optionally attached to final report metadata.

Fields:

- `queue_wait_count`
- `queue_wait_total_ms`
- `queue_wait_max_ms`
- `provider_capacity_queue_count`
- `provider_capacity_retry_count`
- `provider_capacity_retry_success_count`
- `capacity_skip_count`
- `effective_parallel_min`
- `effective_parallel_final`
- `manual_queue_action_count`
- `manual_retry_count`
- `auto_retry_count`
- `auto_retry_suppressed_reason_counts`
- `shared_context_total_calls`
- `shared_context_duplicate_calls`
- `shared_context_duplicate_context_count`

Rules:

- Update counters in memory only at state transitions or final submission.
- Emit at most one debug log line at final `submit_code_review`.
- Attach only aggregate fields to report metadata; do not render them by default unless they affect coverage reliability.
- Do not store source text, diff text, reviewer output, provider raw body, or full file contents.

### Review Settings Additions

Persist these under the default review team config, not as global session behavior:

- `max_parallel_reviewers`: default `4`, min `1`, max `16`.
- `max_queue_wait_seconds`: default `60`, min `0`, max `600`.
- `allow_provider_capacity_queue`: default `true`.
- `allow_bounded_auto_retry`: default `false`.
- `auto_retry_elapsed_guard_seconds`: default `180`, min `30`, max `900`.

Global `ai.subagent_max_concurrency` already exists in Rust config and affects all subagent use. It should not be the primary user-facing control for Deep Review because it can affect normal sessions. If exposed in the UI later, label it as an advanced global setting.

### Cost-Aware Scope Profile

Add a manifest-level scope profile that controls review depth without changing the configured team roster:

- `review_depth`: `high_risk_only | risk_expanded | full_depth`
- `risk_focus_tags`: stable tags such as `security`, `api_contract`, `data_loss`, `concurrency`, `persistence`, `i18n`, `frontend_platform_boundary`, `cross_crate`, and `generated_or_low_risk`
- `max_dependency_hops`: `0` for quick, `1` for normal high-risk paths, and `unbounded_or_policy_limited` for deep
- `optional_reviewer_policy`: `risk_matched_only | configured | full`
- `allow_broad_tool_exploration`: false for quick, limited for normal, true for deep
- `coverage_expectation`: a short string for the judge/report so reduced-depth reviews cannot be mistaken for full coverage

Rules:

- The profile may shrink reviewer depth and optional reviewer activation, but it must not hide changed files from coverage metadata.
- Quick/default high-risk behavior should be fail-open for severe categories: security, data loss, migrations, auth, cross-boundary API, concurrency, and persistence changes remain in scope.
- If a reviewer skips broad exploration because of the profile, the judge must preserve that as an explicit coverage limitation.
- User-selected `deep` overrides reduced-depth defaults.

### Shared Evidence Pack

Create a source-agnostic evidence pack once before subagent launch so each reviewer starts from the same compact facts instead of rediscovering them through repeated tools.

Initial fields:

- changed file list, diff stat, and domain tags
- per-file hunk ranges and changed symbols when available
- risk focus tags and strategy depth profile
- review packet ids and assigned scope
- relevant git metadata such as base/head refs or source label when available
- compact contract hints, such as changed Tauri command names or locale key names, when cheaply derivable

Rules:

- The evidence pack should prefer metadata, hunk ranges, and short summaries over full file contents.
- It must remain source-agnostic: Git is one source, not the abstraction.
- Subagents should read full files only to confirm a suspected issue or inspect a specific missing context.
- Start with manifest/prompt consumption plus current duplicate-tool diagnostics. Programmatic tool-result reuse should be limited to immutable, content-addressed `GetFileDiff`/diff-stat results first, and only after measured duplicate cost justifies it.
- Do not store source text, full diffs, or reviewer output in shared diagnostics.

## Implementation Rounds

### Round 1: Low-Frequency Runtime Diagnostics

**Goal:** Confirm real run behavior without adding hot-path logging or UI noise.

**Status:** Implemented for local queue terminal waits, provider capacity queue/retry/success counts, capacity skips, auto-retry suppression counters, effective concurrency transitions, and shared-context reuse measurements.

**Files:**

- Modify: `src/crates/core/src/agentic/deep_review_policy.rs`
- Modify: `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- Modify: `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs`
- Test: `src/crates/core/src/agentic/deep_review_policy.rs`
- Test: `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs`

Steps:

- [x] Add a per-turn `DeepReviewRuntimeDiagnostics` aggregate in the existing budget tracker.
- [x] Record only current state transitions: local queue terminal wait, capacity skipped, and effective concurrency changes.
- [x] Keep provider capacity retry, retry accepted, and retry suppressed counters as aggregate-only fields.
- [x] Merge current shared-context measurement snapshot into the aggregate at final submission.
- [x] Log one debug line at final `submit_code_review` when diagnostics are non-empty.
- [x] Add tests proving duplicate `Read` / `GetFileDiff` counts remain content-free.
- [x] Add tests proving repeated queue events do not create repeated final diagnostics rows.

Verification:

- `cargo test -p bitfun-core deep_review_shared_context_diagnostics_stays_out_of_report -- --nocapture`
- `cargo test -p bitfun-core deep_review -- --nocapture`

Exit criteria:

- Diagnostics are aggregate-only.
- No per-second queue event is written as a log line.
- Report UI remains unchanged unless a reliability signal already exists.

### Round 2: Settings And Capacity Guidance

**Goal:** Give users a clear recovery path without automatic max-concurrency changes.

**Status:** Implemented for persisted default Review Team capacity/retry settings, the compact Review settings subsection, the capacity-queue recovery actions `Run slower next time` and `Open Review settings`, provider transient queueing, explicit manual retry, and guarded `auto_retry` admission. No Rust global config schema change was required because these fields are scoped to the default Review Team config.

**Files:**

- Modify: `src/web-ui/src/shared/services/reviewTeamService.ts`
- Modify: `src/web-ui/src/infrastructure/config/components/ReviewConfig.tsx`
- Modify: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx`
- Modify: `src/web-ui/src/app/scenes/agents/components/ReviewTeamPage.tsx`
- Modify: `src/web-ui/src/locales/en-US/settings/review.json`
- Modify: `src/web-ui/src/locales/zh-CN/settings/review.json`
- Modify: `src/web-ui/src/locales/zh-TW/settings/review.json`
- Modify: `src/web-ui/src/locales/en-US/flow-chat.json`
- Modify: `src/web-ui/src/locales/zh-CN/flow-chat.json`
- Modify: `src/web-ui/src/locales/zh-TW/flow-chat.json`
- Test: `src/web-ui/src/shared/services/reviewTeamService.test.ts`
- Test: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.test.tsx`
- Test: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.i18n.test.ts`

Steps:

- [x] Persist Deep Review concurrency settings under the default review team config.
- [x] Keep existing defaults unchanged: 4 parallel reviewers, 60s max queue wait.
- [x] Add a compact Review settings subsection named "Capacity and retry".
- [x] Add `max_parallel_reviewers` and `max_queue_wait_seconds` controls with bounds and localized descriptions.
- [x] Add `allow_provider_capacity_queue` and `allow_bounded_auto_retry` toggles.
- [x] Add a `Run slower next time` action path from capacity-limited UI/report to lower `max_parallel_reviewers` by one.
- [x] Keep global `ai.subagent_max_concurrency` out of the normal Review settings path.

Verification:

- `pnpm run lint:web`
- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts`

Exit criteria:

- Users can recover from capacity pressure without editing raw config.
- Settings changes are explicit and reversible.
- Existing Review Team strategy/model behavior is unchanged.

### Round 3: Short Provider Capacity Queue

**Goal:** Automatically wait briefly for provider transient capacity without hiding the wait or stealing normal session capacity.

**Files:**

- Modify: `src/crates/core/src/agentic/deep_review_policy.rs`
- Modify: `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- Modify: `src/crates/events/src/agentic.rs`
- Modify: `src/crates/core/src/agentic/events/types.rs`
- Modify: `src/web-ui/src/flow_chat/utils/deepReviewQueueStateEvents.ts`
- Modify: `src/web-ui/src/flow_chat/store/deepReviewActionBarStore.ts`
- Modify: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx`
- Test: `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- Test: `src/web-ui/src/flow_chat/utils/deepReviewQueueStateEvents.test.ts`
- Test: `src/web-ui/src/flow_chat/store/deepReviewActionBarStore.test.ts`

Steps:

- [x] Treat only provider rate limit, provider concurrency limit, `Retry-After`, and temporary overload as provider-queueable.
- [x] Before returning `capacity_skipped`, perform a short queue wait bounded by `min(Retry-After, max_queue_wait_seconds)`.
- [x] Re-attempt the reviewer once after the short wait if the user has not paused/cancelled the queue.
- [x] Emit the existing queue state event with provider-specific reason and aggregate diagnostics counters.
- [x] Keep queue time separate from reviewer runtime timeout.
- [x] If the short provider queue expires, return `capacity_skipped` with the same reliability signal used today.
- [x] Fail fast for auth, quota, billing, invalid model, policy, invalid tooling, validation, and cancellation.

Verification:

- `cargo test -p bitfun-core deep_review_provider_capacity_error_builds_capacity_skipped_payload_and_lowers_effective_cap -- --nocapture`
- Add and run focused tests for provider queue success, queue expiry, pause, cancel, and non-queueable provider errors.
- `pnpm --dir src/web-ui run test:run -- src/flow_chat/utils/deepReviewQueueStateEvents.test.ts src/flow_chat/store/deepReviewActionBarStore.test.ts`

Exit criteria:

- Provider queue is visible.
- Queue time does not count against reviewer timeout.
- Provider queue never loops indefinitely.

### Round 4: Explicit Retry Action And Bounded Auto-Retry Preference

**Goal:** Let users recover partial reviewers without extending reviews indefinitely.

**Status:** Implemented with guardrails for explicit manual retry and `auto_retry` admission. Backend-owned automatic redispatch scheduling remains deferred.

**Files:**

- Modify: `src/crates/core/src/agentic/deep_review_policy.rs`
- Modify: `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- Modify: `src/web-ui/src/flow_chat/utils/codeReviewReport.ts`
- Modify: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx`
- Modify: `src/web-ui/src/flow_chat/store/deepReviewActionBarStore.ts`
- Modify: `src/web-ui/src/flow_chat/services/DeepReviewService.ts`
- Modify: `src/web-ui/src/locales/en-US/flow-chat.json`
- Modify: `src/web-ui/src/locales/zh-CN/flow-chat.json`
- Modify: `src/web-ui/src/locales/zh-TW/flow-chat.json`
- Test: `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- Test: `src/web-ui/src/flow_chat/utils/codeReviewReport.test.ts`
- Test: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.test.tsx`

Steps:

- [x] Add report metadata parsing for retryable unresolved packets: source packet id, source status, covered files, unresolved files, retry timeout.
- [x] Show `Retry incomplete slices` only when the report has structured reduced retry coverage.
- [x] When clicked, launch a retry Task prompt with `retry: true`, reduced `retry_scope_files`, lower timeout, and source coverage metadata.
- [x] Persist bounded automatic retry preference to Review settings.
- [x] Admit `auto_retry` only when opt-in, structured coverage, reduced scope, lower timeout, retry budget, and elapsed guard all pass.
- [x] Record aggregate suppression reasons when `auto_retry` admission fails.
- [ ] Backend-owned automatic redispatch scheduling remains deferred.

Verification:

- `cargo test -p bitfun-core task_tool::tests::deep_review_retry -- --nocapture`
- `cargo test -p bitfun-core deep_review -- --nocapture`
- `pnpm --dir src/web-ui run test:run -- src/flow_chat/utils/codeReviewReport.test.ts src/flow_chat/components/btw/DeepReviewActionBar.test.tsx`

Exit criteria:

- Default behavior is manual retry.
- The "do not ask again" preference is explicit, reversible, and bounded.
- No retry loop can exceed configured role/packet budget or elapsed guard.

### Round 5: Cost-Aware Scope And Shared Evidence Planning

**Goal:** Reduce default review time and token use for large changes by narrowing quick/default review depth and precomputing shared evidence once.

**Files:**

- Modify: `src/web-ui/src/shared/services/reviewTeamService.ts`
- Modify: `src/web-ui/src/shared/services/reviewTeamService.test.ts`
- Modify: `src/crates/core/src/agentic/agents/prompts/deep_review_agent.md`
- Modify: `src/crates/core/src/agentic/agents/prompts/review_*_agent.md`
- Modify: `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs`
- Modify: `docs/deep-review-design.md`
- Modify: `docs/deep-review-phase2-plan.md`
- Modify: `docs/deep-review-phase2-addendum.md`
- Modify: `docs/deep-review-phase3-followup-plan.md`

Steps:

- [ ] Add `review_depth`, `risk_focus_tags`, `max_dependency_hops`, and `coverage_expectation` to the Deep Review manifest.
- [ ] Map `quick` to high-risk-only review, `normal` to risk-expanded review, and `deep` to full-depth review.
- [ ] Keep optional reviewers risk-matched in quick/default paths instead of running every configured extra reviewer by default.
- [ ] Add a compact shared evidence pack to the manifest with changed files, hunk ranges, domain tags, packet ids, and cheap contract hints.
- [ ] Update reviewer prompts so subagents start from the evidence pack and call `Read`/`GetFileDiff` only for confirmation or missing context.
- [ ] Update Judge/report wording so reduced-depth reviews are clearly marked as high-risk or risk-expanded coverage, not full coverage.
- [ ] Keep programmatic cross-subagent tool-result reuse deferred unless duplicate-tool diagnostics show material repeated `Read`/`GetFileDiff` cost.

Verification:

- `pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts`
- `cargo test -p bitfun-core deep_review -- --nocapture`
- Focused prompt/manifest snapshot tests if prompt packet generation changes.

Exit criteria:

- Quick/default reviews focus on high-risk items without silently dropping file coverage metadata.
- Deep reviews still provide full-depth behavior when explicitly selected.
- Subagents receive enough shared evidence to reduce discovery tool calls.
- The report distinguishes reduced-depth coverage from full-depth review.

### Round 6: Documentation Reconciliation And Product Risk Review

**Goal:** Keep docs aligned with code and leave deferred items explicit.

**Files:**

- Modify: `docs/deep-review-design.md`
- Modify: `docs/deep-review-phase2-plan.md`
- Modify: `docs/deep-review-phase2-addendum.md`
- Modify: `docs/deep-review-phase3-followup-plan.md`

Steps:

- [ ] Update status wording after each completed round.
- [ ] Mark provider short queue as runtime-complete only after tests prove visible bounded behavior.
- [x] Mark bounded auto retry admission as implemented with guardrails after setting and loop guards exist.
- [ ] Keep project-level cache as product-decision-required/deferred.
- [ ] Keep programmatic shared context cache deferred unless real diagnostics justify it.
- [ ] Keep cost-aware depth profiles explicit so quick/default reduced-depth behavior cannot be mistaken for full review.
- [ ] Add a short "measured outcome" section once real run data is sampled.

Verification:

- `rg -n "project-level cache.*implement[[:alpha:]]*ed|auto retry.*compl[[:alpha:]]*ete|provider/adaptive queue.*compl[[:alpha:]]*ete|hard prompt.*compl[[:alpha:]]*ete" docs/deep-review-design.md docs/deep-review-phase2-plan.md docs/deep-review-phase2-addendum.md docs/deep-review-phase3-followup-plan.md`
- Expected: no stale wording claims deferred work is complete.

Exit criteria:

- Docs distinguish implemented behavior from scoped follow-up.
- Remaining risks are named with an owner decision or an implementation round.

## Open Risks

| Risk | Why it matters | Mitigation in this plan |
|---|---|---|
| Diagnostics overhead grows | Per-tool or per-second logging can slow large reviews and create noisy logs. | Aggregate in memory; final debug line only; no content capture. |
| Provider queue feels like a hang | Automatic wait without explanation can look stuck. | Reuse compact queue notice and controls; bounded wait; queue time separated from runtime. |
| Users change the wrong concurrency setting | Global subagent concurrency affects normal sessions. | Prefer Deep Review max parallel reviewer setting; label global concurrency as advanced if exposed later. |
| Retry extends reviews too long | Auto retry can silently double runtime. | Manual by default; opt-in auto retry; role budget, packet budget, smaller scope, lower timeout, elapsed guard. |
| Auto retry repeats bad context | Retrying without coverage can repeat the same large scope. | Require structured coverage and smaller retry scope. |
| Capacity failures are misclassified | Queueing quota/auth/model errors wastes user time. | Keep classifier narrow and fail fast for non-transient categories. |
| Settings surface becomes dense | Review settings already controls strategy, models, members, execution. | Add one compact "Capacity and retry" section; no new page. |
| Project cache leaks sensitive findings | Reviewer outputs can contain code summaries and security findings. | Keep project-level cache deferred; no new storage path in Phase 3. |
| Reduced-depth review misses low-risk regressions | Quick/default paths may skip broad dependency exploration to save time and tokens. | Make depth explicit, preserve file coverage metadata, keep severe categories in scope, and offer `Deepen selected areas` when needed. |
| Evidence pack becomes stale or too heavy | Shared facts can drift from actual tool results or recreate the same token burden in the manifest. | Build the pack immediately before launch, keep it metadata-first, and require reviewers to confirm specific issues with targeted reads. |
| Tool-result reuse changes semantics | Reusing full `Read` results across isolated subagents could hide file changes or leak too much context. | Start with shared evidence and duplicate-call measurement; limit any future programmatic reuse to immutable diff/stat data first. |

## Full Release Gate

Run after all selected Phase 3 rounds are implemented:

- `cargo test -p bitfun-core deep_review -- --nocapture`
- `cargo check --workspace --exclude bitfun-cli`
- `pnpm run lint:web`
- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run test:run`
- `git diff --check`

Expected:

- Diagnostics are aggregate and low-frequency.
- Provider transient queue is short, visible, and bounded.
- Users have a clear path to lower Deep Review parallelism without touching global subagent capacity.
- Retry defaults to explicit user action; bounded auto retry exists only after opt-in.
- Quick/default reviews use high-risk or risk-expanded scope profiles, while deep remains full-depth.
- Shared evidence reduces repeated discovery work without storing source content in diagnostics.
- Project-level cache remains unimplemented and explicitly deferred.

# Deep Review Phase 3 Execution Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute the remaining Deep Review Phase 3 work from `docs/deep-review-phase3-followup-plan.md` with low-frequency diagnostics, visible capacity recovery, explicit retry controls, and no project-level cache expansion.

**Architecture:** Keep Deep Review orchestration in the existing review manifest, TaskTool, runtime policy, events, and Flow Chat action-bar paths. Add narrowly scoped runtime state and user controls instead of a new scheduler, hidden concurrency escalation, or persistent project review cache. Each round must leave the product in a working state with tests proving that queueing, retry, and settings behavior remain bounded.

**Tech Stack:** Rust core (`bitfun-core`), `bitfun-events`, shared React frontend (`src/web-ui`), i18next locale JSON, Vitest, Cargo tests.

---

## Source Of Truth

- Primary design document: `docs/deep-review-phase3-followup-plan.md`
- Related design documents:
  - `docs/deep-review-design.md`
  - `docs/deep-review-phase2-plan.md`
  - `docs/deep-review-phase2-addendum.md`

This plan intentionally does not implement project-level review cache, global automatic concurrency increase, a backend DAG scheduler, or high-frequency telemetry.

## Current Code Assumptions

- `src/crates/core/src/agentic/deep_review_policy.rs` owns Deep Review budget tracking, retry admission, effective concurrency state, capacity classification, and queue state payload types.
- `src/crates/core/src/agentic/tools/implementations/task_tool.rs` owns reviewer Task launch, queue wait behavior, retry coverage validation, provider capacity handling, and queue state event emission.
- `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs` owns final `submit_code_review` reliability folding and shared-context diagnostics logging.
- `src/web-ui/src/shared/services/reviewTeamService.ts` owns default Review Team config, manifest construction, execution policy persistence, and concurrency policy defaults.
- `src/web-ui/src/infrastructure/config/components/ReviewConfig.tsx` owns the Review settings UI.
- `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx` and `src/web-ui/src/flow_chat/store/deepReviewActionBarStore.ts` own live Deep Review queue and recovery controls.
- No `ReviewConfig.test.tsx` exists today, so Round 2 should add one only if component-level coverage is required after the service tests are in place.

## Execution Rules

- Do not stage unrelated untracked files.
- Keep all logs in English and without emoji.
- Use existing queue/action-bar UI patterns before adding new surfaces.
- Add locale entries for every new user-facing string in `en-US`, `zh-CN`, and `zh-TW`.
- Use aggregate counters and final snapshots only. Do not store source text, diff text, reviewer output, provider raw body, or full file contents in diagnostics.
- Queue time must not be counted as reviewer runtime timeout.
- Automatic retry is disabled by default and must stay bounded after the user enables it.
- Quick/default review paths should focus on high-risk coverage first; only `deep` should imply full-depth exploration across broad dependencies.
- Shared review evidence should be generated once and passed to reviewers as compact metadata before adding any deeper tool-result cache.
- After each round, run the listed focused verification and update the status wording in `docs/deep-review-phase3-followup-plan.md`.

## Commit Strategy

Use one commit per completed round unless two adjacent rounds are very small and verified together.

Suggested commit boundaries:

1. `feat(deep-review): add aggregate runtime diagnostics`
2. `feat(review-team): expose capacity and retry settings`
3. `feat(deep-review): queue transient provider capacity errors`
4. `feat(deep-review): add bounded retry controls`
5. `feat(deep-review): add cost-aware review scope`
6. `docs(deep-review): reconcile phase three status`

---

## Round 0: Preflight And Baseline

**Goal:** Prove the branch starts from a known state and prevent unrelated files from entering the work.

**Files:**

- Read: `docs/deep-review-phase3-followup-plan.md`
- Read: `src/crates/core/src/agentic/deep_review_policy.rs`
- Read: `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- Read: `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs`
- Read: `src/web-ui/src/shared/services/reviewTeamService.ts`
- Read: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx`

- [x] **Step 0.1: Confirm git scope**

Run:

```powershell
git status --short --branch
```

Expected:

- Current branch is the Deep Review feature branch.
- Pre-existing unrelated untracked files remain untracked.
- Only files from this plan are staged or committed during execution.

- [x] **Step 0.2: Run the smallest baseline tests**

Run:

```powershell
cargo test -p bitfun-core deep_review -- --nocapture
pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts src/flow_chat/store/deepReviewActionBarStore.test.ts src/flow_chat/utils/deepReviewQueueStateEvents.test.ts
```

Expected:

- Existing Deep Review Rust tests pass.
- Existing Review Team and Deep Review action-bar focused web tests pass.
- If an unrelated failure appears, capture the failing test name and do not mix that repair with Phase 3 implementation unless it blocks this work.

- [x] **Step 0.3: Confirm feature boundaries**

Run:

```powershell
rg -n "project-level cache|global subagent|max_parallel|auto retry|provider capacity|shared_context" docs/deep-review-phase3-followup-plan.md
```

Expected:

- Project-level cache is marked deferred.
- Global subagent concurrency is not the primary user-facing Deep Review setting.
- Provider transient queue and bounded retry remain the active Phase 3 scope.

---

## Round 1: Low-Frequency Runtime Diagnostics

**Goal:** Add aggregate runtime diagnostics for real-run analysis without hot-path logging or report noise.

**Files:**

- Modify: `src/crates/core/src/agentic/deep_review_policy.rs`
- Modify: `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- Modify: `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs`
- Test: `src/crates/core/src/agentic/deep_review_policy.rs`
- Test: `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs`
- Docs: `docs/deep-review-phase3-followup-plan.md`

- [x] **Step 1.1: Write diagnostics tracker tests first**

Add tests in `src/crates/core/src/agentic/deep_review_policy.rs` near the existing Deep Review budget tracker tests:

```rust
#[test]
fn runtime_diagnostics_records_queue_and_capacity_transitions_as_counts() {
    let tracker = DeepReviewBudgetTracker::new();

    tracker.record_runtime_queue_wait("turn-runtime", 1_250);
    tracker.record_runtime_queue_wait("turn-runtime", 2_500);
    tracker.record_runtime_capacity_skip(
        "turn-runtime",
        DeepReviewCapacityQueueReason::ProviderConcurrencyLimit,
    );

    let diagnostics = tracker
        .runtime_diagnostics_snapshot("turn-runtime")
        .expect("runtime diagnostics should exist");

    assert_eq!(diagnostics.queue_wait_count, 2);
    assert_eq!(diagnostics.queue_wait_total_ms, 3_750);
    assert_eq!(diagnostics.queue_wait_max_ms, 2_500);
    assert_eq!(diagnostics.capacity_skip_count, 1);
    assert_eq!(diagnostics.provider_capacity_queue_count, 0);
}

#[test]
fn runtime_diagnostics_merges_shared_context_without_content() {
    let tracker = DeepReviewBudgetTracker::new();

    tracker.record_shared_context_tool_use(
        "turn-runtime-shared",
        "ReviewSecurity",
        "Read",
        "src/lib.rs",
    );
    tracker.record_shared_context_tool_use(
        "turn-runtime-shared",
        "ReviewArchitecture",
        "Read",
        "src/lib.rs",
    );

    let diagnostics = tracker
        .runtime_diagnostics_snapshot("turn-runtime-shared")
        .expect("runtime diagnostics should exist");

    assert_eq!(diagnostics.shared_context_total_calls, 2);
    assert_eq!(diagnostics.shared_context_duplicate_context_count, 1);
    assert!(!format!("{diagnostics:?}").contains("fn "));
}
```

Run:

```powershell
cargo test -p bitfun-core runtime_diagnostics_ -- --nocapture
```

Expected:

- Fails because `DeepReviewRuntimeDiagnostics` and recorder methods do not exist yet.

- [x] **Step 1.2: Add the aggregate diagnostics type**

In `src/crates/core/src/agentic/deep_review_policy.rs`, add a serializable diagnostics struct near the effective concurrency structs:

```rust
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct DeepReviewRuntimeDiagnostics {
    pub queue_wait_count: usize,
    pub queue_wait_total_ms: u64,
    pub queue_wait_max_ms: u64,
    pub provider_capacity_queue_count: usize,
    pub provider_capacity_retry_count: usize,
    pub provider_capacity_retry_success_count: usize,
    pub capacity_skip_count: usize,
    pub effective_parallel_min: Option<usize>,
    pub effective_parallel_final: Option<usize>,
    pub manual_queue_action_count: usize,
    pub manual_retry_count: usize,
    pub auto_retry_count: usize,
    pub auto_retry_suppressed_reason_counts: BTreeMap<String, usize>,
    pub shared_context_total_calls: usize,
    pub shared_context_duplicate_calls: usize,
    pub shared_context_duplicate_context_count: usize,
}
```

Add `runtime_diagnostics: DeepReviewRuntimeDiagnostics` to the per-turn budget record. Use `BTreeMap` for deterministic test output.

- [x] **Step 1.3: Add recorder and snapshot methods**

In `DeepReviewBudgetTracker`, add methods with these names and behavior:

```rust
pub fn record_runtime_queue_wait(&self, parent_dialog_turn_id: &str, queue_elapsed_ms: u64)
pub fn record_runtime_provider_capacity_queue(&self, parent_dialog_turn_id: &str)
pub fn record_runtime_provider_capacity_retry(&self, parent_dialog_turn_id: &str)
pub fn record_runtime_provider_capacity_retry_success(&self, parent_dialog_turn_id: &str)
pub fn record_runtime_capacity_skip(
    &self,
    parent_dialog_turn_id: &str,
    reason: DeepReviewCapacityQueueReason,
)
pub fn record_runtime_manual_queue_action(&self, parent_dialog_turn_id: &str)
pub fn record_runtime_manual_retry(&self, parent_dialog_turn_id: &str)
pub fn record_runtime_auto_retry(&self, parent_dialog_turn_id: &str)
pub fn record_runtime_auto_retry_suppressed(
    &self,
    parent_dialog_turn_id: &str,
    reason: &str,
)
pub fn runtime_diagnostics_snapshot(
    &self,
    parent_dialog_turn_id: &str,
) -> Option<DeepReviewRuntimeDiagnostics>
```

Snapshot behavior:

- Merge current shared-context measurement at snapshot time.
- Return `None` when every counter and optional effective parallel field is empty.
- Do not include raw paths in the returned diagnostics.

- [x] **Step 1.4: Wire diagnostics at state transitions only**

Update `src/crates/core/src/agentic/tools/implementations/task_tool.rs`:

- When local queue emits a terminal wait result, record `record_deep_review_runtime_queue_wait`.
- When provider capacity is classified and skipped, record `record_deep_review_runtime_capacity_skip`.
- Do not record every one-second queue event as a log or diagnostics row.

Update `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs`:

- Replace shared-context-only final debug logging with a runtime diagnostics final snapshot.
- Keep the report payload free of `runtime_diagnostics` unless a later round explicitly needs it for a user-visible reliability signal.

- [x] **Step 1.5: Run focused verification**

Run:

```powershell
cargo test -p bitfun-core runtime_diagnostics_ -- --nocapture
cargo test -p bitfun-core deep_review_shared_context_diagnostics_stays_out_of_report -- --nocapture
cargo test -p bitfun-core deep_review -- --nocapture
```

Expected:

- Diagnostics tests pass.
- Shared-context diagnostics remain out of report content.
- Existing Deep Review tests pass.

- [x] **Step 1.6: Update Phase 3 status wording**

In `docs/deep-review-phase3-followup-plan.md`, mark Round 1 as implemented only after Step 1.5 passes. Keep provider queue, retry action, and settings work as pending.

- [ ] **Step 1.7: Commit Round 1**

Run:

```powershell
git add src/crates/core/src/agentic/deep_review_policy.rs src/crates/core/src/agentic/tools/implementations/task_tool.rs src/crates/core/src/agentic/tools/implementations/code_review_tool.rs docs/deep-review-phase3-followup-plan.md
git commit -m "feat(deep-review): add aggregate runtime diagnostics"
```

Expected:

- Commit includes only Round 1 files.

---

## Round 2: Review Capacity And Retry Settings

**Goal:** Give users explicit Deep Review capacity and retry controls without exposing global subagent concurrency as the normal path.

**Status:** Implemented and locally verified. The settings fit existing component styles without `ReviewConfig.scss` changes, and the persistent controls are scoped to the default Review Team config rather than Rust global config. Round 2 commit is intentionally still pending until requested.

**Files:**

- Modify: `src/web-ui/src/shared/services/reviewTeamService.ts`
- Modify: `src/web-ui/src/shared/services/reviewTeamService.test.ts`
- Modify: `src/web-ui/src/infrastructure/config/components/ReviewConfig.tsx`
- Modify: `src/web-ui/src/infrastructure/config/components/ReviewConfig.scss`
- Modify: `src/web-ui/src/locales/en-US/settings/review.json`
- Modify: `src/web-ui/src/locales/zh-CN/settings/review.json`
- Modify: `src/web-ui/src/locales/zh-TW/settings/review.json`
- Docs: `docs/deep-review-phase3-followup-plan.md`

- [x] **Step 2.1: Write service tests for persisted concurrency settings**

Add tests in `src/web-ui/src/shared/services/reviewTeamService.test.ts` covering:

```ts
it('loads default concurrency and retry settings when config is missing', async () => {
  vi.mocked(configAPI.getConfig).mockRejectedValueOnce(
    new Error("Config path 'ai.review_teams.default' not found"),
  );

  const team = await loadDefaultReviewTeam(WORKSPACE_PATH);

  expect(team.concurrencyPolicy).toEqual({
    maxParallelInstances: 4,
    staggerSeconds: 0,
    maxQueueWaitSeconds: 60,
    batchExtrasSeparately: true,
    allowProviderCapacityQueue: true,
    allowBoundedAutoRetry: false,
    autoRetryElapsedGuardSeconds: 180,
  });
});

it('clamps saved concurrency and retry settings to supported bounds', async () => {
  vi.mocked(configAPI.getConfig).mockResolvedValueOnce({
    extra_subagent_ids: [],
    strategy_level: 'normal',
    member_strategy_overrides: {},
    reviewer_timeout_seconds: 600,
    judge_timeout_seconds: 600,
    reviewer_file_split_threshold: 20,
    max_same_role_instances: 3,
    max_retries_per_role: 1,
    max_parallel_reviewers: 99,
    max_queue_wait_seconds: 999,
    allow_provider_capacity_queue: false,
    allow_bounded_auto_retry: true,
    auto_retry_elapsed_guard_seconds: 1,
  });

  const team = await loadDefaultReviewTeam(WORKSPACE_PATH);

  expect(team.concurrencyPolicy.maxParallelInstances).toBe(16);
  expect(team.concurrencyPolicy.maxQueueWaitSeconds).toBe(600);
  expect(team.concurrencyPolicy.allowProviderCapacityQueue).toBe(false);
  expect(team.concurrencyPolicy.allowBoundedAutoRetry).toBe(true);
  expect(team.concurrencyPolicy.autoRetryElapsedGuardSeconds).toBe(30);
});
```

Use the existing `WORKSPACE_PATH` fixture constant if present. If no constant exists, introduce `const WORKSPACE_PATH = 'D:/workspace/project-a';` once at the top of the test file.

Run:

```powershell
pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts
```

Expected:

- Fails until the persisted fields and normalization logic exist.

- [x] **Step 2.2: Extend Review Team concurrency types**

In `src/web-ui/src/shared/services/reviewTeamService.ts`, extend `ReviewTeamStoredConfig`, `ReviewTeamConcurrencyPolicy`, `ReviewTeam`, and `ReviewTeamRunManifest` handling with:

```ts
allowProviderCapacityQueue: boolean;
allowBoundedAutoRetry: boolean;
autoRetryElapsedGuardSeconds: number;
```

Persist using snake_case fields:

```ts
max_parallel_reviewers
max_queue_wait_seconds
allow_provider_capacity_queue
allow_bounded_auto_retry
auto_retry_elapsed_guard_seconds
```

Keep manifest shape camelCase:

```ts
concurrencyPolicy: {
  maxParallelInstances,
  staggerSeconds,
  maxQueueWaitSeconds,
  batchExtrasSeparately,
  allowProviderCapacityQueue,
  allowBoundedAutoRetry,
  autoRetryElapsedGuardSeconds,
}
```

- [x] **Step 2.3: Add save helper for concurrency settings**

Add a service function near `saveDefaultReviewTeamExecutionPolicy`:

```ts
export async function saveDefaultReviewTeamConcurrencyPolicy(
  concurrencyPolicy: ReviewTeamConcurrencyPolicy,
): Promise<void>
```

Implementation requirements:

- Load the current default review team config.
- Preserve existing extra reviewers, strategy, member overrides, and execution policy values.
- Save only normalized supported bounds.
- Do not write global `ai.subagent_max_concurrency`.

- [x] **Step 2.4: Add compact settings UI**

In `src/web-ui/src/infrastructure/config/components/ReviewConfig.tsx`:

- Import `Switch` from `@/component-library`.
- Add `savingConcurrencyKey` state.
- Add handlers for numeric concurrency settings and boolean toggles.
- Add one compact section titled by locale key `capacity.title`.

Controls:

- `max_parallel_reviewers`: `NumberInput`, min `1`, max `16`.
- `max_queue_wait_seconds`: `NumberInput`, min `0`, max `600`.
- `allow_provider_capacity_queue`: `Switch`.
- `allow_bounded_auto_retry`: `Switch`.
- `auto_retry_elapsed_guard_seconds`: `NumberInput`, min `30`, max `900`, disabled when bounded auto retry is off.

Keep visual density consistent with the existing execution policy rows.

- [x] **Step 2.5: Add locale entries**

Add matching keys to:

- `src/web-ui/src/locales/en-US/settings/review.json`
- `src/web-ui/src/locales/zh-CN/settings/review.json`
- `src/web-ui/src/locales/zh-TW/settings/review.json`

Required key structure:

```json
{
  "capacity": {
    "title": "...",
    "maxParallelReviewers": {
      "label": "...",
      "description": "..."
    },
    "maxQueueWaitSeconds": {
      "label": "...",
      "description": "..."
    },
    "allowProviderCapacityQueue": {
      "label": "...",
      "description": "..."
    },
    "allowBoundedAutoRetry": {
      "label": "...",
      "description": "..."
    },
    "autoRetryElapsedGuardSeconds": {
      "label": "...",
      "description": "..."
    }
  }
}
```

- [x] **Step 2.6: Run Round 2 verification**

Run:

```powershell
pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts
pnpm run lint:web
pnpm run type-check:web
```

Expected:

- Service tests pass.
- Lint and type-check pass.
- No new untranslated literal appears in `ReviewConfig.tsx`.

- [ ] **Step 2.7: Update status and commit**

Update `docs/deep-review-phase3-followup-plan.md` to mark settings persistence and UI as implemented after Step 2.6 passes.

Run:

```powershell
git add src/web-ui/src/shared/services/reviewTeamService.ts src/web-ui/src/shared/services/reviewTeamService.test.ts src/web-ui/src/infrastructure/config/components/ReviewConfig.tsx src/web-ui/src/infrastructure/config/components/ReviewConfig.scss src/web-ui/src/locales/en-US/settings/review.json src/web-ui/src/locales/zh-CN/settings/review.json src/web-ui/src/locales/zh-TW/settings/review.json docs/deep-review-phase3-followup-plan.md
git commit -m "feat(review-team): expose capacity and retry settings"
```

Expected:

- Commit contains only Round 2 files.

---

## Round 3: Short Provider Capacity Queue

**Goal:** Reattempt narrowly classified provider transient capacity errors after a visible bounded wait.

**Files:**

- Modify: `src/crates/core/src/agentic/deep_review_policy.rs`
- Modify: `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- Modify: `src/crates/events/src/agentic.rs`
- Modify: `src/crates/core/src/agentic/events/types.rs`
- Modify: `src/web-ui/src/flow_chat/utils/deepReviewQueueStateEvents.ts`
- Modify: `src/web-ui/src/flow_chat/store/deepReviewActionBarStore.ts`
- Modify: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx`
- Modify: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.scss`
- Modify: `src/web-ui/src/locales/en-US/flow-chat.json`
- Modify: `src/web-ui/src/locales/zh-CN/flow-chat.json`
- Modify: `src/web-ui/src/locales/zh-TW/flow-chat.json`
- Test: `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- Test: `src/web-ui/src/flow_chat/utils/deepReviewQueueStateEvents.test.ts`
- Test: `src/web-ui/src/flow_chat/store/deepReviewActionBarStore.test.ts`
- Test: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.test.tsx`
- Docs: `docs/deep-review-phase3-followup-plan.md`

- [ ] **Step 3.1: Write provider queue classification tests**

In `src/crates/core/src/agentic/deep_review_policy.rs`, add tests proving:

- `rate_limit`, `provider_concurrency_limit`, `retry_after`, and temporary overload are queueable.
- auth, quota, billing, invalid model, invalid tooling, policy, validation, and cancellation are not queueable.
- `Retry-After` values are bounded by `max_queue_wait_seconds`.

Run:

```powershell
cargo test -p bitfun-core capacity_error -- --nocapture
```

Expected:

- Existing classification tests pass.
- New max-wait bounding tests fail until implemented.

- [ ] **Step 3.2: Add provider queue decision helpers**

In `src/crates/core/src/agentic/deep_review_policy.rs`, add helpers:

```rust
pub fn provider_capacity_queue_wait(
    reason: DeepReviewCapacityQueueReason,
    retry_after_seconds: Option<u64>,
    max_queue_wait_seconds: u64,
    allow_provider_capacity_queue: bool,
) -> Option<Duration>
```

Rules:

- Return `None` when `allow_provider_capacity_queue` is false.
- Return `None` for non-provider capacity reasons unless the reason is `RetryAfter`.
- Return `None` when `max_queue_wait_seconds` is `0`.
- Return `Some(duration)` bounded by `max_queue_wait_seconds`.
- Never return an unbounded duration.

- [ ] **Step 3.3: Convert provider skip into one visible queue reattempt**

In `src/crates/core/src/agentic/tools/implementations/task_tool.rs`:

- When a reviewer fails with queueable provider capacity, emit a provider queue state event before final skip.
- Wait for the bounded provider queue duration.
- Respect existing pause, continue, cancel, and skip controls if they are stored by parent turn.
- Reattempt the reviewer once after the wait.
- Start reviewer runtime timeout only for the actual execution attempt.
- On second provider capacity failure or queue expiry, return the existing `capacity_skipped` payload.

Required queue result fields:

```json
{
  "status": "capacity_skipped",
  "reason": "provider_concurrency_limit",
  "queue_elapsed_ms": 60000
}
```

- [ ] **Step 3.4: Extend event and UI reason mapping**

In event types and web queue parsers, add provider-specific reason display without creating a new banner component:

- `provider_rate_limit`
- `provider_concurrency_limit`
- `retry_after`
- `provider_temporary_overload`

UI copy must say queue time does not count against reviewer runtime.

- [ ] **Step 3.5: Add action-bar tests**

Add or update tests proving:

- Provider queue notice renders as a compact action-bar state.
- Pause and cancel actions remain available.
- Provider queue reason text is localized.
- Repeated queue events update the same state rather than appending duplicate notices.

Run:

```powershell
pnpm --dir src/web-ui run test:run -- src/flow_chat/utils/deepReviewQueueStateEvents.test.ts src/flow_chat/store/deepReviewActionBarStore.test.ts src/flow_chat/components/btw/DeepReviewActionBar.test.tsx
```

Expected:

- New provider reason tests pass.
- Existing local capacity queue tests still pass.

- [ ] **Step 3.6: Run Rust verification**

Run:

```powershell
cargo test -p bitfun-core deep_review_provider_capacity_error_builds_capacity_skipped_payload_and_lowers_effective_cap -- --nocapture
cargo test -p bitfun-core deep_review -- --nocapture
```

Expected:

- Existing capacity-skip behavior remains compatible after bounded provider queueing.
- Deep Review suite passes.

- [ ] **Step 3.7: Update status and commit**

Update `docs/deep-review-phase3-followup-plan.md` to mark short provider capacity queue as implemented only after Steps 3.5 and 3.6 pass.

Run:

```powershell
git add src/crates/core/src/agentic/deep_review_policy.rs src/crates/core/src/agentic/tools/implementations/task_tool.rs src/crates/events/src/agentic.rs src/crates/core/src/agentic/events/types.rs src/web-ui/src/flow_chat/utils/deepReviewQueueStateEvents.ts src/web-ui/src/flow_chat/store/deepReviewActionBarStore.ts src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.scss src/web-ui/src/locales/en-US/flow-chat.json src/web-ui/src/locales/zh-CN/flow-chat.json src/web-ui/src/locales/zh-TW/flow-chat.json src/web-ui/src/flow_chat/utils/deepReviewQueueStateEvents.test.ts src/web-ui/src/flow_chat/store/deepReviewActionBarStore.test.ts src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.test.tsx docs/deep-review-phase3-followup-plan.md
git commit -m "feat(deep-review): queue transient provider capacity errors"
```

Expected:

- Commit contains provider queue runtime, UI, locale, tests, and status doc only.

---

## Round 4: Explicit Retry Action And Bounded Auto-Retry Preference

**Goal:** Let users retry unresolved reviewer slices manually by default, and only run bounded automatic retries after explicit opt-in.

**Files:**

- Modify: `src/crates/core/src/agentic/deep_review_policy.rs`
- Modify: `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- Modify: `src/web-ui/src/flow_chat/utils/codeReviewReport.ts`
- Modify: `src/web-ui/src/flow_chat/store/deepReviewActionBarStore.ts`
- Modify: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx`
- Modify: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.scss`
- Modify: `src/web-ui/src/flow_chat/services/DeepReviewService.ts`
- Modify: `src/web-ui/src/shared/services/reviewTeamService.ts`
- Modify: `src/web-ui/src/locales/en-US/flow-chat.json`
- Modify: `src/web-ui/src/locales/zh-CN/flow-chat.json`
- Modify: `src/web-ui/src/locales/zh-TW/flow-chat.json`
- Test: `src/crates/core/src/agentic/deep_review_policy.rs`
- Test: `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- Test: `src/web-ui/src/flow_chat/utils/codeReviewReport.test.ts`
- Test: `src/web-ui/src/flow_chat/store/deepReviewActionBarStore.test.ts`
- Test: `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.test.tsx`
- Test: `src/web-ui/src/flow_chat/services/DeepReviewService.test.ts`
- Docs: `docs/deep-review-phase3-followup-plan.md`

- [ ] **Step 4.1: Write retry metadata parser tests**

In `src/web-ui/src/flow_chat/utils/codeReviewReport.test.ts`, add cases for:

- `partial_timeout` packet with unresolved files returns a retryable slice.
- transient `capacity_skipped` packet with unresolved files returns a retryable slice.
- non-transient `capacity_skipped` returns no retry action.
- retry scope must be smaller than original assigned scope.

Expected retryable shape:

```ts
{
  sourcePacketId: 'ReviewFrontend-1',
  sourceStatus: 'partial_timeout',
  reviewerId: 'ReviewFrontend',
  retryScopeFiles: ['src/web-ui/src/App.tsx'],
  coveredFiles: ['src/web-ui/src/index.tsx'],
  retryTimeoutSeconds: 300,
}
```

Run:

```powershell
pnpm --dir src/web-ui run test:run -- src/flow_chat/utils/codeReviewReport.test.ts
```

Expected:

- Fails until retry metadata extraction exists.

- [ ] **Step 4.2: Add retry action-bar state**

In `src/web-ui/src/flow_chat/store/deepReviewActionBarStore.ts`, add state for:

```ts
retryableSlices: DeepReviewRetryableSlice[];
retryInProgress: boolean;
autoRetryPreferenceVisible: boolean;
```

Actions:

- `setRetryableSlices`
- `startManualRetry`
- `finishManualRetry`
- `setAutoRetryPreferenceVisible`

Do not show retry controls when there is no structured coverage.

- [ ] **Step 4.3: Add explicit retry UI**

In `DeepReviewActionBar.tsx`:

- Add `Retry unresolved slice` button.
- Add secondary action `Allow bounded automatic retries for future Deep Reviews`.
- Keep the copy compact.
- Keep button disabled while a retry is already in progress.
- Show unresolved status when retry is suppressed by bounds.

Locale keys must live in `flow-chat.json` for all three locales.

- [ ] **Step 4.4: Wire manual retry service call**

In `src/web-ui/src/flow_chat/services/DeepReviewService.ts`:

- Add a method or helper that launches a Deep Review retry from a retryable slice.
- Pass `retry: true`, `retry_coverage`, `retry_scope_files`, source status, source packet id, covered files, and lower timeout.
- Do not alter the original report or hide unresolved findings while retry is running.

- [ ] **Step 4.5: Add bounded auto-retry admission**

In Rust policy and TaskTool:

- Use `allowBoundedAutoRetry` from manifest concurrency policy.
- Record `manual_retry_count`, `auto_retry_count`, and suppressed reasons.
- Permit automatic retry only when:
  - preference is true;
  - source status is `partial_timeout` or transient `capacity_skipped`;
  - retry scope is non-empty and smaller than source scope;
  - role retry budget remains;
  - elapsed guard remains;
  - failure type is not auth, quota, billing, invalid model, policy, invalid tooling, validation, or cancellation.

Use stable suppression reason strings:

```rust
"preference_disabled"
"budget_exhausted"
"scope_not_reduced"
"elapsed_guard_exceeded"
"non_retryable_status"
"non_transient_error"
"missing_coverage"
```

- [ ] **Step 4.6: Persist auto-retry preference from the UI**

Use the Round 2 `saveDefaultReviewTeamConcurrencyPolicy` helper to update `allowBoundedAutoRetry`.

Requirements:

- The first retry remains manual.
- The opt-in action updates settings explicitly.
- Settings can disable the preference later.

- [ ] **Step 4.7: Run Round 4 verification**

Run:

```powershell
cargo test -p bitfun-core task_tool -- --nocapture
cargo test -p bitfun-core deep_review -- --nocapture
pnpm --dir src/web-ui run test:run -- src/flow_chat/utils/codeReviewReport.test.ts src/flow_chat/store/deepReviewActionBarStore.test.ts src/flow_chat/components/btw/DeepReviewActionBar.test.tsx src/flow_chat/services/DeepReviewService.test.ts
pnpm run lint:web
pnpm run type-check:web
```

Expected:

- Manual retry works for structured unresolved slices.
- Bounded auto retry is disabled by default.
- Enabled auto retry stops at configured guards.
- No missing locale keys are introduced.

- [ ] **Step 4.8: Update status and commit**

Update `docs/deep-review-phase3-followup-plan.md` to mark retry controls as implemented after Step 4.7 passes.

Run:

```powershell
git add src/crates/core/src/agentic/deep_review_policy.rs src/crates/core/src/agentic/tools/implementations/task_tool.rs src/web-ui/src/flow_chat/utils/codeReviewReport.ts src/web-ui/src/flow_chat/store/deepReviewActionBarStore.ts src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.scss src/web-ui/src/flow_chat/services/DeepReviewService.ts src/web-ui/src/shared/services/reviewTeamService.ts src/web-ui/src/locales/en-US/flow-chat.json src/web-ui/src/locales/zh-CN/flow-chat.json src/web-ui/src/locales/zh-TW/flow-chat.json src/web-ui/src/flow_chat/utils/codeReviewReport.test.ts src/web-ui/src/flow_chat/store/deepReviewActionBarStore.test.ts src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.test.tsx src/web-ui/src/flow_chat/services/DeepReviewService.test.ts docs/deep-review-phase3-followup-plan.md
git commit -m "feat(deep-review): add bounded retry controls"
```

Expected:

- Commit includes only retry runtime, UI, service, locales, tests, and status doc.

---

## Round 5: Cost-Aware Review Scope And Shared Evidence

**Goal:** Reduce review time and token usage on large changes by making quick/default strategies high-risk-first and by giving reviewers a shared evidence pack before they perform targeted reads.

**Files:**

- Modify: `src/web-ui/src/shared/services/reviewTeamService.ts`
- Modify: `src/web-ui/src/shared/services/reviewTeamService.test.ts`
- Modify: `src/crates/core/src/agentic/agents/prompts/deep_review_agent.md`
- Modify: `src/crates/core/src/agentic/agents/prompts/review_business_logic_agent.md`
- Modify: `src/crates/core/src/agentic/agents/prompts/review_performance_agent.md`
- Modify: `src/crates/core/src/agentic/agents/prompts/review_security_agent.md`
- Modify: `src/crates/core/src/agentic/agents/prompts/review_architecture_agent.md`
- Modify: `src/crates/core/src/agentic/agents/prompts/review_frontend_agent.md`
- Modify: `src/crates/core/src/agentic/agents/prompts/review_quality_gate_agent.md`
- Modify: `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs`
- Docs: `docs/deep-review-phase3-followup-plan.md`

- [ ] **Step 5.1: Add manifest tests for strategy depth**

In `src/web-ui/src/shared/services/reviewTeamService.test.ts`, add tests proving:

- `quick` produces `reviewDepth: 'high_risk_only'`.
- `normal` produces `reviewDepth: 'risk_expanded'`.
- `deep` produces `reviewDepth: 'full_depth'`.
- Quick/default optional reviewers are included only when the target tags match their applicability rules or when the user explicitly selects deep/full coverage.
- Every reduced-depth manifest still includes file coverage metadata and a `coverageExpectation` string.

Run:

```powershell
pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts
```

Expected:

- New tests fail until the manifest carries the scope profile.

- [ ] **Step 5.2: Add the cost-aware scope profile**

In `src/web-ui/src/shared/services/reviewTeamService.ts`, add a manifest profile with this shape:

```ts
type DeepReviewScopeProfile = {
  reviewDepth: 'high_risk_only' | 'risk_expanded' | 'full_depth';
  riskFocusTags: string[];
  maxDependencyHops: number | 'policy_limited';
  optionalReviewerPolicy: 'risk_matched_only' | 'configured' | 'full';
  allowBroadToolExploration: boolean;
  coverageExpectation: string;
};
```

Mapping:

- `quick`: high-risk only, `maxDependencyHops: 0`, optional reviewers risk-matched only, broad exploration off.
- `normal`: risk-expanded, `maxDependencyHops: 1`, optional reviewers configured but still applicability-gated, broad exploration limited.
- `deep`: full-depth, `maxDependencyHops: 'policy_limited'`, optional reviewers follow configured/deep behavior, broad exploration allowed.

- [ ] **Step 5.3: Build a shared evidence pack**

In `reviewTeamService.ts`, add a compact evidence pack under the run manifest:

```ts
type DeepReviewEvidencePack = {
  changedFiles: string[];
  diffStat: {
    fileCount: number;
    totalChangedLines: number;
  };
  domainTags: string[];
  riskFocusTags: string[];
  packetIds: string[];
  hunkHints: Array<{
    filePath: string;
    changedLineCount: number;
  }>;
  contractHints: Array<{
    kind: 'i18n_key' | 'tauri_command' | 'api_contract' | 'config_key';
    value: string;
    filePath: string;
  }>;
};
```

Rules:

- Do not include full file text or full diff text.
- Derive contract hints only from already available changed files, target classification, and cheap key/name extraction.
- Keep the pack source-agnostic. If the target is not Git-based, file lists and source labels are still valid.

- [ ] **Step 5.4: Update reviewer prompt boundaries**

Update `deep_review_agent.md` and specialist prompts so reviewers:

- Start from the shared evidence pack.
- Treat quick mode as high-risk gate, not broad audit.
- Treat normal mode as risk-expanded, one-hop review.
- Use `Read` and `GetFileDiff` for targeted confirmation, not initial discovery.
- Report coverage limitations when the scope profile prevents broad exploration.

Update `review_quality_gate_agent.md` so the judge does not treat a high-risk-only pass as full-depth coverage.

- [ ] **Step 5.5: Fold reduced-depth status into final report metadata**

In `code_review_tool.rs`, preserve `reviewDepth`, `coverageExpectation`, and reduced-depth reliability notes when the manifest provides them.

Do not create a dense report section; use existing reliability/coverage wording.

- [ ] **Step 5.6: Run focused verification**

Run:

```powershell
pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts
cargo test -p bitfun-core deep_review -- --nocapture
```

Expected:

- Manifest tests pass.
- Deep Review Rust tests pass.
- No prompt or report claim says quick/default is full-depth.

- [ ] **Step 5.7: Update status and commit**

Update `docs/deep-review-phase3-followup-plan.md` after Step 5.6 passes.

Run:

```powershell
git add src/web-ui/src/shared/services/reviewTeamService.ts src/web-ui/src/shared/services/reviewTeamService.test.ts src/crates/core/src/agentic/agents/prompts/deep_review_agent.md src/crates/core/src/agentic/agents/prompts/review_business_logic_agent.md src/crates/core/src/agentic/agents/prompts/review_performance_agent.md src/crates/core/src/agentic/agents/prompts/review_security_agent.md src/crates/core/src/agentic/agents/prompts/review_architecture_agent.md src/crates/core/src/agentic/agents/prompts/review_frontend_agent.md src/crates/core/src/agentic/agents/prompts/review_quality_gate_agent.md src/crates/core/src/agentic/tools/implementations/code_review_tool.rs docs/deep-review-phase3-followup-plan.md
git commit -m "feat(deep-review): add cost-aware review scope"
```

Expected:

- Commit includes only scope-profile, evidence-pack, prompt/report, tests, and status doc changes.

---

## Round 6: Documentation Reconciliation And Product Risk Review

**Goal:** Make all Deep Review documents accurately reflect implemented behavior, deferred scope, and remaining risks.

**Files:**

- Modify: `docs/deep-review-design.md`
- Modify: `docs/deep-review-phase2-plan.md`
- Modify: `docs/deep-review-phase2-addendum.md`
- Modify: `docs/deep-review-phase3-followup-plan.md`
- Modify: `docs/superpowers/plans/2026-05-09-deep-review-phase3-execution-plan.md`

- [ ] **Step 6.1: Scan for stale completion claims**

Run:

```powershell
rg -n "project-level cache.*implemented|automatic retry.*complete|provider/adaptive queue.*complete|hard prompt.*complete|global.*concurrency.*automatic" docs/deep-review-design.md docs/deep-review-phase2-plan.md docs/deep-review-phase2-addendum.md docs/deep-review-phase3-followup-plan.md docs/superpowers/plans/2026-05-09-deep-review-phase3-execution-plan.md
```

Expected:

- No stale text claims deferred work is already implemented.
- Any matches describe a verified completed round or an explicit deferred item.

- [ ] **Step 6.2: Reconcile status wording**

Use these status labels consistently:

- `Implemented` for code landed and verified.
- `Implemented with guardrails` for behavior that is active but bounded by settings, budgets, or user controls.
- `Deferred by product decision` for project-level cache and programmatic shared-context cache expansion.
- `Risk accepted for Phase 3` for known non-blocking behavior with a documented mitigation.
- `Pending implementation` for items that remain in this execution plan.

- [ ] **Step 6.3: Add measured outcome notes**

In `docs/deep-review-phase3-followup-plan.md`, add a concise measured outcome section after implementation:

```markdown
## Measured Outcome Notes

- Runtime diagnostics are emitted as one aggregate final snapshot per Deep Review turn.
- Provider transient capacity waits are visible through the existing queue action bar and bounded by Review settings.
- Retry defaults to manual action. Bounded automatic retry runs only after explicit opt-in and stops at role, packet, and elapsed guards.
- Quick/default review paths are high-risk or risk-expanded by design; deep remains the full-depth option.
- Shared evidence packs reduce repeated discovery work without storing source content in diagnostics.
- Project-level cache remains deferred and no new persistent review-output cache is introduced.
```

- [ ] **Step 6.4: Run full release gate**

Run:

```powershell
cargo test -p bitfun-core deep_review -- --nocapture
cargo check --workspace --exclude bitfun-cli
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/web-ui run test:run
git diff --check
```

Expected:

- All listed verification commands pass.
- Any failure is triaged before final status is marked complete.

- [ ] **Step 6.5: Commit Round 6**

Run:

```powershell
git add docs/deep-review-design.md docs/deep-review-phase2-plan.md docs/deep-review-phase2-addendum.md docs/deep-review-phase3-followup-plan.md docs/superpowers/plans/2026-05-09-deep-review-phase3-execution-plan.md
git commit -m "docs(deep-review): reconcile phase three status"
```

Expected:

- Commit contains only documentation reconciliation.

---

## Final Acceptance Checklist

- [ ] Runtime diagnostics are aggregate-only and low frequency.
- [ ] Shared-context measurements remain content-free.
- [ ] Provider transient queue is short, visible, pauseable, cancellable, and bounded.
- [ ] Queue time is separated from reviewer runtime timeout.
- [ ] Deep Review max parallel reviewers can be lowered explicitly without changing global subagent concurrency.
- [ ] Manual retry exists for structured unresolved slices.
- [ ] Bounded auto retry is disabled by default and requires explicit opt-in.
- [ ] Auto retry cannot loop indefinitely.
- [ ] Quick/default strategies emphasize high-risk review instead of full-depth broad exploration.
- [ ] Shared evidence packs reduce repeated reviewer discovery work without storing source, full diff, or reviewer output content.
- [ ] All new UI strings are localized in `en-US`, `zh-CN`, and `zh-TW`.
- [ ] Project-level review cache remains deferred.
- [ ] Existing Deep Review report and Review Team behavior are not disrupted outside the planned controls.
- [ ] Full release gate passes.

## Stop Conditions

Stop and review the design before continuing if any of these occur:

- A fix requires changing global `ai.subagent_max_concurrency` as the normal Deep Review recovery path.
- A runtime change stores source, diff, reviewer output, provider raw body, or full file contents in diagnostics.
- Provider queue needs more than one automatic reattempt per reviewer packet.
- Auto retry needs to retry a scope that is not smaller than the original packet.
- Quick/default cost reduction would hide changed files from coverage metadata instead of marking reduced-depth coverage.
- Shared evidence reuse requires caching full `Read` outputs across subagents before duplicate-call diagnostics justify it.
- A UI change introduces a new page or modal when the existing action bar or Review settings section can carry the workflow.
- A test requires broad snapshots instead of behavior assertions for queue, retry, or settings state.

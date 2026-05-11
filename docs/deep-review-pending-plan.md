# Deep Review Pending Plan And Architecture Refactor

## Purpose

This document consolidates all pending and deferred Deep Review work into one executable plan. It also includes the architecture refactor design that should guide future implementation without changing current behavior unless a specific product checkpoint approves it.

The completed behavior baseline is tracked separately in `docs/deep-review-completed-status.md`.

## Source Documents

This plan merges the pending work from:

- `docs/deep-review-design.md`
- `docs/deep-review-phase2-plan.md`
- `docs/deep-review-phase2-addendum.md`
- `docs/deep-review-phase3-followup-plan.md`
- `docs/deep-review-architecture-refactor-plan.md`
- `docs/deep-review-nondeepreview-impact-inventory.md`
- `docs/superpowers/plans/2026-05-09-deep-review-phase3-execution-plan.md`
- local companion copies under `docs/superpowers/specs/` and `docs/superpowers/plans/` that duplicate the same Deep Review design lineage.

## Product And Architecture Boundaries

Future implementation must stay inside these boundaries:

1. Deep Review remains prompt-driven with deterministic guardrails. Do not replace it with a backend DAG scheduler without a separate design approval.
2. Deep Review queueing must not become global subagent queueing by accident.
3. Deep Review must not silently consume normal user-session concurrency. If the session is already busy, the product should warn, pause, or require manual continuation.
4. Queue wait time must not count against reviewer runtime timeout; local reviewer queue liveness now waits while the same Deep Review still has an active reviewer.
5. Provider capacity queueing must be short, visible, bounded, pauseable, and cancellable.
6. Automatic retry is manual by default and can only become bounded automatic retry after explicit user opt-in.
7. Automatic retry must never loop indefinitely.
8. Quick/default review depth may reduce breadth, but must not hide changed files from coverage metadata.
9. Diagnostics must be low-frequency and content-free.
10. Project-level review cache remains deferred until retention, deletion, invalidation, and user visibility rules are approved.
11. Refactor rounds must preserve existing behavior unless a behavior-change checkpoint is explicitly approved.

## Remaining Functional Plan

### Round 1: Short Provider Capacity Queue

Status: Implemented with guardrails.

Goal: When the provider returns a narrowly classified transient capacity error, Deep Review waits briefly and retries once before reporting `capacity_skipped`.

In scope:

- Treat only the following as queueable:
  - provider rate limit;
  - provider concurrency limit;
  - explicit `Retry-After`;
  - temporary overload/capacity pressure.
- Fail fast for:
  - authentication;
  - billing/quota exhaustion;
  - invalid model;
  - policy violation;
  - user cancellation;
  - invalid reviewer tooling;
  - deterministic validation errors.
- Bound wait by `min(Retry-After, max_queue_wait_seconds)`.
- Reattempt the reviewer once when the user has not paused or cancelled.
- Emit existing queue-state events with provider-specific reasons.
- Record aggregate diagnostics counters:
  - provider queue count;
  - provider retry count;
  - provider retry success count;
  - final capacity skip count.
- Keep queue time separate from reviewer runtime timeout.
- Reuse the compact action-bar queue notice and controls.

Risks:

| Risk | Why it matters | Mitigation |
|---|---|---|
| Provider wait looks stuck | Users may think the review froze. | Visible queue notice, elapsed queue time, pause/continue/cancel controls. |
| Wrong error is queued | Auth/quota/model errors could wait forever. | Narrow classifier with fail-fast tests. |
| Deep Review starves the active session | Queued reviewers could resume when normal work needs capacity. | Preserve active-session warning and manual pause/continue. |
| Retry extends total review time | A slow model can make capacity waits expensive. | One short reattempt, max queue wait, aggregate diagnostics. |

Verification:

- Rust tests exist for queueable vs non-queueable provider errors, queue expiry, pause, cancel, and diagnostics counters.
- Frontend tests exist for provider queue notice, localized reason text, and queue-state updates.
- Earlier milestone Rust verification passed with `cargo test -p bitfun-core deep_review -- --nocapture` and a partial workspace check; the agreed merge verification pass later completed `cargo test -p bitfun-events deep_review_queue_state_event_serializes_stable_contract -- --nocapture`, `cargo test -p bitfun-core deep_review -- --nocapture`, `cargo check --workspace`, and `cargo test --workspace`.

Exit criteria:

- Provider queue is visible and bounded.
- Queue time is timeout-separated.
- Non-transient errors fail fast.
- Final report remains honest when the provider queue expires.

### Round 2: Explicit Retry Action And Bounded Auto-Retry Preference

Status: Implemented with guardrails; backend-owned automatic redispatch scheduling remains deferred.

Goal: Give users a clear retry action for unresolved reviewer slices, while allowing future backend-owned automatic retries only after explicit opt-in and runtime admission checks.

In scope:

- Extract retryable unresolved packets from report metadata:
  - source packet id;
  - reviewer id;
  - source status;
  - covered files;
  - unresolved files;
  - retry timeout.
- Retryable sources:
  - `partial_timeout`;
  - transient `capacity_skipped`.
- Non-retryable sources:
  - auth;
  - quota/billing;
  - invalid model;
  - policy;
  - invalid tooling;
  - validation;
  - cancellation;
  - non-transient capacity skip.
- Add an explicit action-bar button for retrying unresolved slices.
- Persist the opt-in through Review Team settings.
- Keep bounded automatic retry disabled by default.
- `auto_retry` admission may pass only when:
  - preference is enabled;
  - source status is retryable;
  - retry coverage is structured;
  - retry scope is non-empty and smaller than the original scope;
  - role/packet retry budget remains;
  - elapsed guard remains;
  - timeout is lower than the source task timeout.
- Stable suppression reasons:
  - `auto_retry_disabled`;
  - `budget_exhausted`;
  - `scope_not_reduced`;
  - `elapsed_guard_exceeded`;
  - `non_retryable_status`;
  - `missing_coverage`.

Risks:

| Risk | Why it matters | Mitigation |
|---|---|---|
| Retry loops forever | Token and time usage can explode. | Role budget, packet budget, smaller scope, elapsed guard, one-slice-at-a-time execution. |
| Retry repeats bad context | Broad retry can produce the same failure. | Structured coverage and reduced scope are mandatory. |
| User loses control | Hidden retry feels like unexpected automation. | Manual by default; opt-in is explicit and reversible in settings. |
| Report becomes noisy | Retry metadata can crowd findings. | Keep retry controls compact and show only unresolved status when needed. |

Verification:

- Frontend report parser tests exist for retryable and non-retryable slices.
- Action-bar tests exist for manual retry launch metadata and disabled/in-flight state coverage.
- Rust unit tests exist for retry admission, auto-retry opt-in, suppression reasons, and budget guards.
- Lint, type-check, focused web tests, and full Rust Deep Review tests remain part of the combined milestone verification pass.

Exit criteria:

- Manual retry works for structured unresolved slices.
- Automatic retry is disabled by default.
- Opted-in `auto_retry` admission remains bounded and cannot loop indefinitely.
- No backend scheduler automatically redrives reviewer slices yet.

### Round 3: Cost-Aware Review Scope

Status: Implemented with guardrails; release-gate verification, observability, and wording alignment are recorded.

Goal: Reduce review time and token use on large or slow-model changes by making quick/default strategies focus first on high-risk evidence, while keeping `deep` as the full-depth option.

Scope profile:

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

Strategy mapping:

| Strategy | Review depth | Dependency context | Optional reviewers | Exploration |
|---|---|---|---|---|
| `quick` | `high_risk_only` | Changed files and direct high-risk contracts only | Risk-matched only | Broad exploration off |
| `normal` | `risk_expanded` | Changed files plus one-hop high-risk context | Configured but applicability-gated | Limited |
| `deep` | `full_depth` | Policy-limited broad context | Configured/full behavior | Allowed |

High-risk categories that must stay in quick/default scope:

- security;
- data loss;
- migrations;
- authentication/authorization;
- cross-boundary API contracts;
- concurrency;
- persistence;
- configuration changes;
- platform boundary violations.

Risks:

| Risk | Why it matters | Mitigation |
|---|---|---|
| Reduced depth misses low-risk regressions | Quick/default trades breadth for speed. | Label coverage honestly and offer a deeper path. |
| Judge overstates confidence | A high-risk-only pass is not full review. | Judge prompt and report metadata must preserve `reviewDepth` and `coverageExpectation`. |
| Optional reviewers disappear unexpectedly | Users may expect configured reviewers to run. | Use clear applicability/risk-match metadata and show skipped reasons. |
| Strategy becomes hidden override | Runtime might change user-selected intent. | Scope profile narrows depth, but does not secretly change selected strategy. |

Verification:

- Manifest tests for all three depth profiles.
- Tests that reduced-depth manifests preserve changed-file coverage metadata.
- Prompt updates for reviewers and judge.
- Report reliability tests for reduced-depth wording.
- Rust Deep Review tests to ensure report schema remains compatible.

Exit criteria:

- Quick is high-risk-only.
- Normal is risk-expanded.
- Deep remains full-depth.
- Reports do not claim full coverage for reduced-depth runs.

### Round 4: Shared Evidence Pack

Status: Implemented with guardrails; programmatic full tool-result cache remains deferred.

Goal: Let reviewers start from compact shared facts so they spend less time and fewer tokens rediscovering the same files, hunks, and contract hints.

Proposed manifest shape:

```ts
type DeepReviewEvidencePack = {
  version: 1;
  source: 'target_manifest';
  changedFiles: string[];
  diffStat: {
    fileCount: number;
    totalChangedLines?: number;
    lineCountSource: 'unknown' | 'diff_stat' | 'estimated';
  };
  domainTags: string[];
  riskFocusTags: string[];
  packetIds: string[];
  hunkHints: Array<{
    filePath: string;
    changedLineCount: number;
    lineCountSource: 'unknown' | 'diff_stat' | 'estimated';
  }>;
  contractHints: Array<{
    kind: 'i18n_key' | 'tauri_command' | 'api_contract' | 'config_key';
    filePath: string;
    source: 'path_classifier';
  }>;
  budget: {
    maxChangedFiles: number;
    maxHunkHints: number;
    maxContractHints: number;
    omittedChangedFileCount: number;
    omittedHunkHintCount: number;
    omittedContractHintCount: number;
  };
  privacy: {
    content: 'metadata_only';
    excludes: [
      'source_text',
      'full_diff',
      'model_output',
      'provider_raw_body',
      'full_file_contents',
    ];
  };
};
```

Rules:

- Do not include full source text.
- Do not include full diff text.
- Do not include reviewer output.
- Do not include provider raw response bodies.
- Keep the evidence source-agnostic.
- Prefer metadata, hunk ranges, domain tags, risk tags, packet ids, and cheap contract hints.
- Use targeted `Read` and `GetFileDiff` calls only for confirmation or missing context.
- Defer programmatic cross-subagent `Read` output reuse until duplicate-tool diagnostics prove material repeated cost.

Risks:

| Risk | Why it matters | Mitigation |
|---|---|---|
| Evidence pack becomes a hidden context blob | It could recreate the token/privacy problem. | Metadata-first, content-free by default. |
| Hints become stale | Reviewers may rely on outdated metadata. | Derive pack once from the same manifest inputs and include source labels. |
| Tool-result reuse changes reviewer isolation | Shared full reads can leak or freeze context. | Defer full tool-result reuse; start with evidence only. |
| Contract extraction gets expensive | Heavy static analysis would hurt launch latency. | Use cheap extraction from already known changed files and names. |

Verification:

- Manifest tests for evidence pack structure, source label, size budget, and privacy boundary.
- Tests proving no full source, full diff, model output, provider raw body, or full file contents are stored in the pack.
- Prompt tests or snapshot-light assertions proving reviewers start from evidence but must verify stale hints with tools.
- Diagnostics keep content-free duplicate measurement and include an aggregate duplicate discovery savings candidate count.

Exit criteria:

- Reviewers receive compact shared evidence.
- No source/diff/model output is stored in diagnostics.
- Duplicate discovery should reduce without changing tool semantics.

### Round 5: Documentation Reconciliation And Release Gate

Status: Completed release gate for provider queue, retry-control, cost-aware scope, and evidence-pack rounds.

Goal: Keep documents and code aligned after each functional close.

Actions:

- Update status wording only after verification passes.
- Keep provider queue marked implemented-with-guardrails, not a generic provider/adaptive scheduler.
- Keep retry controls marked implemented-with-guardrails, not backend-owned automatic redispatch.
- Keep project-level cache deferred.
- Keep programmatic shared context cache deferred unless measurements justify it.
- Keep hard prompt-byte clipping deferred.
- Scan for stale completion claims.

Verification:

```powershell
rg -n "project-level cache.*implement[[:alpha:]]*ed|auto retry.*compl[[:alpha:]]*ete|provider/adaptive queue.*compl[[:alpha:]]*ete|hard prompt.*compl[[:alpha:]]*ete|global.*concurrency.*auto[[:alpha:]]*matic" docs/deep-review-design.md docs/deep-review-phase2-plan.md docs/deep-review-phase2-addendum.md docs/deep-review-phase3-followup-plan.md
cargo test -p bitfun-core deep_review -- --nocapture
cargo check --workspace
cargo test --workspace
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/web-ui run test:run
git diff --check
```

Exit criteria:

- Docs distinguish implemented, guarded, prompt-guided, deferred, and pending behavior.
- No document claims exceed code behavior.
- No new user-facing string lacks locale coverage.
- No queue/retry/cache/token feature introduces hidden confusion or undocumented privacy risk.

## Deferred Product Decisions

Product recommendation snapshot:

| Decision item | Priority | Recommended product path | Advantages | Trade-offs / risks |
|---|---:|---|---|---|
| Backend batch / stagger scheduling | P0 | Design a lightweight default stagger with visible launched/waiting reviewer counts before adding advanced controls. | Reduces provider bursts and makes waiting easier to explain. | Can increase wall-clock time and complicate failure ordering. |
| Backend-owned automatic retry redispatch scheduler | P1 | Start with explicit opt-in, one bounded automatic redispatch for transient capacity/timeout slices only. | Improves completion rate without requiring manual monitoring. | Hidden cost, stale context, retry loops, and report merge confusion if not visible/cancellable. |
| User-facing effective-cap override controls | P1 | Prefer intent-based settings such as Conservative / Balanced / Faster; keep learned effective cap as a system recommendation, not a raw required concept. | Gives advanced users control while reducing configuration confusion. | Still needs careful copy and safe bounds to avoid degraded performance. |
| Hard prompt-byte clipping | P2 | Offer an honest reduced-coverage choice with explicit coverage/reliability metadata. | Provides a hard ceiling for large targets. | Hidden coverage loss if report/export language is not strict. |
| Authoritative runtime strategy selection | P2 | Keep advisory warnings first; require user confirmation before any strategy override. | Prevents under/over-review without taking away control. | Opaque scoring can reduce trust and expand validation scope. |
| Programmatic shared tool-result cache | P3 | Continue measurement-only diagnostics until duplicate cost is proven material, then consider session-scoped limited caching. | Could reduce repeated tool IO/token cost. | Source/diff privacy and stale-result semantics are heavy. |
| Project-level review cache | P3 | Defer until retention, deletion, invalidation, and user-visible management are designed together. | Speeds repeated project reviews. | Persistent security findings and stale results are product/privacy risks. |
| Generic global subagent queue | P4 | Treat as a platform runtime project, not a Deep Review cleanup. | Could unify fairness and capacity telemetry across hidden subagents. | May unexpectedly slow ordinary subagents and break event/settings expectations. |
| Backend DAG scheduler | P4 | Keep prompt-driven orchestration with deterministic guardrails; revisit only if Deep Review needs workflow-level SLA guarantees. | Deterministic orchestration and observability. | Highest migration and compatibility risk. |

### Backend-Owned Automatic Retry Redispatch Scheduler

Status: Deferred for separate product design.

Current boundary:

- manual retry is implemented for structured unresolved slices;
- `allow_bounded_auto_retry` is an admission guard only;
- there is no backend-owned caller, retry queue, or automatic redispatch loop.

Potential benefit: automatically recover transient capacity/timeout failures without requiring the user to notice and trigger manual retry.

Primary risks: hidden automation, repeated cost, stale context, retry loops, confusing report merge states, and conflicts with queue pause/cancel controls.

Do not implement until trigger timing, visibility, cancellation, retry budgets, failure attribution, and final-report merge semantics are designed as product behavior.

### Project-Level Review Cache

Status: Deferred by product decision.

Do not implement until a separate plan defines:

- retention duration;
- invalidation across file rename, model, strategy, roster, and prompt changes;
- deletion behavior;
- user visibility and management UI;
- whether reviewer outputs may be persisted outside session metadata;
- privacy review for source summaries and security findings.

Current boundary: per-session cache only.

Potential benefit: repeated reviews of the same project can reuse prior context across sessions.

Primary risks: stale findings, source/privacy retention, deletion semantics, and invalidation across model, strategy, roster, prompt, or file changes.

### Programmatic Shared Tool-Result Cache

Status: Deferred pending measured need.

Current boundary:

- prompt-level reuse guidance;
- content-free duplicate `Read`/`GetFileDiff` measurement;
- final aggregate diagnostics.

Do not intercept and reuse full tool results until real-run measurements prove material duplicate cost and a separate semantics/privacy plan is approved.

Potential benefit: reduce repeated `Read` / `GetFileDiff` work and token/IO cost.

Primary risks: stale tool results, source/diff leakage, cache growth, and silently bypassing tool semantics.

### Hard Prompt-Byte Clipping

Status: Deferred.

Current boundary:

- heuristic prompt-byte estimate;
- summary-first full-scope metadata;
- file splitting/max-file guardrails.

Do not hard-clip files from reviewer coverage unless every omitted or reduced file remains explicit in coverage/reliability metadata.

Potential benefit: provide a hard cost and timeout ceiling for very large review targets.

Primary risks: hidden coverage loss, missed high-risk files, and reports that overstate full-review confidence.

### Backend DAG Scheduler

Status: Deferred.

Current boundary:

- prompt-driven orchestration;
- TaskTool hard guardrails;
- local-cap queue controls;
- Deep Review-scoped short provider capacity queue.

Do not replace the orchestrator with a deterministic backend workflow engine in the current plan.

Potential benefit: deterministic orchestration, unified retry/queue handling, and clearer observability.

Primary risks: large migration blast radius, reduced model adaptability, complicated interruption/resume semantics, and session compatibility.

### Authoritative Runtime Strategy Selection

Status: Deferred.

Current boundary:

- advisory/mismatch-warning metadata only.

Do not let backend risk scoring override the user's selected strategy until measured complexity signals and product approval exist.

Potential benefit: automatically prevent under- or over-reviewing when the selected strategy mismatches target complexity.

Primary risks: overriding user intent, opaque scoring, project-type misclassification, and a much larger validation matrix.

### Backend Batch / Stagger Scheduling

Status: Deferred for separate product design.

Current boundary: Deep Review may queue for local/provider capacity, local reviewer queue liveness now waits while the same Deep Review still has an active reviewer, and a narrow launch-batch overlap guard (`launch_batch_blocked`) queues later batches when an earlier batch is active. It still does not intentionally own full batch/stagger reviewer launch order as a scheduling product behavior.

Potential benefit: reduce provider burst pressure, improve capacity success rate, and make progress easier to surface incrementally.

Primary risks: longer wall-clock completion, worse perceived waiting, more complex failure ordering, and overlap with active-session warning, pause, and manual continuation semantics.

Do not implement until launch cadence, batch size, defaults, user-facing waiting copy, fairness, and failure recovery are designed.

### User-Facing Effective-Cap Override Controls

Status: Deferred for separate product design.

Current boundary: effective capacity learning remains a runtime guardrail and diagnostic signal, not a direct user control surface.

Potential benefit: advanced users can tune throughput or reduce local/provider pressure for their environment.

Primary risks: confusing overlap among learned effective cap, global subagent cap, and Review Team capacity settings; unsafe high values; degraded performance from low values; and unclear accountability when overrides conflict with learned limits.

Do not implement until the control model, settings placement, safe bounds, explanatory copy, and relationship to learned effective cap are designed.

## Architecture Refactor Plan

Status: Partially implemented. Backend Deep Review modules, Flow Chat compatibility facades, and the frontend review-team directory have been extracted. The latest no-behavior-change frontend passes split review-team pure helpers and Flow Chat Deep Review helpers for launch parsing/targeting/prompt/errors, report reliability/manifest/section/markdown formatting, and action-bar capacity/header/formatting/remediation/action-controls/diagnostics components. Remaining refactor follow-ups are no-behavior-change unless explicitly called out and approved.

### Refactor Goals

1. Move Deep Review-specific logic out of broad shared files where possible.
2. Separate generic subagent runtime primitives from Deep Review policy adapters.
3. Keep standard subagent behavior stable unless a change is explicitly reviewed as a product decision.
4. Reduce oversized files and repeated definitions.
5. Preserve existing Deep Review behavior during refactor rounds.
6. Keep dependencies acyclic and location choices predictable.
7. Make non-DeepReview impact explicit and testable.
8. Keep frontend and backend Deep Review boundaries clear.
9. Avoid new performance, quality, or security risks.

### Current Refactor Pressure

| Area | Current pressure |
|---|---|
| `src/crates/core/src/agentic/deep_review_policy.rs` | Contains role/team definition, manifest parsing, execution policy, concurrency, queue controls, effective cap learning, retry, diagnostics, shared-context measurement, cache, and tests. |
| `src/crates/core/src/agentic/tools/implementations/task_tool.rs` | Generic Task tool contains Deep Review capacity wait, retry admission, packet/cache lookup, provider capacity skip, queue event, and tests. |
| `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs` | Standard Code Review and Deep Review report logic are mixed with packet fallback, reliability, diagnostics, and cache write-through. |
| `src/crates/core/src/agentic/tools/pipeline/tool_pipeline.rs` | Generic tool pipeline carries Deep Review context propagation and duplicate read/diff measurement. |
| `src/crates/events/src/agentic.rs` | Shared event crate contains Deep Review queue event payload. |
| `src/web-ui/src/shared/services/reviewTeamService.ts` / `src/web-ui/src/shared/services/review-team/` | `reviewTeamService.ts` is now a two-line facade. The directory owns focused pure helper modules plus `index.ts` for side-effectful config/backend loading, default team assembly, and final manifest assembly. |
| `src/web-ui/src/flow_chat/services/DeepReviewService.ts` / `src/web-ui/src/flow_chat/deep-review/launch/DeepReviewService.ts` | Old path is a facade; launch orchestration keeps child-session launch and runtime manifest signals while command parsing, target resolution, launch prompt formatting, and launch errors are split. |
| `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx` / `src/web-ui/src/flow_chat/deep-review/action-bar/DeepReviewActionBar.tsx` | Old path is a facade; capacity queue notice, partial-results panel, recovery-plan preview, action header, formatting, remediation selection, action controls, and diagnostics text building are split. The main action-bar path now owns state orchestration and callback wiring. |
| `src/web-ui/src/flow_chat/utils/codeReviewReport.ts` / `src/web-ui/src/flow_chat/deep-review/report/codeReviewReport.ts` | Old path is a facade; retry-slice helpers and types stay in the core report module while reliability notices, manifest rendering, section normalization, and markdown export are split. |

### Target Backend Layout

Create a Deep Review subsystem under core:

```text
src/crates/core/src/agentic/deep_review/
  mod.rs
  constants.rs
  team_definition.rs
  manifest.rs
  execution_policy.rs
  concurrency_policy.rs
  queue.rs
  retry.rs
  diagnostics.rs
  shared_context.rs
  incremental_cache.rs
  report.rs
  task_adapter.rs
  tests/
```

Responsibilities:

| Module | Responsibility |
|---|---|
| `constants.rs` | Agent type constants and role families. |
| `team_definition.rs` | Default review team definition and strategy profile data. |
| `manifest.rs` | Typed accessors for `deep_review_run_manifest`, packet lookup, strategy/concurrency/cache/token fields. |
| `execution_policy.rs` | Timeouts, file split thresholds, retry limit config, and risk helper. |
| `concurrency_policy.rs` | Configured cap and effective-cap calculations. |
| `queue.rs` | Queue state, queue controls, capacity classification, local/provider queue decisions. |
| `retry.rs` | Structured retry coverage validation, retry prompt block, retry budget helpers. |
| `diagnostics.rs` | Aggregate runtime diagnostics and low-frequency final logging data. |
| `shared_context.rs` | Duplicate `Read`/`GetFileDiff` measurement and future evidence metadata helpers. |
| `incremental_cache.rs` | Per-session packet cache model and serialization. |
| `report.rs` | Deep Review-specific reliability signals and packet metadata helpers for `CodeReviewTool`. |
| `task_adapter.rs` | Deep Review-specific TaskTool orchestration hooks. |

`deep_review_policy.rs` should become a compatibility facade during migration, then shrink to re-exports or be removed after imports are updated.

### Generic Subagent Runtime Boundary

The first no-behavior generic runtime primitive is now present:

```text
src/crates/core/src/agentic/subagent_runtime/
  mod.rs
  queue_timing.rs
```

`queue_timing.rs` only handles queue elapsed time, pause/continue accounting, elapsed milliseconds, and expiration checks. It must stay free of Deep Review roles, queue reasons, provider classifications, retry policy, events, and report wording.

Introduce additional generic runtime areas only after Deep Review extraction proves stable and the primitive is proven generic:

```text
src/crates/core/src/agentic/subagent_runtime/
  capacity.rs
  queue_state.rs
  retry_admission.rs
```

Rules:

- Do not move behavior here until it is proven generic.
- Generic modules must not import Deep Review modules.
- Deep Review adapters may import generic modules.
- Provider-capacity auto queueing must not become global subagent behavior in this refactor.

Generic candidates:

- capacity acquisition/release guard;
- queue state shape independent of Deep Review labels;
- timeout separation between queue wait and running time;
- bounded retry admission primitives.

### Backend Tool Facades

Keep public tool entrypoints stable:

- `TaskTool` remains the registered Task tool.
- `CodeReviewTool` remains the registered report submission tool.

Move feature-specific logic behind adapters:

```rust
let deep_review_context = deep_review::manifest::Context::from_tool_context(context);
deep_review::task_adapter::prepare_launch(...);
deep_review::retry::validate_retry(...);
deep_review::queue::wait_for_reviewer_capacity(...);
```

```rust
deep_review::report::fill_packet_metadata(...);
deep_review::report::fill_reliability_signals(...);
deep_review::incremental_cache::persist_completed_packets(...);
deep_review::diagnostics::log_final_snapshot(...);
```

Required guardrail: normal Task and standard Code Review behavior must remain unchanged when Deep Review context is absent.

### Target Frontend Review-Team Layout

Split `reviewTeamService.ts` into a directory with a compatibility facade:

```text
src/web-ui/src/shared/services/review-team/
  index.ts
  types.ts
  defaults.ts
  strategy.ts
  pathMetadata.ts
  manifestMembers.ts
  workPackets.ts
  tokenBudget.ts
  risk.ts
  scopeProfile.ts
  evidencePack.ts
  cachePlan.ts
  preReviewSummary.ts
  promptBlock.ts
```

Keep this import path working:

```text
src/web-ui/src/shared/services/reviewTeamService.ts
```

The old file should become a facade exporting from `./review-team`.

Dependency rules:

- `types.ts` must be dependency-light and should not import implementation modules.
- `index.ts` may import config and agent APIs because it is the service facade for persisted config, backend default definition loading, default team assembly, and final manifest assembly.
- Pure helper modules must not import API adapters or `index.ts`.
- Prompt formatting consumes an already-built manifest and must not make launch policy decisions.
- Flow Chat launch modules should import the facade unless a tighter boundary is justified.

### Target Flow Chat Deep Review Layout

Split launch, action-bar, and report concerns:

```text
src/web-ui/src/flow_chat/deep-review/
  launch/
    commandParser.ts
    targetResolver.ts
    launchPrompt.ts
    launchErrors.ts
    DeepReviewService.ts
  action-bar/
    CapacityQueueNotice.tsx
    PartialResultsPanel.tsx
    RecoveryPlanPreview.tsx
    ReviewActionHeader.tsx
    RemediationSelectionPanel.tsx
    ReviewActionControls.tsx
    interruptionDiagnostics.ts
    actionBarFormatting.ts
    DeepReviewActionBar.tsx
  report/
    codeReviewReport.ts
    reliabilityNotices.ts
    manifestSections.ts
    reportSections.ts
    markdown.ts
```

Keep current public exports from:

- `DeepReviewService.ts`
- `DeepReviewActionBar.tsx`
- `codeReviewReport.ts`

Flow Chat ownership notes live in `src/web-ui/src/flow_chat/deep-review/README.md` and should be updated when responsibilities move.

### Refactor Execution Rounds

#### Refactor Round 0: Baseline And Guardrails

Actions:

- Record current line counts for oversized files.
- Run focused Deep Review tests.
- Confirm non-DeepReview impact inventory is current.
- Do not change behavior.

Verification:

- `pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts src/flow_chat/components/btw/DeepReviewActionBar.test.tsx src/flow_chat/utils/codeReviewReport.test.ts`
- `cargo test -p bitfun-core deep_review -- --nocapture`

#### Refactor Round 1: Backend Deep Review Module Extraction

Actions:

- Create `src/crates/core/src/agentic/deep_review/`.
- Move constants and team definitions first.
- Move execution policy and strategy helpers.
- Move concurrency, queue, diagnostics, shared context, retry, and cache one module at a time.
- Keep `deep_review_policy.rs` as compatibility facade.

Verification:

- Rust Deep Review tests.
- `rg -n "deep_review_policy::" src/crates/core/src` to confirm imports are intentional.

Behavior change allowed: none.

#### Refactor Round 2: TaskTool Adapter Extraction

Actions:

- Add `deep_review::task_adapter`.
- Move Deep Review context detection, packet id resolution, cache lookup, retry validation, retry prompt preparation, and queue/capacity calls behind the adapter.
- Add or preserve a non-DeepReview Task regression test.

Verification:

- Deep Review TaskTool tests.
- Non-DeepReview Task test proving queue/retry/cache paths do not run without Deep Review context.

Behavior change allowed: none.

#### Refactor Round 3: CodeReviewTool Report Adapter

Actions:

- Add `deep_review::report`.
- Move packet metadata fallback, reliability signals, token budget notes, diagnostics logging, and incremental cache write-through.
- Add or preserve standard Code Review regression tests.

Verification:

- Deep Review report tests.
- Standard Code Review test proving Deep Review metadata is absent outside Deep Review.

Behavior change allowed: none.

#### Refactor Round 4: Event And Tool Pipeline Containment

Actions:

- Keep current Deep Review queue event contract stable.
- Move payload conversion helpers into Deep Review modules.
- Replace inline Deep Review context propagation in `tool_pipeline.rs` with a small hook/helper.
- Keep duplicate read/diff measurement Deep Review-gated.

Verification:

- Queue event serialization tests.
- Tool pipeline non-DeepReview regression tests.

Behavior change allowed: none.

Deferred behavior change:

- Replacing `DeepReviewQueueStateChanged` with a generic `SubagentQueueStateChanged` event.

#### Refactor Round 5: Frontend Review Team Decomposition

Actions:

- Completed: `src/web-ui/src/shared/services/reviewTeamService.ts` exports from `./review-team`.
- Completed: `types.ts`, `defaults.ts`, `strategy.ts`, `pathMetadata.ts`, `manifestMembers.ts`, `workPackets.ts`, `tokenBudget.ts`, `risk.ts`, `scopeProfile.ts`, `evidencePack.ts`, `cachePlan.ts`, `preReviewSummary.ts`, and `promptBlock.ts` are separated under `review-team/`.
- Keep config persistence, backend definition loading, default team assembly, and final manifest assembly in `review-team/index.ts` until a later adapter-only split has equivalent tests and no API drift.
- Add new pure behavior to narrow helper modules first; do not grow `index.ts` with new policy logic.

Verification:

- `pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts`
- `pnpm run type-check:web`

Behavior change allowed: none.

#### Refactor Round 6: Flow Chat Deep Review Decomposition

Status: stable frontend cleanup complete for the current scope. Public facades remain unchanged; partial-results, recovery-plan preview, remediation selection, action controls, and diagnostics text building have been split into focused action-bar modules. Future action-bar cleanup should be limited to new density that appears after behavior work and should not be combined with behavior changes.

Actions:

- Completed: split command parsing, target resolution, launch prompt formatting, launch errors, and the remaining launch orchestrator.
- Completed: split capacity queue notice, review action header, and elapsed-time formatting.
- Completed: split reliability notices, manifest markdown, report section normalization, and markdown export.
- Completed follow-up: split remediation selection and diagnostics/action controls as no-behavior component/helper moves with focused tests.

Verification:

- `pnpm --dir src/web-ui run test:run -- src/flow_chat/services/DeepReviewService.test.ts src/flow_chat/components/btw/DeepReviewActionBar.test.tsx src/flow_chat/utils/codeReviewReport.test.ts`
- `pnpm run lint:web`
- `pnpm run type-check:web`

Behavior change allowed: none.

#### Refactor Round 7: Documentation, Comments, And Ownership Cleanup

Status: Flow Chat ownership cleanup is complete for the current frontend split. Backend module docs have a first pass covering Deep Review policy, budget, queue, manifest, cache, diagnostics, report, shared-context, tool-context, tool-measurement, and the new generic `subagent_runtime::queue_timing` primitive.

Actions:

- Completed for Flow Chat: add subsystem-level ownership notes covering launch, action-bar, report, facade guardrails, Deep Review gating, privacy constraints, and focused verification.
- Completed for Flow Chat: update status/planning docs without claiming new product behavior.
- Completed for backend first pass: add module-level Rust docs where Deep Review product policy, privacy/cache boundaries, report honesty, and the generic runtime boundary were not obvious.
- Future backend cleanup: add module-level Rust docs only when additional modules are touched and their responsibilities are not obvious.
- Future no-behavior cleanup: remove duplicated constants and wording only when a focused scan finds real duplication.

Verification:

- `rg -n "TODO|TBD|temporary|copy of|duplicate" src/crates/core/src/agentic/deep_review src/web-ui/src/shared/services/review-team`
- Focused frontend and Rust Deep Review tests.

Behavior change allowed: none.

### Refactor Behavior-Change Checkpoints

Stop and ask before doing any of the following:

1. Applying Deep Review queue behavior to all subagents.
2. Making provider transient errors auto-queue for ordinary subagents.
3. Replacing Deep Review-specific queue events with generic subagent queue events.
4. Changing retry from structured model/user-issued retry to backend-owned automatic redispatch.
5. Making backend risk scoring authoritative over user/team strategy.
6. Persisting review cache outside session metadata.
7. Hard-clipping prompt bytes or hiding files from coverage metadata.
8. Changing quick/normal/deep semantics beyond the cost-aware plan.

## Non-DeepReview Impact Requirements

Future rounds must preserve these rules:

1. Generic subagent runtime modules must not import Deep Review modules.
2. Deep Review adapters may import generic runtime modules.
3. Shared tools may call Deep Review adapters only after explicit context gating.
4. Standard Code Review must work without a Deep Review manifest.
5. Deep Review queue time must not become a global subagent timeout rule.
6. Provider capacity queueing must remain Deep Review-scoped unless product approves broader behavior.
7. Diagnostics must stay aggregate-only and content-free.

Required regression tests:

- Normal Task without `deep_review_run_manifest` does not apply Deep Review queue controls.
- Normal Task retry does not require Deep Review `retry_coverage`.
- Standard Code Review submission does not emit Deep Review packet/cache/queue metadata.
- Deep Review queue events serialize with the current stable shape.
- Tool pipeline duplicate-read measurement ignores non-DeepReview `Read` and `GetFileDiff` calls.
- Standard Code Review action bar renders without Deep Review queue controls.
- Deep Review queue controls render only for Deep Review state.
- Standard Code Review markdown export omits Deep Review manifest/cache sections.
- Review settings copy distinguishes Review Team max reviewers from global subagent concurrency.

## Cross-Cutting UX And I18n Requirements

- Every new user-facing string must be localized in `en-US`, `zh-CN`, and `zh-TW`.
- Prefer existing action bar and Review settings surfaces over new modals.
- Keep queue and retry notices compact.
- Do not show dense token/cost internals by default.
- Use default-collapsed details for reliability and coverage explanations.
- Preserve theme compatibility and compact layout behavior.
- Do not add visible text explaining implementation internals unless it helps the user make a decision.

## Performance And Privacy Requirements

- Diagnostics must be low-frequency.
- Runtime logs must be English-only and contain no emojis.
- Do not log or store source text, full diff text, reviewer output, provider raw body, or full file contents in diagnostics.
- Shared evidence must stay compact and metadata-first.
- Queue and retry automation must be bounded by settings and budgets.
- Large-change cost reduction must be transparent through coverage metadata.

## Final Release Gate

Run after any batch that completes pending functionality or refactor rounds:

```powershell
cargo test -p bitfun-core deep_review -- --nocapture
cargo check --workspace
cargo test --workspace
pnpm run lint:web
pnpm run type-check:web
pnpm --dir src/web-ui run test:run
git diff --check
```

If backend, desktop API, or Tauri adapters are touched, also run the nearest desktop/backend verification required by `AGENTS.md`.

## Stop Conditions

Stop and re-review the design if:

- a fix requires changing global `ai.subagent_max_concurrency` as the normal Deep Review recovery path;
- diagnostics need to store source, diff, reviewer output, provider raw body, or full file contents;
- provider queue needs more than one automatic reattempt per reviewer packet;
- auto retry needs a scope that is not smaller than the original packet;
- quick/default cost reduction would hide changed files instead of marking reduced-depth coverage;
- shared evidence reuse requires full `Read` output caching before duplicate-call diagnostics justify it;
- a UI change needs a new page or modal when the action bar or Review settings can carry the workflow;
- refactor work changes behavior outside Deep Review without a confirmed checkpoint.

## Expected End State

When all pending functional and refactor work is complete:

- Provider transient queue is short, visible, bounded, and controllable.
- Retry defaults to explicit user action, and opted-in automatic retry is bounded.
- Quick/default reviews reduce time and token cost by focusing on risk, while `deep` remains full-depth.
- Reviewers start from a compact shared evidence pack.
- Project-level cache, full tool-result reuse, hard prompt clipping, and DAG scheduling remain deferred unless separately approved.
- Deep Review backend logic lives in a dedicated module tree.
- Shared TaskTool, CodeReviewTool, tool pipeline, and event code contain only thin Deep Review hooks.
- Frontend review-team and Flow Chat Deep Review code are split by responsibility with stable facades.
- Non-DeepReview behavior is covered by focused regression tests.
- Documentation and code status remain aligned.

# Deep Review Completed Status

## Purpose

This document consolidates the completed Deep Review work from the current design and phase documents into one standalone status reference. It separates verified implementation from planned or deferred work so future changes can be reviewed against the actual product boundary.

## Source Documents

This consolidation covers the following current Deep Review documents and companion local design notes:

| Source | Role in this consolidation |
|---|---|
| `docs/deep-review-design.md` | Original strategy-engine, Architecture Reviewer, Frontend Reviewer, prompt ownership, and current implementation status. |
| `docs/deep-review-phase2-plan.md` | Phase 2 implementation status for strategy, concurrency, retry, cache, token budget, and report reliability work. |
| `docs/deep-review-phase2-addendum.md` | Truth model, status wording, risk register, completed rounds, and deferred boundaries. |
| `docs/deep-review-phase3-followup-plan.md` | Latest product decisions and the Phase 3 lineage for diagnostics, settings, provider queue/retry controls, cost-aware scope, and shared evidence pack work. |
| `docs/deep-review-architecture-refactor-plan.md` | Architecture refactor goals and module boundaries. Backend module extraction, Flow Chat compatibility facades, frontend review-team pure-helper split, and the stable first Flow Chat launch/action-bar/report split are implemented; broader shared-runtime refactors remain bounded by that plan. |
| `docs/deep-review-nondeepreview-impact-inventory.md` | Shared-runtime impact rules that are already documented and must continue to constrain future work. |
| `docs/superpowers/plans/2026-05-09-deep-review-phase3-execution-plan.md` | Round-level execution status and verification history for Phase 3. |
| `docs/superpowers/specs/deep-review-design.md` and `docs/superpowers/plans/deep-review-phase2-plan.md` | Local companion copies checked for drift; they duplicate the main design and Phase 2 plan shape and do not add a different current boundary. |

## Status Wording

Use the wording below when updating or reviewing Deep Review work:

| Wording | Meaning |
|---|---|
| Implemented | Runtime behavior exists in code and has focused verification. |
| Implemented with guardrails | Behavior exists but is intentionally bounded by settings, budgets, user controls, or trust metadata. |
| Safety net | Runtime blocks or reports an unsafe condition, but does not provide the full smoother product behavior. |
| Prompt-guided | The manifest or prompt asks the orchestrator to perform the step; weak models can still miss or mis-sequence it. |
| Deferred by product decision | Implementation is intentionally blocked until privacy, retention, UX, or product rules are approved. |
| Pending implementation | The design is agreed but code has not landed. |

This document only lists implemented or implemented-with-guardrails behavior. Pending and deferred work is tracked in `docs/deep-review-pending-plan.md`.

## Current Runtime Shape

Deep Review remains a prompt-driven 5-phase orchestrator:

1. Scope identification.
2. Parallel specialist review.
3. Sequential judge/quality gate.
4. Final report synthesis through `submit_code_review`.
5. Optional remediation through normal editing tools.

The current reviewer team contains:

| Reviewer | Runtime status | Scope |
|---|---|---|
| `ReviewBusinessLogic` | Always on | Correctness, business rules, data/state transitions. |
| `ReviewPerformance` | Always on | Runtime hot paths, expensive computations, payload cost. |
| `ReviewSecurity` | Always on | Exploitable trust-boundary, auth, data-handling, and security risks. |
| `ReviewArchitecture` | Always on | Layer boundaries, dependency direction, API contract shape, maintainability. |
| `ReviewFrontend` | Conditional | React, UI state, i18n, accessibility, frontend-backend contract drift, and frontend platform boundaries. |
| `ReviewJudge` | Sequential | Deduplicates, resolves overlap, validates evidence, and synthesizes the report. |
| Custom review agents | Optional | Must satisfy the minimum review-agent tooling contract. |

The orchestrator is still source-agnostic. Git-backed changes, local workspace changes, or future sources should flow through a target manifest rather than assuming one Git-only abstraction.

## Completed Reviewer And Prompt Work

### Core Role Expansion

- `ReviewArchitecture` is implemented as an always-on core reviewer.
- `ReviewFrontend` is implemented as a frontend-focused reviewer with conditional activation.
- Dedicated prompts exist for Architecture and Frontend reviewers.
- Existing prompts were narrowed to reduce overlap:
  - Business Logic does not own UI state or layer-boundary analysis.
  - Performance does not own React render optimization.
  - Security focuses on exploitable risks rather than general structural boundary violations.
- The Judge prompt handles cross-reviewer overlap for Architecture/Business Logic, Architecture/Security, Frontend/Performance, and Frontend/Business Logic.
- The DeepReview orchestrator prompt contains role-specific strategy amplification and frontend strategy directives.

### Role Metadata, Visibility, And I18n

- Backend-provided reviewer definitions are the runtime source for frontend team resolution and review-agent visibility.
- Frontend fallback metadata remains only as degraded-mode safety behavior.
- Settings and Agents page i18n include Architecture and Frontend reviewer names.
- The Agents page Code Review Team card was adjusted to avoid clipped reviewer tags and to present a compact role summary.
- Hidden review-agent metadata is derived dynamically so review agents can stay hidden from normal agent pickers while still being visible in Review Team surfaces.

## Completed Target Classification And Conditional Dispatch

- `ReviewFrontend` dispatch changed from always-present execution to conditional execution.
- Conditional activation is driven by target/domain classification and a reviewer applicability registry, not scattered hardcoded file checks.
- The current classifier supports frontend UI, frontend style, frontend i18n, frontend contract, desktop contract, backend core, API layer, transport, and other domain tags.
- `hasFrontendFiles()` remains backward compatible by deriving from frontend-related tags.
- The same registry is intended to support future conditional reviewers.
- Custom review subagents are included only when valid and applicable; invalid custom review agents remain explainable instead of silently disappearing.

## Completed Custom Review-Agent Contract

- The minimum valid custom review-agent tool set is centralized as:
  - `GetFileDiff`
  - `Read`
- Missing required tools are reported as `invalid_tooling`.
- Missing recommended investigation tools such as `Grep`, `Glob`, or `LS` are treated as degraded review quality, not invalid configuration.
- The UI and runtime share the same contract definition so create/edit affordances and Review Team enforcement do not drift.
- Invalid or skipped reviewers are surfaced in manifest/report metadata rather than being filtered out before the user can understand why they did not run.

## Completed Strategy And Risk Metadata

- Backend `ChangeRiskFactors` and `auto_select_strategy()` exist as pure policy helpers.
- Launch manifests record:
  - frontend recommendation;
  - backend-compatible recommendation;
  - explicit user override;
  - final strategy;
  - mismatch state;
  - mismatch severity.
- Backend scoring is advisory and mismatch-warning only.
- Backend scoring does not override the selected strategy, expand the reviewer roster, or change token/concurrency cost.
- `max_cyclomatic_complexity_delta` remains explicitly `not_measured`; authoritative strategy selection is not implemented.

## Completed Predictive Timeout And Partial Result Capture

- Launch manifests record target file count and diff line stats.
- Effective reviewer and judge timeouts are derived from strategy and target size.
- TaskTool honors the effective manifest policy when launching Deep Review reviewer subagents.
- `SubagentResultStatus::PartialTimeout` exists.
- The coordinator can preserve a timed-out subagent final message when it arrives inside the configured grace period.
- The limitation is explicit: arbitrary stream fragments are not reconstructed outside the grace window.

## Completed Concurrency And Queue Foundation

### Runtime Enforcement

- `DeepReviewConcurrencyPolicy` parsing exists.
- TaskTool bounded-waits for local reviewer-cap saturation.
- Queue time is separated from reviewer runtime time.
- Expired local-cap waits can become `CapacitySkipped`.
- Turn-local effective concurrency learning lowers capacity after local capacity skips and explicit provider transient-capacity reviewer failures.
- Successful reviewer observations can cautiously recover the effective cap.
- Capacity skips are folded into final report reliability signals.

### User-Visible Queue Controls

- A backend queue-state event contract exists.
- Compact queue notices exist in the Flow Chat action-bar path.
- Backend-bound local-cap queue controls exist for:
  - pause;
  - continue;
  - cancel;
  - optional-extra skip.
- Launch-time active-session concurrency warning exists so Deep Review does not silently compete with a busy user session.
- Recovery actions include running slower next time and opening Review settings.

### Current Boundary

- Current queue automation is narrow and Deep Review reviewer-oriented.
- Local-cap waits are bounded, visible, pauseable, continuable, cancellable, and optional-extra skippable.
- Local reviewer queue liveness now waits while the same Deep Review still has an active reviewer; a fixed short wait window is not treated as a per-reviewer expiry.
- Explicit provider transient-capacity reviewer failures now enter a short bounded provider queue and reattempt once before final `capacity_skipped`.
- Provider queue/retry/success counts and provider reason counts are aggregate diagnostics.
- Provider queue time remains separated from reviewer runtime timeout.
- Generic runtime containment has a first no-behavior primitive: `agentic/subagent_runtime/queue_timing.rs` owns queue wait / pause timing only.
- Full backend batch/stagger scheduling is pending; a narrow launch-batch overlap guard now queues later batches with `launch_batch_blocked` while an earlier reviewer batch is still active.
- User-facing effective-cap override controls are pending.
- Deep Review queueing is not global subagent queueing.

## Completed Retry Guardrails

- Retry budget tracking exists.
- Reviewer timeout retry guidance exists.
- Retry guidance uses the effective manifest policy when available.
- TaskTool structured retry admission exists.
- A retry reviewer Task must include structured coverage and pass runtime checks:
  - `retry: true`;
  - source packet/status information;
  - retryable source status;
  - reduced retry scope;
  - lower timeout;
  - available retry budget.
- Accepted retry Tasks receive a bounded retry-scope prompt block.
- Missing coverage, broad scope, non-retryable status, non-lowered timeout, and exhausted budget are rejected.
- User-facing explicit retry action exists for structured unresolved Deep Review slices.
- Manual retry is enabled only for `partial_timeout` or transient `capacity_skipped` sources with explicit reduced retry scope.
- Bounded automatic retry admission is guarded by Review Team opt-in, structured coverage, reduced scope, lower timeout, retry budget, and elapsed guard.
- Bounded automatic retry remains disabled by default.
- Backend-owned automatic redispatch scheduling remains pending; `auto_retry` is only an admission path for backend-owned retry callers, not a general scheduler.

## Completed Incremental Cache Boundary

- Per-session `DeepReviewIncrementalCache` primitives exist.
- Session metadata contains the cache field.
- Existing persistence preserves the cache field.
- TaskTool can read a matching per-session cache hit by resolved `packet_id`.
- `submit_code_review` can write completed reviewer output back to the per-session cache.
- Read/write paths align on work-packet `packet_id`.
- Report reliability signals can show cache hit/miss behavior.
- The current cache has no independent retention period beyond session metadata.
- Deleting or clearing session metadata removes this cache.
- Project-level or cross-session cache is not implemented.

## Completed Packet Metadata And Report Reliability

- `submit_code_review` has packet metadata fallback.
- Missing reviewer `packet_id` values can be inferred from the manifest when possible.
- Lower-confidence fallback metadata is marked as such.
- Final report reliability signals cover:
  - partial timeout;
  - retry guidance;
  - skipped reviewers;
  - capacity/concurrency limits;
  - cache hits/misses;
  - token-budget tradeoffs.
- Report/export utilities keep dense reliability details collapsed or summarized.
- Standard Code Review should not receive Deep Review-only packet/cache/queue signals unless Deep Review context is present.

## Completed Shared-Context Measurement

- Deep Review reviewer `Read` and `GetFileDiff` calls are measured by parent turn, reviewer type, tool name, normalized path, call count, and reviewer count.
- Measurement is content-free.
- Measurement does not store source text, diff text, tool output, model output, or provider raw body.
- Final Deep Review submission can emit aggregate debug diagnostics once.
- The report remains free of raw shared-context diagnostics.
- Programmatic cross-subagent tool-result reuse is not implemented.

## Completed Token And Context Budget Guardrails

- Launch manifests include heuristic per-mode prompt-byte thresholds.
- Manifests include estimated maximum reviewer prompt bytes.
- Summary-first full-scope metadata exists.
- File split and max-file style guardrails exist.
- Summary-first behavior keeps every assigned file visible; it must not silently hide files from coverage metadata.
- Hard prompt-byte clipping and byte-accurate enforcement remain deferred.

## Completed Cost-Aware Scope And Shared Evidence Pack

- Launch manifests include `DeepReviewScopeProfile`.
- `quick` maps to `high_risk_only`.
- `normal` maps to `risk_expanded`.
- `deep` maps to `full_depth`.
- Reduced-depth runs emit or infer `reduced_scope` reliability signals and must not claim full-depth coverage.
- Changed-file coverage metadata remains visible for reduced-depth runs.
- Launch manifests include metadata-first `DeepReviewEvidencePack` v1.
- Evidence packs include version, source label, changed-file metadata, diff stats, domain/risk tags, packet ids, hunk hints, contract hints, budget, and privacy boundary.
- Evidence packs are `metadata_only` and do not contain source text, full diff, model output, provider raw body, or full file contents.
- Reviewer and judge prompts treat hunk/contract hints as orientation only; findings must be confirmed with `GetFileDiff`, `Read`, `Grep`, or read-only `Git`.
- Rust manifest parsing validates evidence pack version, source, size limits, privacy boundary, and forbidden content keys.
- Runtime diagnostics include a content-free duplicate discovery savings candidate count for later evidence-pack impact comparison.
- Report/export utilities show only evidence pack aggregate counts, source, and privacy boundary.
- Programmatic full tool-result cache remains unimplemented.

## Completed Consent, Recovery, And Settings UX

### First-Run And Launch UX

- The Deep Review consent dialog includes a compact pre-review summary:
  - file count;
  - risk areas;
  - selected strategy;
  - optional reviewer count;
  - summary-first state;
  - skipped reviewer warnings when present.
- The dialog copy was intentionally reduced to key reminders.
- User-facing copy is localized.
- Dense lineup/cost cards remain deferred.

### Action Bar And Recovery

- Deep Review action-bar surfaces support interruption/recovery states.
- Manual cancellation preserves parent summary rather than treating every stop as full review loss.
- Continue/resume controls remain visible when recovery is possible.
- Diagnostics copy actions preserve raw diagnostic metadata while keeping user-facing copy localized.

### Review Capacity And Retry Settings

- Default Review Team config stores:
  - `max_parallel_reviewers`;
  - `max_queue_wait_seconds`;
  - `allow_provider_capacity_queue`;
  - `allow_bounded_auto_retry`;
  - `auto_retry_elapsed_guard_seconds`.
- Defaults remain conservative:
  - 4 parallel reviewers;
  - 60 seconds max queue wait;
  - provider capacity queue allowed by policy and bounded to one short reattempt;
  - bounded automatic retry disabled by default;
  - 180 seconds elapsed guard.
- Controls are scoped to Review Team settings.
- They do not change global `ai.subagent_max_concurrency`.

## Completed Adaptive Runtime Support

- Context health snapshot support exists for degraded long-running sessions.
- Model capability profile support exists for weaker model handling.
- Runtime policy can adapt context profile behavior based on model capability and session health.
- This is a guardrail layer, not a replacement for user-selected review strategy.

## Completed Compression Contract Integration

- `CompressionContract` and conversion from `EvidenceLedgerSummary` are complete.
- The compressor prompt already injects contract content.
- No additional Deep Review implementation is currently needed for this item.

## Completed Non-DeepReview Impact Documentation

The shared-impact inventory is documented and must remain active during future work:

| Shared area | Completed boundary |
|---|---|
| `TaskTool` | Deep Review queue, retry, packet, and cache logic must stay behind explicit Deep Review context checks. |
| `tool_pipeline.rs` | Duplicate `Read`/`GetFileDiff` measurement is Deep Review-gated and content-free. |
| `CodeReviewTool` | Deep Review report enrichments are gated by Deep Review context. |
| `bitfun-events` | Current Deep Review queue event is stable and domain-specific; generic event replacement is a future product/API decision. |
| Session metadata | Deep Review cache is per-session and absent for non-DeepReview sessions. |
| Review action bar | Queue/recovery panels render only for Deep Review state. |
| Report utilities | Manifest/cache/token-budget sections remain optional and Deep Review-gated. |
| Review settings | Review Team capacity settings are labeled as review-scoped, not global subagent concurrency. |

## Verification History Recorded In Source Docs

The source documents record focused and release-gate verification, including:

- `cargo test -p bitfun-core deep_review -- --nocapture`
- `cargo check --workspace`
- `cargo test --workspace`
- `pnpm run lint:web`
- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run test:run`
- focused frontend tests for `reviewTeamService`, Deep Review action bar/store, queue events, and report utilities;
- focused Rust tests for runtime diagnostics, cache behavior, retry admission, queue/capacity behavior, and report reliability.

The latest merge verification has run the full Rust gate: `cargo test -p bitfun-events deep_review_queue_state_event_serializes_stable_contract -- --nocapture`, `cargo test -p bitfun-core deep_review -- --nocapture`, `cargo check --workspace`, and `cargo test --workspace` all pass. A real Deep Review manual smoke / E2E pass remains a separate pre-release product acceptance step.

## Completed Boundary Summary

Deep Review has moved from a prompt-only concept to a guarded runtime with:

- always-on architecture review;
- conditional frontend review;
- backend-provided team definitions;
- data-driven reviewer applicability;
- explainable custom-reviewer validation;
- advisory strategy metadata;
- predictive timeouts;
- partial-timeout final-message capture;
- local-cap queue controls;
- bounded provider-capacity queue with one reattempt;
- structured retry admission;
- explicit manual retry action for structured unresolved slices;
- guarded `auto_retry` admission with default-off Review Team settings;
- per-session packet cache;
- packet fallback;
- report reliability signals;
- content-free duplicate-tool diagnostics;
- cost-aware reduced-depth scope profiles;
- metadata-only shared evidence packs;
- content-free duplicate discovery savings diagnostics;
- compact evidence-pack report/export summaries;
- compact launch summary;
- review-scoped capacity and retry settings;
- no-behavior frontend review-team helper extraction with stable `reviewTeamService.ts` facade;
- no-behavior Flow Chat Deep Review helper extraction with stable `DeepReviewService.ts`, `DeepReviewActionBar.tsx`, and `codeReviewReport.ts` facades;
- Flow Chat Deep Review ownership documentation covering launch, action-bar, report, facade, privacy, and focused verification boundaries.

The completed boundary intentionally stops before backend-owned retry redispatch scheduling, full backend batch/stagger scheduling, project-level cache, hard byte clipping, programmatic shared tool-result reuse, global subagent scheduling, and additional shared-runtime architecture behavior changes. Local queue liveness is complete for the current reviewer-admission path; the current launch-batch guard is only an overlap safety net, not a complete dispatcher.

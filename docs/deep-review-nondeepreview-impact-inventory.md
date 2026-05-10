# Deep Review Refactor Non-DeepReview Impact Inventory

## Purpose

This inventory lists the shared areas touched by the current Deep Review work where future refactoring could affect non-DeepReview behavior. It supports the architecture rule that subagent runtime changes must not silently become Deep Review-specific or unexpectedly alter ordinary subagents.

## Current Shared Impact Areas

| Area | Current Deep Review change | Non-DeepReview risk | Required mitigation |
|---|---|---|---|
| `TaskTool` | Deep Review reviewer capacity queueing, provider short queue/one reattempt, effective concurrency learning, provider capacity skip conversion, structured retry admission, guarded `auto_retry` admission, packet/cache lookup | Ordinary hidden subagents could accidentally enter queue/retry/cache behavior if context gating is wrong | Keep all Deep Review logic behind explicit agent type/manifest checks; add regression tests for ordinary Task calls. |
| `tool_pipeline.rs` / `tools/framework.rs` | `tool_pipeline.rs` propagates Deep Review context variables through `deep_review::tool_context`; `tools/framework.rs` records successful duplicate `Read`/`GetFileDiff` measurements through `deep_review::tool_measurement` | Generic tool execution can become feature-aware and harder to reuse | Keep both calls as thin Deep Review hooks; do not inline policy, cache, retry, report, or queue behavior in shared tool execution. |
| `CodeReviewTool` | Deep Review packet metadata fallback, reliability signals, runtime diagnostics, incremental cache write-through | Standard Code Review reports could gain Deep Review-only reliability or cache behavior | Gate enrichments by Deep Review context and add standard Code Review regression tests. |
| `bitfun-events` / agentic events | Adds Deep Review queue state event payload | Event enum becomes increasingly domain-specific | Keep current event stable for compatibility; only design generic subagent queue events after product/API review. |
| Session metadata | Adds Deep Review run manifest and per-session cache fields | Session metadata can accumulate feature-specific blobs | Keep cache per-session, content-bounded, and absent for non-DeepReview sessions. |
| Review action bar store/component | Shared `ReviewActionBar` path now includes Deep Review queue, recovery, remediation, and diagnostics affordances | Standard Code Review UI can inherit irrelevant Deep Review states or lose standard remediation behavior | Keep queue/recovery panels Deep Review-gated; keep remediation selection/action controls covered by standard-review regression tests. |
| Report utilities | Shared code review report helpers render manifest/cache/token-budget sections | Standard Code Review exports can become noisy or show irrelevant Deep Review sections | Keep manifest sections optional and Deep Review-gated. |
| Review settings | Adds Deep Review capacity/retry settings under Review config | Users may confuse Deep Review reviewer concurrency with global subagent concurrency | Label settings as Review Team scoped; keep global `ai.subagent_max_concurrency` out of normal Review settings. |

## Latest M1-M3 Implementation Impact

| Shared file or area touched | Change type | Non-DeepReview behavior risk | Regression evidence |
|---|---|---|---|
| `src/crates/core/src/agentic/tools/implementations/task_tool.rs` | Deep Review adapter extraction, queue/capacity handling, retry admission, retry guidance and packet/cache gates | Normal Task could accidentally enter Deep Review queue/retry/cache behavior or receive Deep Review retry guidance | Rust non-DeepReview Task and Deep Review focused tests were added in earlier milestones; post-review regression `deep_review_retry_guidance_only_applies_to_initial_reviewer_timeout` keeps retry guidance Deep Review-gated. The next full Rust gate is deferred to the agreed cargo pass. |
| `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs` | Deep Review packet metadata, reliability signals, cache write-through, evidence-pack validation signal | Standard Code Review could receive Deep Review-only report metadata | Rust report tests cover standard submission and Deep Review enrichment boundaries from earlier milestones. The next full Rust gate is deferred to the agreed cargo pass. |
| `src/crates/core/src/agentic/deep_review/*` | Deep Review subsystem ownership for policy, queue, diagnostics, manifest, report, task adapter and cache | Future contributors could bypass explicit Deep Review gates | Module-level tests and facade compatibility tests were added in earlier milestones. The next full Rust gate is deferred to the agreed cargo pass. |
| `src/crates/core/src/agentic/mod.rs` and `src/crates/core/src/agentic/subagent_runtime/*` | Adds a `pub(crate)` generic runtime area with a queue timing primitive used by Deep Review queue waits | Future generic modules could accidentally absorb Deep Review product policy or make queueing global | `subagent_runtime` must not import Deep Review modules; current static import scan found no Deep Review dependency in the new runtime directory. Full Rust verification is deferred to the next agreed cargo pass. |
| `src/web-ui/src/shared/services/review-team/*` | Review Team service has focused pure-helper modules for path metadata, manifest-member projection, scope profile, evidence pack, work packets, token budget, cache plan, pre-review summary, risk, and prompt block formatting | Shared frontend service could change standard review/team behavior or leak content into prompt metadata if helper boundaries drift | Current no-behavior refactor passed `pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts` and `pnpm run type-check:web`; previous release gate also covered `pnpm run lint:web`. |
| `src/web-ui/src/flow_chat/deep-review/report/*` and `src/web-ui/src/flow_chat/utils/codeReviewReport.ts` facade | Deep Review markdown export includes manifest, scope profile, evidence pack and reliability summaries; reliability, manifest, section, and markdown helpers are now split | Standard Code Review export could include Deep Review-only manifest/cache/evidence sections | `pnpm --dir src/web-ui exec vitest run src/flow_chat/deep-review/report/manifestSections.test.ts src/flow_chat/deep-review/report/reliabilityNotices.test.ts src/flow_chat/deep-review/report/markdown.test.ts src/flow_chat/utils/codeReviewReport.test.ts`; export privacy grep found no content-field output in the report formatter. |
| `src/web-ui/src/flow_chat/deep-review/launch/*` and `src/web-ui/src/flow_chat/services/DeepReviewService.ts` facade | Deep Review launch parsing, target resolution, prompt formatting, and launch errors are split from child-session orchestration | Standard Code Review launch/session behavior could be affected if shared Flow Chat services are called differently | Focused launch tests passed for command parser, target resolver, launch errors, launch prompt, and `DeepReviewService.test.ts`; no standard Code Review entrypoint was changed. |
| `src/web-ui/src/flow_chat/deep-review/action-bar/*` and `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx` facade | Deep Review capacity queue notice, partial-results panel, recovery-plan preview, review action header, formatting helpers, remediation selection, action controls, and diagnostics text builder are split from the shared action-bar component | Standard Code Review action bar could show Deep Review queue controls or lose remediation behavior | `pnpm --dir src/web-ui exec vitest run src/flow_chat/deep-review/action-bar/RemediationSelectionPanel.test.tsx src/flow_chat/deep-review/action-bar/ReviewActionControls.test.tsx src/flow_chat/deep-review/action-bar/interruptionDiagnostics.test.ts src/flow_chat/components/btw/DeepReviewActionBar.test.tsx`; standard review remediation test remains covered. |
| `src/web-ui/src/flow_chat/deep-review/README.md` | Documents Flow Chat Deep Review ownership, facade guardrails, Deep Review gating, privacy boundaries, and focused verification | Documentation drift could mislead future shared UI/report changes | Keep this README updated in the same commit whenever launch, action-bar, or report responsibilities move. |
| `src/web-ui/src/flow_chat/tool-cards/*` and consent/action surfaces | Reduced-depth and retry/queue notices remain compact and Deep Review scoped | Standard Code Review UI could show Deep Review controls or dense internals | Focused component tests cover tool card, consent dialog, and action-bar surfaces; rerun full web suite at the final release gate. |
| `src/web-ui/src/locales/{en-US,zh-CN,zh-TW}/flow-chat.json` | Queue/retry/reduced-depth/evidence-facing copy | Missing locale could expose raw keys or inconsistent UX | `pnpm --dir src/web-ui exec vitest run src/shared/services/reviewTeamLocaleCompleteness.test.ts`; `pnpm run lint:web`; `pnpm run type-check:web`. |

## Safe Refactor Rules

1. Generic subagent runtime modules must not import Deep Review modules.
2. Deep Review adapters may import generic runtime modules.
3. Shared tools may call Deep Review adapters only after context gating.
4. Standard Code Review must continue to work without a Deep Review manifest.
5. Deep Review queue time must not become a global subagent timeout rule unless explicitly approved.
6. Provider capacity requeue must remain Deep Review-scoped until product confirms broader behavior.
7. Diagnostics must stay aggregate-only and content-free.
8. Adding `subagent_runtime` primitives does not imply generic global subagent queue behavior is implemented.

## Regression Tests To Keep Or Add

### Backend

- A normal `Task` tool call without `deep_review_run_manifest` does not apply Deep Review queue controls.
- A normal `Task` tool retry does not require Deep Review `retry_coverage`.
- A normal `Task` tool call is not affected by Deep Review `auto_retry` admission unless the parent agent is Deep Review.
- Standard `CodeReviewTool` submission does not emit Deep Review packet metadata, cache hit/miss, or queue reliability signals.
- Deep Review queue events serialize with the existing stable event shape.
- Tool pipeline duplicate-read measurement ignores non-DeepReview `Read` and `GetFileDiff` calls.

### Frontend

- Standard Code Review action bar renders without capacity queue controls.
- Deep Review capacity queue controls render only when the store has Deep Review queue state.
- Standard Code Review markdown export omits Deep Review manifest/cache sections.
- Review settings copy distinguishes Review Team max reviewers from global subagent concurrency.

## Behavior Changes That Need User Confirmation

The following are not safe as pure refactors:

1. Applying Deep Review capacity queueing to all subagents.
2. Making provider transient errors auto-queue for ordinary subagents.
3. Replacing Deep Review-specific queue events with generic subagent queue events.
4. Persisting Deep Review cache at project level.
5. Auto-retrying reviewer packets without explicit structured coverage and budget guards.
6. Making backend strategy recommendations override user-selected strategy.

## Documentation Follow-Up

If any refactor round touches one of the shared areas above, update this document in the same commit with:

- the exact shared file touched;
- whether behavior changed or only ownership changed;
- the regression test that proves non-DeepReview behavior stayed stable;
- any product decision still required.

# Deep Review Architecture Refactor Plan

## Scope

This plan reviews the Deep Review-related changes in the local branch from commit `fce420c87284b8534cae657fce07bd8c6fb9e3ef` through `HEAD`. It is a design and execution plan only. It must not be treated as approval to change runtime behavior.

The branch range contains unrelated product and packaging changes. This document focuses only on Deep Review surfaces: review team construction, target classification, launch manifests, TaskTool reviewer execution, queue/capacity handling, retry admission, incremental cache, diagnostics, report shaping, consent UI, and Flow Chat recovery/action surfaces.

## Refactor Goals

1. Move Deep Review-specific logic out of broad shared files where possible.
2. Separate generic subagent runtime primitives from Deep Review policy adapters.
3. Keep standard subagent behavior stable unless a change is explicitly reviewed as a product decision.
4. Reduce oversized files and repeated definitions.
5. Preserve existing Deep Review behavior during refactor rounds.
6. Keep dependencies acyclic and location choices predictable.
7. Make any non-Deep Review impact explicit and testable.
8. Keep frontend and backend Deep Review boundaries clear.
9. Avoid new performance, quality, or security risks.

## Current Change Surface

### Backend Core

| File | Current line count | Deep Review responsibility currently present | Refactor pressure |
|---|---:|---|---|
| `src/crates/core/src/agentic/deep_review/` | Split modules | Team definition, constants, manifest parsing, execution policy, concurrency policy, queue controls, budget tracking, diagnostics, shared-context measurement, incremental cache, report helpers, task adapter helpers | Current subsystem home. New work should prefer these modules over adding Deep Review logic to broad tool files. |
| `src/crates/core/src/agentic/deep_review_policy.rs` | ~1483 | Compatibility facade, global tracker accessors, config loading, public re-exports, and legacy tests | Medium. The major subsystem extraction is complete, but this facade still needs gradual shrinkage as imports move to module paths. |
| `src/crates/core/src/agentic/tools/implementations/task_tool.rs` | 2245 | Generic Task tool plus Deep Review reviewer cap waits, retry admission, packet/cache lookup, provider capacity skip, queue events, tests | Very high. Shared subagent execution is coupled to Deep Review behavior. |
| `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs` | 1894 | Code review submission plus Deep Review packet fallback, reliability signals, runtime diagnostics, cache write-through, report schema tests | High. Standard Code Review and Deep Review report behavior share one tool. |
| `src/crates/core/src/agentic/tools/pipeline/tool_pipeline.rs` | 1363 | Generic tool pipeline plus Deep Review context propagation and duplicate `Read`/`GetFileDiff` measurement | Medium. Deep Review metadata leaks into a generic pipeline. |
| `src/crates/events/src/agentic.rs` | Not measured here | Adds Deep Review queue event contract | Medium. Event is domain-specific in a shared event crate. |

### Frontend

| File | Current line count | Deep Review responsibility currently present | Refactor pressure |
|---|---:|---|---|
| `src/web-ui/src/shared/services/review-team/` | Split modules | Defaults, strategy profiles, public types, path metadata, risk scoring, work packets, token budget, scope profile, evidence pack, cache plans, pre-review summary, prompt block formatting, manifest-member projection, and the review-team service facade | Current subsystem home. The public import path is backed by a directory; new pure behavior should land in the narrow helper module instead of growing `index.ts`. |
| `src/web-ui/src/flow_chat/services/DeepReviewService.ts` / `src/web-ui/src/flow_chat/deep-review/launch/DeepReviewService.ts` | 2 / 338 | Compatibility facade plus launch orchestration. Command parsing, target resolution, launch prompt formatting, and launch error shaping now live in focused modules. | Lower. Keep child-session creation and runtime-signal manifest assembly in the orchestrator unless a later no-behavior adapter split has equivalent tests. |
| `src/web-ui/src/flow_chat/components/btw/DeepReviewActionBar.tsx` / `src/web-ui/src/flow_chat/deep-review/action-bar/DeepReviewActionBar.tsx` | 5 / 785 | Compatibility facade plus shared review action bar. Capacity queue notice, partial-results panel, recovery-plan preview, header, elapsed-time formatting, remediation selection, action controls, and diagnostics text building are split; the main file owns state orchestration and callback wiring. | Lower. Continue only with small no-behavior components if future behavior work makes the action bar dense again. |
| `src/web-ui/src/flow_chat/utils/codeReviewReport.ts` / `src/web-ui/src/flow_chat/deep-review/report/codeReviewReport.ts` | 2 / 423 | Compatibility facade plus retry-slice helpers and shared report types. Reliability notices, manifest markdown, report sections, and markdown export are split. | Lower. New report behavior should land in the narrow helper module instead of regrowing the facade/core type file. |
| `src/web-ui/src/shared/services/reviewTargetClassifier.ts` | 319 | Source-agnostic target classification and reviewer applicability registry | Good candidate to keep as an independent module. |
| `src/web-ui/src/shared/services/reviewSubagentCapabilities.ts` | 43 | Shared tool contract for custom review agents | Good candidate to keep as common review-team support. |

## Architectural Problems

### 1. Deep Review Is A Subsystem, But Backend Code Is Still File-Oriented

`deep_review_policy.rs` used to contain independent concepts that now live mostly under `agentic/deep_review/`:

- role and team definition
- manifest parsing
- execution policy and predictive timeout
- concurrency policy
- queue controls
- effective concurrency learning
- retry budget/admission data
- runtime diagnostics
- shared-context measurement
- incremental cache

The remaining facade still increases merge risk when new logic lands there. Future work should treat `deep_review_policy.rs` as compatibility glue and add behavior to the narrower modules instead.

### 2. Shared Subagent Execution Has Deep Review Branches

`TaskTool` is the canonical route for hidden subagent execution, not only Deep Review. Current Deep Review additions are mostly gated by manifest/context, but the implementation details live directly inside the generic tool. This makes it too easy for future queue/retry behavior to accidentally affect ordinary subagents.

### 3. Frontend Review Team Assembly Has Too Many Responsibilities

`reviewTeamService.ts` constructs config, validates custom agents, classifies risk, builds manifests, formats prompt blocks, builds work packets, estimates token budgets, and creates cache plans. The public API is useful, but the implementation is too large to reason about safely.

### 4. Report And UI Surfaces Are Blending Standard Review With Deep Review

`CodeReviewTool` and `CodeReviewReport` are shared by standard Code Review and Deep Review. That reuse is good, but Deep Review-only packet, cache, queue, and reliability logic should be isolated behind Deep Review-specific normalizers so standard Code Review remains easy to reason about.

### 5. Some Concepts Are Repeated Across Frontend And Backend

Strategy levels, execution policy fields, concurrency fields, retry limits, and token budget concepts exist in both TypeScript and Rust. Some duplication is expected because frontend builds launch manifests and Rust enforces runtime guardrails, but the boundaries should be explicit:

- TypeScript owns UX defaults, manifest construction, and prompt generation.
- Rust owns enforcement, queue safety, retry admission, and final trust boundaries.
- Shared JSON field names must be centralized in manifest parser/builders, not hand-read in many locations.

## Target Architecture

### Backend Module Layout

The backend now has a Deep Review subsystem directory under core:

```text
src/crates/core/src/agentic/deep_review/
  mod.rs
  constants.rs
  team_definition.rs
  manifest.rs
  execution_policy.rs
  concurrency_policy.rs
  queue.rs
  diagnostics.rs
  shared_context.rs
  incremental_cache.rs
  report.rs
  task_adapter.rs
  tool_context.rs
  tool_measurement.rs
```

Current responsibilities:

- `constants.rs`: agent type constants and role families.
- `team_definition.rs`: default review team definition and strategy profile data.
- `manifest.rs`: typed accessors for `deep_review_run_manifest`, scope profile parsing, evidence pack validation, and manifest gating.
- `execution_policy.rs`: timeouts, file split thresholds, retry limit config, risk helper.
- `concurrency_policy.rs`: configured cap and effective-cap calculations.
- `queue.rs`: queue state, queue controls, capacity error classification, local/provider queue decisions.
- `diagnostics.rs`: aggregate runtime diagnostics, final low-frequency logging data.
- `shared_context.rs`: duplicate `Read`/`GetFileDiff` measurement and future evidence-pack metadata helpers.
- `incremental_cache.rs`: per-session packet cache data model and serialization.
- `report.rs`: Deep Review-specific reliability signal and packet metadata helpers used by `CodeReviewTool`.
- `task_adapter.rs`: Deep Review-specific TaskTool adapter helpers.
- `tool_context.rs` / `tool_measurement.rs`: context detection and content-free tool measurement helpers.

The existing `src/crates/core/src/agentic/deep_review_policy.rs` is now a compatibility facade with global tracker accessors and re-exports. It should continue shrinking as call sites move to `agentic::deep_review::*`, but the current facade remains intentionally available for compatibility.

### Generic Subagent Runtime Boundary

Deep Review should not own generic subagent scheduling. The first no-behavior generic runtime primitive now exists:

```text
src/crates/core/src/agentic/subagent_runtime/
  mod.rs
  queue_timing.rs
```

`queue_timing.rs` only tracks queue wait, pause/continue time, elapsed milliseconds, and expiration checks. It does not know about Deep Review roles, provider reasons, queue events, retry policy, reports, or user-facing wording.

Future generic runtime-facing shapes may be introduced only after the current behavior proves stable:

```text
src/crates/core/src/agentic/subagent_runtime/
  capacity.rs
  queue_state.rs
  retry_admission.rs
```

Initial rule: do not move behavior here until it is proven generic.

Generic candidates:

- capacity acquisition/release guard
- queue state shape independent of Deep Review labels
- timeout separation between queue wait and running time
- bounded retry admission primitives

Deep Review-specific adapters remain in `agentic/deep_review/queue.rs` and `agentic/deep_review/retry.rs`.

Do not make provider-capacity auto queueing a global subagent behavior in this refactor. That is a product behavior change and needs a separate confirmation.

### Backend Tool Facades

Keep tool entrypoints stable:

- `TaskTool` remains the tool registered in the registry.
- `CodeReviewTool` remains the tool registered for report submission.

But move Deep Review branches behind helper modules:

```rust
// task_tool.rs
let deep_review_context = deep_review::manifest::Context::from_tool_context(context);
deep_review::task_adapter::prepare_launch(...);
deep_review::retry::validate_retry(...);
deep_review::queue::wait_for_reviewer_capacity(...);
```

```rust
// code_review_tool.rs
deep_review::report::fill_packet_metadata(...);
deep_review::report::fill_reliability_signals(...);
deep_review::incremental_cache::persist_completed_packets(...);
deep_review::diagnostics::log_final_snapshot(...);
```

This keeps the public tool behavior unchanged while making feature-specific code easier to test.

### Frontend Module Layout

The frontend review team service has been split into a directory with a facade:

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

Keep the current import path working:

```text
src/web-ui/src/shared/services/reviewTeamService.ts
```

The compatibility import path now exports from `./review-team`. Pure helper extraction is complete for path metadata, member projection, work packets, token budget, risk, scope profile, evidence pack, cache plan, pre-review summary, and prompt block formatting. Keep side-effectful config/backend loading and the public manifest assembly path in `index.ts` unless a later no-behavior-change round adds a narrower adapter with equivalent facade tests.

### Flow Chat Deep Review Layout

Split launch and UI helpers without changing visible behavior:

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

Keep current public exports from `DeepReviewService.ts`, `DeepReviewActionBar.tsx`, and `codeReviewReport.ts` during migration. The current low-risk action-bar cleanup has split diagnostics, action controls, and remediation selection; future no-behavior cleanup should only resume if later behavior work makes the action-bar file dense again.

Ownership notes now live in `src/web-ui/src/flow_chat/deep-review/README.md`. Keep that README aligned whenever launch, action-bar, or report responsibilities move.

## Proposed Execution Rounds

### Round 0: Baseline And Guardrails

Goal: create a safe refactor baseline.

Actions:

- Record line counts for the oversized files listed above.
- Run focused Deep Review tests already used on this branch.
- Add a short architecture note to the PR description that this is a no-behavior-change refactor plan.
- Confirm that non-Deep Review impact inventory is tracked in `docs/deep-review-nondeepreview-impact-inventory.md`.

Verification:

- `pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts src/flow_chat/components/btw/DeepReviewActionBar.test.tsx src/flow_chat/utils/codeReviewReport.test.ts`
- `cargo test -p bitfun-core deep_review -- --nocapture` when Cargo registry access is available.

Behavior change allowed: none.

### Round 1: Backend Deep Review Module Extraction

Goal: reduce `deep_review_policy.rs` without changing behavior.

Actions:

- Create `src/crates/core/src/agentic/deep_review/`.
- Move constants and default team definition to `constants.rs` and `team_definition.rs`.
- Move execution policy and strategy helpers to `execution_policy.rs`.
- Move concurrency, queue, diagnostics, shared context, retry, and cache into separate modules one at a time.
- Keep `deep_review_policy.rs` as a compatibility facade until imports are migrated.

Verification:

- Existing Rust Deep Review tests.
- `rg -n "deep_review_policy::" src/crates/core/src` to verify imports are intentionally retained or migrated.

Behavior change allowed: none.

Risks:

- Moving tests can hide coverage if not migrated with the module.
- Public helper visibility may be widened accidentally.

Mitigation:

- Move tests with modules.
- Prefer `pub(crate)` until cross-module callers require `pub`.

### Round 2: TaskTool Deep Review Adapter Extraction

Goal: keep generic TaskTool free from Deep Review implementation detail.

Actions:

- Add a `deep_review::task_adapter` module that owns:
  - detecting Deep Review context
  - resolving packet ids from manifests
  - attaching per-session cache to manifests
  - validating structured retry coverage
  - preparing retry prompt prefixes
  - calling queue/capacity helpers
- Leave `TaskTool` with a small orchestration call into the adapter.
- Keep ordinary subagent path unchanged.

Verification:

- Deep Review TaskTool tests.
- Add or preserve a regression test that a non-DeepReview Task with the same fields does not enter Deep Review queue/retry/cache paths.

Behavior change allowed: none.

Risks:

- This touches generic subagent execution.
- Mistakes can alter normal hidden subagent behavior.

Mitigation:

- Add explicit non-DeepReview tests before moving logic.
- Do not generalize provider queueing in this round.

### Round 3: CodeReviewTool Deep Review Report Adapter

Goal: separate standard Code Review report behavior from Deep Review report enrichments.

Actions:

- Add `deep_review::report` module for:
  - packet metadata fallback
  - reliability signal filling
  - token budget reliability notices
  - runtime diagnostics logging
  - per-session incremental cache write-through
- Keep `CodeReviewTool` schema and public behavior unchanged.
- Ensure standard Code Review does not receive Deep Review-only signals.

Verification:

- Existing `code_review_tool` Deep Review tests.
- Standard Code Review report tests or new tests proving no Deep Review-only metadata appears outside Deep Review context.

Behavior change allowed: none.

### Round 4: Shared Event And Tool Pipeline Containment

Goal: prevent Deep Review-specific details from spreading through shared runtime code.

Actions:

- Keep current `DeepReviewQueueStateChanged` event contract stable.
- Move event payload conversion helpers to Deep Review modules.
- In `tool_pipeline.rs`, replace inline Deep Review context propagation with a small hook/helper.
- Keep duplicate `Read`/`GetFileDiff` measurement gated by Deep Review context.

Verification:

- `cargo test -p bitfun-events deep_review_queue_state_event_serializes_stable_contract -- --nocapture`
- Tool pipeline tests covering non-DeepReview tools.

Behavior change allowed: none.

Deferred behavior change:

- Replacing `DeepReviewQueueStateChanged` with a generic `SubagentQueueStateChanged` event. This would affect frontend/API contracts and requires user confirmation before implementation.

### Round 5: Frontend Review Team Service Decomposition

Goal: shrink `reviewTeamService.ts` and make review team responsibilities discoverable.

Actions:

- Completed: `src/web-ui/src/shared/services/reviewTeamService.ts` is a compatibility facade over `./review-team`.
- Completed: type definitions, defaults, strategy profiles, path metadata, risk, work packets, token budget, scope profile, evidence pack, cache plan, pre-review summary, manifest-member projection, and prompt formatting are split into focused modules.
- Keep `review-team/index.ts` as the side-effectful service facade for config persistence, backend definition loading, default team assembly, and final manifest assembly.
- Add future review-team behavior to the narrow helper module first; only grow `index.ts` for API-facing orchestration or adapter calls.

Verification:

- `pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts`
- `pnpm run type-check:web`

Behavior change allowed: none.

Risks:

- Circular imports between `types`, `manifestBuilder`, and `promptBlock`.
- Tests can pass through the facade while internal modules become poorly bounded.

Mitigation:

- `types.ts` must not import implementation modules.
- `manifestBuilder.ts` may import pure helpers, but helpers must not import `manifestBuilder.ts`.

### Round 6: Frontend Flow Chat Deep Review Decomposition

Goal: separate launch, action bar, and report concerns.

Current status: stable frontend cleanup complete for the current scope. The compatibility facades are preserved; launch command/target/prompt/error helpers, report manifest/reliability/section/markdown helpers, and action-bar capacity/partial-results/recovery-plan/header/formatting/remediation/action-controls/diagnostics helpers are now separated. Future action-bar extraction should only be done when new behavior work creates real density again.

Actions:

- Completed: split `DeepReviewService.ts` into command parsing, target resolution, launch prompt formatting, launch errors, and the remaining child-session launch orchestrator.
- Completed: split `DeepReviewActionBar.tsx` capacity queue notice, review action header, elapsed-time formatting, remediation selection, action controls, and diagnostics text building.
- Completed: split `codeReviewReport.ts` into reliability notice building, manifest markdown sections, report section normalization, and markdown export.
- Remaining optional cleanup: none in the current low-risk action-bar scope; revisit only if future behavior work increases density.

Verification:

- `pnpm --dir src/web-ui run test:run -- src/flow_chat/services/DeepReviewService.test.ts src/flow_chat/components/btw/DeepReviewActionBar.test.tsx src/flow_chat/utils/codeReviewReport.test.ts`
- `pnpm run lint:web`
- `pnpm run type-check:web`

Behavior change allowed: none.

### Round 7: Documentation, Comments, And Ownership Cleanup

Goal: document module boundaries without adding noisy comments.

Current status: Flow Chat ownership cleanup is complete for the current frontend split. The subsystem README records module boundaries, facade guardrails, Deep Review gating, privacy constraints, and focused verification. Backend ownership cleanup has a focused first pass: `deep_review` module docs now explain product-policy ownership, cache/diagnostics/privacy boundaries, and the generic runtime split; `subagent_runtime` docs define the generic primitive boundary.

Actions:

- Completed for Flow Chat: add subsystem-level TypeScript ownership documentation without adding noisy per-file comments.
- Completed for Flow Chat: update Deep Review plan/status docs to reflect the real first-pass split and optional action-bar follow-ups.
- Completed for backend first pass: add module-level Rust docs for the Deep Review policy, budget, queue, manifest, cache, diagnostics, report, shared-context, tool-context, tool-measurement, and generic `subagent_runtime` boundary.
- Future backend cleanup: add module-level Rust docs only when additional modules are touched and their responsibilities are not obvious.
- Future no-behavior cleanup: remove duplicated constants or status wording only when a focused scan finds real duplication, not business terms such as capacity `temporary_overload`.

Verification:

- `rg -n "TODO|TBD|temporary|copy of|duplicate" src/crates/core/src/agentic/deep_review src/web-ui/src/shared/services/review-team`
- Full focused frontend and Rust Deep Review tests.

Behavior change allowed: none.

## Dependency Rules

### Backend

- `agentic/deep_review/*` may depend on shared core utilities and tool/report types.
- Shared `TaskTool`, `CodeReviewTool`, and `tool_pipeline` may call Deep Review adapters, but they should not own Deep Review policy data.
- `agentic/subagent_runtime/*` must not import Deep Review modules.
- `events` crate must remain data-only and should not import core Deep Review policy.
- No module in `deep_review` should depend on desktop, Tauri, or frontend-specific concepts.

### Frontend

- `review-team/types.ts` must be dependency-light and should not import API adapters.
- `review-team/index.ts` may import config and agent APIs because it remains the service facade for persisted config, backend default definition loading, and final manifest assembly.
- Pure helper modules such as `risk.ts`, `workPackets.ts`, `tokenBudget.ts`, `cachePlan.ts`, `preReviewSummary.ts`, `scopeProfile.ts`, `evidencePack.ts`, `manifestMembers.ts`, and `promptBlock.ts` must not import API adapters or call the facade.
- Helper dependencies must stay one-way: prompt formatting consumes an already-built manifest, work packets may consume member projection, and no helper should import `index.ts`.
- Flow Chat launch modules may import the review-team facade, not internal modules unless there is a clear reason.
- UI components must not call Tauri APIs directly.

## Non-Goals

- Do not introduce a new crate unless module extraction shows a stable boundary. A crate split is higher friction because many Deep Review helpers still use core session, tool, and error types.
- Do not make Deep Review queueing global subagent behavior.
- Do not change default concurrency, retry, or strategy behavior.
- Do not add project-level review cache.
- Do not replace the prompt-driven DeepReview orchestrator with a backend DAG scheduler.
- Do not replace existing event names in this refactor.

## Behavior Change Checkpoints

The following items would be behavior changes and must be confirmed before implementation:

1. Moving local/provider queue behavior from Deep Review to all subagents.
2. Replacing `DeepReviewQueueStateChanged` with a generic event.
3. Changing retry from model/user-issued structured retry to backend-owned automatic redispatch.
4. Making backend risk scoring authoritative over user/team strategy.
5. Persisting review cache outside session metadata.
6. Hard-clipping prompt bytes or hiding files from coverage metadata.
7. Changing default quick/normal/deep semantics beyond what is already documented in the current cost-aware plan.

## Quality Gates

Minimum verification for each refactor round:

- Rust-only round:
  - `cargo test -p bitfun-core deep_review -- --nocapture`
  - Add narrower tests for the moved module when possible.
- Shared tool/runtime round:
  - Rust Deep Review tests.
  - Non-DeepReview TaskTool or CodeReviewTool regression test.
- Frontend service round:
  - `pnpm --dir src/web-ui run test:run -- src/shared/services/reviewTeamService.test.ts`
  - `pnpm run type-check:web`
- Frontend UI/report round:
  - focused component/util tests
  - `pnpm run lint:web`
  - `pnpm run type-check:web`

Full release gate after all rounds:

- `cargo test -p bitfun-core deep_review -- --nocapture`
- `cargo check --workspace --exclude bitfun-cli`
- `pnpm run lint:web`
- `pnpm run type-check:web`
- `pnpm --dir src/web-ui run test:run`
- `git diff --check`

## Expected End State

- `deep_review_policy.rs` is no longer an oversized mixed-responsibility file.
- `TaskTool` and `CodeReviewTool` contain only thin Deep Review adapter calls.
- Generic subagent runtime concepts are available without forcing Deep Review behavior onto ordinary subagents.
- `reviewTeamService.ts` is a stable facade over smaller review-team modules.
- Flow Chat Deep Review launch, report, and action-bar logic are separated.
- Non-DeepReview impact is documented and covered by focused regression tests.
- No key Deep Review behavior changes unless separately confirmed.

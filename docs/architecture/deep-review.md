# DeepReview Architecture

## Scope

DeepReview is a child-session workflow that runs a configurable Code Review Team against a review target. The current implementation has three layers:

- Frontend launch and UI orchestration in `src/web-ui`.
- Platform adapter commands in `src/apps/desktop/src/api/agentic_api.rs`.
- Platform-agnostic runtime policy, task admission, queue state, retry metadata, and report enrichment in `src/crates/core/src/agentic`.

The backend does not choose the review target or build the launch manifest. The frontend builds the effective `ReviewTeamRunManifest`, persists it on the DeepReview child session, and sends it with the first user message.

## Runtime Roles

`src/crates/core/src/agentic/agents/deep_review_agent.rs` defines the writable `DeepReview` orchestrator. It can call `Task`, read/search/git tools, `submit_code_review`, `AskUserQuestion`, and write/edit/bash tools for user-approved remediation.

`src/crates/core/src/agentic/agents/review_specialist_agents.rs` defines read-only reviewer agents:

- `ReviewBusinessLogic`
- `ReviewPerformance`
- `ReviewSecurity`
- `ReviewArchitecture`
- `ReviewFrontend`
- `ReviewJudge`

The reviewer agents use instruction-only context and read/search/git/diff tools. `ReviewFrontend` is a conditional role. `ReviewJudge` validates reviewer evidence and consistency instead of performing a full independent review pass.

`ReviewFixer` exists as a separate remediation agent, but DeepReview runtime policy rejects it during review execution. Remediation is launched later only from the frontend action surface after user approval.

## Launch Flow

DeepReview can be launched from session-file review controls or a `/DeepReview` slash command.

Frontend launch code lives in `src/web-ui/src/flow_chat/deep-review/launch`:

- `commandParser.ts` identifies `/DeepReview` commands and optional file or git targets.
- `targetResolver.ts` resolves slash-command targets from git status, changed files, and diffs when a workspace is available.
- `launchPrompt.ts` formats the user-facing launch prompt.
- `DeepReviewService.ts` builds the review-team manifest, creates a child session, opens it in the auxiliary pane, sends the launch prompt, and inserts the parent-session summary marker.
- `src/web-ui/src/flow_chat/services/DeepReviewService.ts` is a compatibility re-export.

`launchDeepReviewSession` creates a child session with:

- `sessionKind: 'deep_review'`
- `agentType: 'DeepReview'`
- tools enabled
- safe mode enabled
- auto-compaction enabled
- context compression enabled
- `deepReviewRunManifest` stored on the child session metadata

If launch fails after the child session is created, the frontend closes the auxiliary pane, deletes the backend session when possible, discards local session state, and reports cleanup issues with the launch error.

## Review Team Configuration

The default review team contract is mirrored in Rust and TypeScript.

Rust source:

- `src/crates/core/src/agentic/deep_review/team_definition.rs`
- `src/crates/core/src/agentic/deep_review_policy.rs`
- `src/apps/desktop/src/api/agentic_api.rs`

Frontend source:

- `src/web-ui/src/shared/services/review-team/defaults.ts`
- `src/web-ui/src/shared/services/review-team/types.ts`
- `src/web-ui/src/shared/services/review-team/index.ts`

The desktop command `get_default_review_team_definition` returns the backend default definition. The frontend normalizes that response and falls back to its TypeScript default if the command is unavailable.

The persisted config path is `ai.review_teams.default`. The frontend config shape includes:

- extra subagent ids
- team strategy level
- per-member strategy overrides
- reviewer and judge timeouts
- reviewer file-split threshold
- max same-role instances
- max retries per role
- max parallel reviewers
- max queue wait seconds
- provider capacity queue enablement
- bounded auto-retry enablement and elapsed guard

Extra team members must be enabled subagents with read-only review tooling. Core team members, `DeepReview`, and `ReviewFixer` are disallowed as extra members.

## Manifest Shape

`buildEffectiveReviewTeamManifest` in `src/web-ui/src/shared/services/review-team/index.ts` builds the launch manifest. The manifest has `reviewMode: 'deep'` and may include:

- workspace path
- policy source
- target classification
- final strategy level
- scope profile
- frontend and backend strategy recommendations
- strategy decision
- execution policy
- concurrency policy
- change stats
- pre-review summary
- evidence pack
- shared-context cache plan
- incremental-review cache plan
- token-budget plan
- active core reviewers
- quality-gate reviewer
- enabled extra reviewers
- skipped reviewers
- work packets

The target classifier drives conditional reviewer selection. `ReviewFrontend` is included only when the target matches frontend-oriented files.

The evidence pack is metadata-only. It lists changed file paths, aggregate diff stats, domain/risk tags, packet ids, hunk hints, contract hints, and budget counts. It explicitly excludes source text, full diff text, model output, provider raw bodies, and full file contents.

## Strategies and Scope

The frontend owns strategy profile text and manifest planning in `src/web-ui/src/shared/services/review-team/strategy.ts` and `scopeProfile.ts`.

Supported strategy levels are `quick`, `normal`, and `deep`.

- `quick` uses high-risk-only scope, zero dependency hops, risk-matched optional reviewers, and no broad tool exploration.
- `normal` uses risk-expanded scope, one dependency hop, configured optional reviewers, and no broad tool exploration.
- `deep` uses full-depth scope, policy-limited dependency context, full optional reviewer policy, and broad tool exploration.

The backend parses the strategy from the manifest/config and uses it for runtime guardrails such as timeouts, policy classification, and retry limits. Backend strategy scoring is advisory and does not replace the frontend manifest decision.

## Work Packets

`src/web-ui/src/shared/services/review-team/workPackets.ts` creates pure launch-plan metadata. Work packets do not inspect file contents and do not make runtime retry or queue decisions.

Each work packet includes:

- packet id
- phase (`reviewer` or `judge`)
- launch batch
- subagent id and labels
- assigned scope
- allowed tools
- timeout seconds
- required output fields
- strategy level and directive
- model slot

If the included file count exceeds the reviewer file-split threshold and same-role instances are allowed, reviewer scopes are split into module-aware groups. Reviewer packets are then assigned launch batches using the concurrency policy. The judge packet, when present, runs in the batch after the final reviewer batch.

## Backend Policy and Admission

`DeepReviewExecutionPolicy` in `src/crates/core/src/agentic/deep_review/execution_policy.rs` parses runtime policy from config and classifies subagent launches.

Allowed DeepReview runtime launches are:

- core reviewer roles
- conditional reviewer roles when active in the manifest
- configured extra reviewer roles
- `ReviewJudge`

Rejected launches include:

- `ReviewFixer` during review execution
- nested `DeepReview`
- any subagent not configured for the review team
- subagents skipped or absent from the run manifest

`DeepReviewRunManifestGate` in `manifest.rs` reads active subagent ids from `workPackets`, `coreReviewers`, `enabledExtraReviewers`, and `qualityGateReviewer`. It also records skipped reviewer reasons so policy failures can explain why a reviewer is inactive.

## Task Execution and Queue State

The generic `Task` tool is adapted for DeepReview in:

- `src/crates/core/src/agentic/tools/implementations/task_tool.rs`
- `src/crates/core/src/agentic/deep_review/task_adapter.rs`
- `src/crates/core/src/agentic/deep_review/queue.rs`
- `src/crates/core/src/agentic/deep_review/budget.rs`

DeepReview task execution uses the manifest and tool context to:

- identify reviewer role and packet id
- attach incremental review cache data
- enforce policy and retry coverage
- cap active reviewers
- preserve launch-batch ordering
- wait for transient capacity when allowed
- emit queue state events
- record runtime diagnostics and capacity skips

Queueable capacity reasons are:

- `provider_rate_limit`
- `provider_concurrency_limit`
- `retry_after`
- `local_concurrency_cap`
- `launch_batch_blocked`
- `temporary_overload`

Queue states are:

- `queued_for_capacity`
- `paused_by_user`
- `running`
- `capacity_skipped`

Queue wait time is tracked separately from reviewer run time. Paused and queued time does not consume reviewer timeout.

The desktop command `control_deep_review_queue` validates `sessionId`, `dialogTurnId`, and `toolId`, then applies one of:

- `pause`
- `continue`
- `cancel`
- `skip_optional`

Pause, continue, and cancel are scoped to a specific turn and tool id. `skip_optional` is turn-scoped.

## Runtime Events

Queue state events are defined in `src/crates/events/src/agentic.rs` as `AgenticEvent::DeepReviewQueueStateChanged`.

The frontend listens through `AgentAPI.onDeepReviewQueueStateChanged` on `agentic://deep-review-queue-state-changed`. The TypeScript event shape mirrors the Rust event fields:

- tool id
- subagent type
- queue status
- optional reason
- queued reviewer count
- optional active reviewer count
- optional effective parallel instances
- optional optional-reviewer count
- optional queue/run elapsed time
- optional max queue wait
- session concurrency flag

`src/web-ui/src/flow_chat/utils/deepReviewQueueStateEvents.ts` applies queue events only to `deep_review` sessions.

## Report Submission

Review results are submitted through `submit_code_review` in `src/crates/core/src/agentic/tools/implementations/code_review_tool.rs`.

In DeepReview context, the tool requires the deep-review fields in addition to the standard summary/issues/positive-points shape:

- `review_mode`
- `review_scope`
- `reviewers`
- `remediation_plan`

DeepReview report enrichment lives in `src/crates/core/src/agentic/deep_review/report.rs`. It fills missing reviewer packet metadata when a unique packet can be inferred, adds runtime diagnostics, updates incremental cache data, and adds reliability signals for cache hits, cache misses, partial coverage, capacity skips, retry guidance, queue waits, reduced scope, and evidence-pack metadata.

Report enrichment is guarded by the tool context. Standard Code Review output should not receive DeepReview-only metadata unless the active tool context proves `agent_type == 'DeepReview'`.

## Frontend Report and Action UI

DeepReview report rendering lives under `src/web-ui/src/flow_chat/deep-review/report` and is consumed by `CodeReviewToolCard`.

The action surface is shared with standard Code Review but includes DeepReview-specific phases and capacity state:

- `src/web-ui/src/flow_chat/store/deepReviewActionBarStore.ts`
- `src/web-ui/src/flow_chat/components/btw/BtwSessionPanel.tsx`
- `src/web-ui/src/flow_chat/deep-review/action-bar`

`BtwSessionPanel` detects `sessionKind === 'deep_review'`, reads the latest code-review result, derives interrupted DeepReview state, restores persisted action-bar state, and renders `ReviewActionBar`.

The action bar can show:

- capacity queue notice and inline controls
- partial results
- recovery plan preview
- remediation item selection
- needs-decision gate
- fix, fix-and-review, resume, and retry controls

The action bar dispatches queue controls through `agentAPI.controlDeepReviewQueue` when backend queue-control identifiers are available. Otherwise it falls back to local/session-stop-only behavior exposed by the store.

## Persistence

The DeepReview child session stores `deepReviewRunManifest` in frontend session state, session metadata, and history metadata. The backend also reads `deep_review_run_manifest` from tool context/session metadata when a DeepReview tool call needs manifest data.

The review action bar persists UI state separately through `ReviewActionBarPersistenceService` so historical review sessions can restore visible remediation progress without rerunning the review.

## Boundary Rules

- Frontend components do not call Tauri directly; they use infrastructure APIs such as `agentAPI`.
- Shared core stays platform-agnostic and uses event/config/tool abstractions instead of Tauri handles.
- The frontend owns target resolution, team manifest construction, strategy profile wording, prompt-block construction, consent, and action UI.
- The backend owns policy validation, runtime admission, queue/retry state, event emission, and report enrichment.
- Reviewer subagents stay read-only. Remediation runs after user approval through the action surface, not during the reviewer pass.
- Work packets and evidence packs are planning metadata; they must not embed file contents or full diffs.

## Change Checklist

When changing DeepReview behavior, update all affected contracts together:

- Backend constants, team definition, execution policy, manifest gate, task adapter, queue events, and report enrichment.
- Frontend review-team defaults/types, manifest builder, prompt block, launch service, action-bar store, event mapping, report rendering, and locales.
- Desktop Tauri command DTOs when queue controls or default team definition contracts change.
- Tests near the touched module, especially policy tests, review-team manifest tests, queue event tests, launch tests, action-bar tests, and locale completeness tests.

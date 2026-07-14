# DeepReview / Strict Review Architecture

## Scope

DeepReview is the compatibility runtime for `Review: Strict`, the highest-strength mode of the unified Review experience. It remains a read-only child session, but the child is now the primary strict reviewer rather than a dispatcher for a fixed reviewer committee. It should not be presented as a second ordinary product entry next to Review.

Product-facing guardrails are summarized here:

- `Review` is the primary user-facing entry.
- `/review` is the intended long-term command entry; `/DeepReview` is only a transitional typed compatibility command for historical strict-review launches.
- `ReviewTeam` is an internal strict-review reviewer-set configuration, not a separate product concept users must learn.
- PR Review consumes Review results and future readiness projections; it must not own another reviewer executor.

The current implementation has three layers:

- Frontend launch and UI orchestration in `src/web-ui`.
- Platform adapter commands in `src/apps/desktop/src/api/agentic_api.rs`.
- Platform-agnostic runtime policy, task admission, queue state, retry metadata, and report enrichment in `src/crates/assembly/core/src/agentic`.

The launch adapter is currently desktop-only. Browser/server surfaces hide every Review launch action, including fix follow-up retry, and reject typed Review commands with a clear unsupported-state message until the server owns the same session, Git-target, and policy command contracts; existing review attempts remain viewable. Adding only one RPC method to the current ping-only server would not make the workflow functional. The Review settings route remains visible for navigation compatibility on those surfaces, but renders a read-only desktop-only state and never loads or saves desktop capacity settings.

Review strength follows explicit intent instead of a target-size or risk-score threshold table. Ordinary `Review` always launches one read-only `CodeReview` child. Only an explicit strict intent—initial `/review strict`, the historical `/DeepReview` alias, or a strict fix follow-up—builds a `ReviewTeamRunManifest` and enters the DeepReview runtime. L0 completion checks and Verify evidence remain outside this production contract until the separate Verify exploration defines a trustworthy evidence source.

The backend does not resolve the review target or build the launch manifest. The frontend resolves and validates bounded target evidence before either launch path. Strict Review builds one deep L3 manifest and reuses it unchanged for consent and execution. The manifest, session kind, agent type, storage keys, and queue event names stay compatible with historical DeepReview sessions.

## Runtime Roles

`CodeReview` and `DeepReview` are read-only adversarial review identities. `CodeReview` handles ordinary Review as one isolated child and cannot silently expand into multiple reviewers. `DeepReview` is reserved for an explicit strict request, reviews the prepared target directly, may ask one manifest-approved specialist for a focused second perspective, and submits the report; it has no edit, command, Git-mutation, or remediation tools.

`src/crates/assembly/core/src/agentic/agents/definitions/review/review_specialists.rs` defines read-only reviewer agents:

- `ReviewBusinessLogic`
- `ReviewPerformance`
- `ReviewSecurity`
- `ReviewArchitecture`
- `ReviewFrontend`
- `ReviewJudge`

These agents form an optional specialist pool, not mandatory coverage lanes. A new strict run may launch at most one specialist for a concrete uncertainty. The existing generic Git exposure remains for legacy compatibility, but it is not authorized as prepared changed-code evidence. Prepared `GetFileDiff` is the source of truth for changed code; when the local binding is `matching_clean`, existing Read/Grep/Glob/LS tools may supplement it with repository context. `ReviewJudge` is a conditional quality check used only for a high-severity finding, conflicting evidence, or a materially low-confidence conclusion; it does not perform a full independent review pass.

`ReviewFixer` is the separate writable remediation identity. DeepReview runtime policy rejects it during review execution. The frontend action surface invokes it only after user approval, and a new read-only Review run checks the fix when requested.

## Launch Flow

Review can be launched from session-file controls or `/review`. Session-file controls and ordinary `/review` launch one standard read-only reviewer regardless of target size or heuristic risk tags. `/review strict` explicitly requests the deep L3 path. Historical `/DeepReview` and `/deepreview` inputs remain compatibility aliases, and a fix follow-up from an existing strict review preserves that explicit strict intent.

Frontend launch code lives in `src/web-ui/src/flow_chat/deep-review/launch`:

- `commandParser.ts` identifies canonical `/review strict` commands, transitional `/DeepReview` compatibility aliases, and optional file or git targets.
- `targetResolver.ts` resolves slash-command target file lists, immutable base/head revisions, change statistics, and target evidence from git status, explicit ranges, and diffs when a workspace is available. Explicit file and directory targets remain exact instead of widening to the whole worktree. File-scoped evidence reads untracked content through the registered, remote-aware workspace API.
- `launchPrompt.ts` formats the user-facing launch prompt.
- `DeepReviewService.ts` builds the review-team manifest, creates a child session, sends the launch prompt, and inserts the parent-session summary marker. The unified `ReviewService.ts` opens the child in the existing auxiliary pane.
- `src/web-ui/src/flow_chat/services/DeepReviewService.ts` is a compatibility re-export.
- `src/web-ui/src/flow_chat/services/ReviewService.ts` owns the unified prepared plan and launches either one read-only CodeReview child or the existing DeepReview child runtime.
- Fix follow-up uses the same service to re-evaluate the union of the original review files and files directly changed by `ReviewFixer`. If command, Git, or stdin tools can produce changes that cannot be attributed safely, the UI explicitly falls back to the current workspace diff instead of claiming a narrower scope. It remeasures the selected diff, preserves the original standard-or-strict intent, and opens one fresh isolated reviewer child in the existing auxiliary pane. The fixer baseline and exact selected remediation ids are persisted before remediation starts, so restart restores only unfinished items from the original selection. The follow-up reservation stores the same request id later written to the existing child relationship metadata and used to derive the backend session id. A launch acknowledgement failure preserves the stable local turn and created child, returns `uncertain`, and does not automatically or after restart resubmit the launch message. Backend creation returns an existing session only when the immutable identity (`agent_type`, relationship kind, parent session, and parent request) matches; mutable parent turn location does not break an explicit user retry. This early-return path is restricted to Review/DeepReview child relationships with a parent request, so ordinary explicit-id session restoration keeps its existing coordinator rebuild behavior. The action bar distinguishes retry, in-progress, completed, failed, cancelled, and view states instead of leaving a permanently disabled button. A metadata-only historical child is opened and hydrated before terminal state is inferred; lack of loaded turns is not treated as permission to launch a duplicate. Scope, changed-file records, and the final child id stay in session metadata so restart does not widen scope or duplicate a known run. Older sessions without recoverable scope explicitly notify the user before falling back to the current workspace diff.

`launchDeepReviewSession` creates a child session with:

- `sessionKind: 'deep_review'`
- `agentType: 'DeepReview'`
- tools enabled
- safe mode enabled
- auto-compaction enabled
- context compression enabled
- `deepReviewRunManifest` stored on the child session metadata

If the first launch message has an uncertain outcome after the child session is created, the frontend preserves the local turn with a request-derived stable turn id, opens the child session, records an interruption marker, and returns an explicit `uncertain` launch status. It does not automatically resubmit the message or delete a possibly running backend session.

## Target Evidence

Review target evidence is session-scoped and covers current workspace changes, an explicit local Git range, or a provider pull request. It carries:

- source kind and opaque target fingerprint
- base and head revisions
- changed file path, previous path, and add/modify/delete/rename status
- completeness and limitation facts for truncation, binary files, or unavailable content
- a deterministic workspace binding that says whether the local repository head matches the target head and whether any staged, unstaged, untracked, or conflicted worktree state could contaminate repository context
- final report evidence status (`complete`, `limited`, `stale`, or `failed`) separately from the model's risk and recommendation

An explicit, complete Git range with a matching clean workspace or a provider PR with immutable base/head and complete per-file diff availability may report `complete`. Workspace evidence remains `limited` because it is mutable, even when its prepared diff coverage is complete. Limited or stale evidence does not rewrite the model's risk or recommendation; the report and UI display reliability separately. Invalid evidence fails closed, while historical manifests with no target evidence keep legacy behavior.

Prepared Review target evidence uses bounded `GetFileDiff` pages as changed-code evidence. Local ranges read exact Git revisions; PR targets read provider diffs on demand and revalidate base/head before each file. The parent Review has a 240,000-character aggregate allowance and admits at most 128 provider diff acquisitions before provider I/O; one acquisition normally performs one file-page request and one detail request. Repeating the same page for the same reviewer returns a compact already-served result instead of the diff again. Exhaustion and stale target bindings return structured limited evidence. Existing generic Git exposure remains for legacy compatibility but does not authorize ref guessing or scope widening; Read/Grep/Glob/LS are supplemental only for a matching clean Git-range binding, never for a provider-only PR target.

Deleted, renamed, binary, oversized, conflicted, or unavailable files remain visible as coverage facts. The PR panel is the only built-in PR Review entry and associates progress/results by provider repository, PR id, and immutable revisions. Cached overview data is display-only until the selected PR is revalidated; revision or runtime-evidence changes make prior results stale, and failed or unavailable results remain distinct from limited coverage. The implementation does not add automatic checkout, reviewer command execution, speculative cache plans, automatic Review, inline comments, approval, merge, or automatic publishing.

## Strict Review Delegation Policy

The default strict-review contract is mirrored in Rust and TypeScript. New strict launches use the following fixed boundary:

- the `DeepReview` child performs the primary full review itself;
- applicable core and explicitly configured extra reviewers form an allowed specialist pool;
- at most one specialist may be launched for a concrete unresolved question;
- `ReviewJudge` is available only as a conditional quality check;
- automatic file splitting, same-role fan-out, and reviewer retry are disabled;
- the run uses one primary review-agent execution, with at most one specialist execution and one quality-inspector execution.

The runtime enforces the one-specialist budget even if a weak model ignores the prompt. This is a resource ceiling, not a keyword or risk-score workflow rule. The model decides whether delegation is useful from the actual evidence and task, while the manifest limits which read-only agents it may call.

Historical configuration fields for reviewer timeouts, file-split thresholds, same-role instances, retries, concurrency, and queue behavior remain readable so stored sessions can recover honestly. New strict manifests override split, same-role, retry, and specialist-call values to the bounded policy above. Extra reviewers must still be enabled subagents with read-only review tooling. `DeepReview` and `ReviewFixer` remain disallowed.

## Manifest Shape

`buildEffectiveReviewTeamManifest` in `src/web-ui/src/shared/services/review-team/index.ts` builds the launch manifest. The manifest keeps `reviewMode: 'deep'`, resolved target evidence, strategy/scope metadata, execution policy, specialist pool, optional quality-inspector identity, skipped members, and token/call budget facts.

For new strict launches:

- `coreReviewers` and `enabledExtraReviewers` describe agents the primary reviewer may choose from; they are not scheduled calls;
- `qualityGateReviewer` identifies the available conditional inspector and does not require it to run;
- `workPackets` is empty;
- `executionPolicy.maxReviewerCalls` is `1`;
- file splitting and retries are disabled;
- the launch preview reports one planned primary review-agent execution and a maximum of three review-agent executions; it does not claim a bound on underlying model requests.

The evidence pack remains metadata-only. It lists changed file paths, aggregate diff stats, domain/risk tags, hunk hints, contract hints, budget counts, and workspace/Git-range target facts. It excludes embedded source text, full diff text, model output, provider raw bodies, speculative cache plans, and full file contents.

## Strategies and Scope

Ordinary Review remains one `CodeReview` child. A new explicit strict request always selects the deep profile, but “deep” now means deeper evidence inspection by the primary reviewer, not maximum fan-out. Security, performance, architecture, frontend, and test concerns are investigation dimensions for that model.

`quick` and `normal` strategy values, legacy work packets, and older L2 manifests remain readable for stored-session recovery. They do not create new production Review launches. New L3 validation requires the deep strategy but no longer requires every core reviewer or a Judge call. If a quality-gate member is present, it must be `ReviewJudge`.

Launch consent shows the exact target, one planned primary review-agent execution, the maximum bounded review-agent execution count, runtime tendency, and read-only boundary. It does not estimate underlying model requests or tokens.

## Historical Work Packet Compatibility

New strict reviews do not generate work packets or module-aware reviewer shards. Stored manifests may still contain reviewer/judge packets, launch batches, packet ids, assigned scopes, and retry metadata. Runtime parsing, report enrichment, recovery UI, and target-evidence validation continue to read those fields so historical sessions are not rewritten as complete or successful.

Compatibility code must not turn historical packet support back into a new-launch requirement. Packet-specific queue and retry behavior applies only when an existing manifest actually contains those packets.

## Backend Policy and Admission

`DeepReviewExecutionPolicy` in `src/crates/execution/agent-runtime/src/deep_review/execution_policy.rs` parses runtime policy and the new per-turn specialist-call ceiling. `DeepReviewRunManifestGate` admits only specialist-pool members and the optional `ReviewJudge`, rejects `ReviewFixer`, nested `DeepReview`, skipped members, and unconfigured agents, and preserves legacy manifest parsing and membership validation.

`DeepReviewBudgetTracker` separately permits at most one initial specialist and one Judge call for a new strict turn. This keeps the safety boundary deterministic without hard-coding which domain deserves delegation.

## Task Execution and Queue State

The generic `Task` tool is adapted for DeepReview in:

- `src/crates/assembly/core/src/agentic/tools/implementations/task_tool.rs`
- `src/crates/assembly/core/src/agentic/deep_review/task_adapter.rs`
- `src/crates/assembly/core/src/agentic/deep_review/queue.rs`
- `src/crates/assembly/core/src/agentic/deep_review/budget.rs`

DeepReview task execution uses the manifest and tool context to:

- identify an optional specialist or quality-inspector role and any historical packet id
- read historical incremental-cache metadata when present, without creating cache plans for new runs
- enforce the new specialist-call ceiling and historical retry coverage
- cap active optional reviewers
- preserve launch-batch ordering only for historical packet manifests
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

Queue state events are defined in `src/crates/contracts/events/src/agentic.rs` as `AgenticEvent::DeepReviewQueueStateChanged`.

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

Review results are submitted through `submit_code_review` in `src/crates/assembly/core/src/agentic/tools/implementations/code_review_tool.rs`.

In DeepReview context, the tool requires the deep-review fields in addition to the standard summary/issues/positive-points shape:

- `review_mode`
- `review_scope`
- `reviewers`
- `remediation_plan`

DeepReview report enrichment lives in `src/crates/assembly/core/src/agentic/deep_review/report.rs`. It fills missing reviewer packet metadata when a unique packet can be inferred, adds runtime diagnostics, preserves read compatibility for historical incremental-cache data, and adds reliability signals for partial coverage, capacity skips, retry guidance, queue waits, reduced scope, and evidence status. Missing or invalid report summaries fail closed instead of defaulting to approval.

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
- fix, independently review fixes, resume, and retry controls

The action bar dispatches queue controls through `agentAPI.controlDeepReviewQueue` when backend queue-control identifiers are available. Otherwise it falls back to local/session-stop-only behavior exposed by the store.

## Persistence

The DeepReview child session stores `deepReviewRunManifest` in frontend session state, session metadata, and history metadata. The backend also reads `deep_review_run_manifest` from tool context/session metadata when a DeepReview tool call needs manifest data.

The review action bar persists UI state separately through `ReviewActionBarPersistenceService` so historical review sessions can restore visible remediation progress without rerunning the review.

## Boundary Rules

- Frontend components do not call Tauri directly; they use infrastructure APIs such as `agentAPI`.
- Shared core stays platform-agnostic and uses event/config/tool abstractions instead of Tauri handles.
- The frontend owns the explicit standard-or-strict launch boundary, target resolution, strict team manifest construction, strategy profile wording, prompt-block construction, consent, and action UI. No product-domain risk policy or desktop decision RPC upgrades ordinary Review.
- Project integration adapters own raw workspace/Git target acquisition. The artifact/evidence layer owns the fixed session target manifest and its completeness. Mutable workspace targets may have complete prepared diff coverage, but their final evidence status remains `limited`. Reviewers may not mutate or silently widen that target.
- The backend owns policy validation, runtime admission, queue/retry state, event emission, and report enrichment.
- Reviewer subagents and review orchestrators stay read-only. Remediation runs under `ReviewFixer` after user approval, not during the reviewer pass.
- Historical work packets and current evidence packs are metadata only; they must not embed file contents or full diffs.
- Existing reviewer Git exposure remains unchanged for legacy compatibility, but prepared target evidence does not authorize it as changed-code evidence and no dedicated multi-operation Git tool is added. Prepared `GetFileDiff` must be bounded and disable external diff/text conversion; live repository reads are supplemental and require a deterministic clean local binding.

## Change Checklist

When changing DeepReview behavior, update all affected contracts together:

- Backend constants, team definition, execution policy, manifest gate, task adapter, queue events, and report enrichment.
- Frontend strict-review defaults/types, manifest builder, prompt block, launch service, action-bar store, event mapping, report rendering, and locales.
- Desktop Tauri command DTOs when capacity controls or default review definition contracts change.
- Tests near the touched module, especially policy tests, strict-review manifest tests, queue event tests, launch tests, action-bar tests, and locale completeness tests.
- For target-evidence changes, add contract coverage for workspace changes, exact file/directory scopes, explicit ranges, provider PR identity/base/head, provider diff availability, head invalidation, clean checkout, deleted/renamed/binary/oversized files, dirty-workspace isolation, exact-diff bounds, fail-closed reports, uncertain launch preservation, and unchanged ordinary Agent behavior.

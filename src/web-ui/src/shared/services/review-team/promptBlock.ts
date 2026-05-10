import { DEFAULT_REVIEW_TEAM_MODEL } from './defaults';
import {
  REVIEW_STRATEGY_COMMON_RULES,
  REVIEW_STRATEGY_LEVELS,
  REVIEW_STRATEGY_PROFILES,
} from './strategy';
import { toManifestMember } from './manifestMembers';
import type {
  DeepReviewEvidencePack,
  DeepReviewScopeProfile,
  ReviewRoleDirectiveKey,
  ReviewStrategyLevel,
  ReviewStrategyProfile,
  ReviewTeam,
  ReviewTeamIncrementalReviewCachePlan,
  ReviewTeamManifestMember,
  ReviewTeamPreReviewSummary,
  ReviewTeamRunManifest,
  ReviewTeamSharedContextCachePlan,
  ReviewTeamTokenBudgetDecision,
  ReviewTeamWorkPacket,
} from './types';

// Prompt formatting consumes an already-built manifest. Keep launch policy and
// side effects in the manifest/service layers so this stays deterministic.
function formatResponsibilities(items: string[]): string {
  return items.map((item) => `    - ${item}`).join('\n');
}

function formatStrategyImpact(
  strategyLevel: ReviewStrategyLevel,
  strategyProfiles: Record<ReviewStrategyLevel, ReviewStrategyProfile> = REVIEW_STRATEGY_PROFILES,
): string {
  const definition = strategyProfiles[strategyLevel];
  return `Token/time impact: approximately ${definition.tokenImpact} token usage and ${definition.runtimeImpact} runtime.`;
}

function formatManifestList(
  members: ReviewTeamManifestMember[],
  emptyValue: string,
): string {
  if (members.length === 0) {
    return emptyValue;
  }

  return members
    .map((member) =>
      member.reason
        ? `${member.subagentId}: ${member.reason}`
        : member.subagentId,
    )
    .join(', ');
}

function workPacketToPromptPayload(packet: ReviewTeamWorkPacket) {
  return {
    packet_id: packet.packetId,
    phase: packet.phase,
    launch_batch: packet.launchBatch,
    subagent_type: packet.subagentId,
    display_name: packet.displayName,
    role: packet.roleName,
    assigned_scope: {
      kind: packet.assignedScope.kind,
      target_source: packet.assignedScope.targetSource,
      target_resolution: packet.assignedScope.targetResolution,
      target_tags: packet.assignedScope.targetTags,
      file_count: packet.assignedScope.fileCount,
      files: packet.assignedScope.files,
      excluded_file_count: packet.assignedScope.excludedFileCount,
      ...(packet.assignedScope.groupIndex !== undefined
        ? { group_index: packet.assignedScope.groupIndex }
        : {}),
      ...(packet.assignedScope.groupCount !== undefined
        ? { group_count: packet.assignedScope.groupCount }
        : {}),
    },
    allowed_tools: packet.allowedTools,
    timeout_seconds: packet.timeoutSeconds,
    required_output_fields: packet.requiredOutputFields,
    strategy: packet.strategyLevel,
    model_id: packet.model,
    prompt_directive: packet.strategyDirective,
  };
}

function formatWorkPacketBlock(workPackets: ReviewTeamWorkPacket[] = []): string {
  if (workPackets.length === 0) {
    return '- none';
  }

  return [
    '```json',
    JSON.stringify(workPackets.map(workPacketToPromptPayload), null, 2),
    '```',
  ].join('\n');
}

function formatPreReviewSummaryBlock(summary: ReviewTeamPreReviewSummary): string {
  return [
    'Pre-generated diff summary:',
    '```json',
    JSON.stringify(summary, null, 2),
    '```',
  ].join('\n');
}

function evidencePackToPromptPayload(pack: DeepReviewEvidencePack) {
  return {
    version: pack.version,
    source: pack.source,
    changed_files: pack.changedFiles,
    diff_stat: {
      file_count: pack.diffStat.fileCount,
      ...(pack.diffStat.totalChangedLines !== undefined
        ? { total_changed_lines: pack.diffStat.totalChangedLines }
        : {}),
      line_count_source: pack.diffStat.lineCountSource,
    },
    domain_tags: pack.domainTags,
    risk_focus_tags: pack.riskFocusTags,
    packet_ids: pack.packetIds,
    hunk_hints: pack.hunkHints.map((hint) => ({
      file_path: hint.filePath,
      changed_line_count: hint.changedLineCount,
      line_count_source: hint.lineCountSource,
    })),
    contract_hints: pack.contractHints.map((hint) => ({
      kind: hint.kind,
      file_path: hint.filePath,
      source: hint.source,
    })),
    budget: {
      max_changed_files: pack.budget.maxChangedFiles,
      max_hunk_hints: pack.budget.maxHunkHints,
      max_contract_hints: pack.budget.maxContractHints,
      omitted_changed_file_count: pack.budget.omittedChangedFileCount,
      omitted_hunk_hint_count: pack.budget.omittedHunkHintCount,
      omitted_contract_hint_count: pack.budget.omittedContractHintCount,
    },
    privacy: pack.privacy,
  };
}

function formatEvidencePackBlock(pack?: DeepReviewEvidencePack): string {
  if (!pack) {
    return [
      'Evidence pack:',
      '- none',
    ].join('\n');
  }

  return [
    'Evidence pack:',
    '```json',
    JSON.stringify(evidencePackToPromptPayload(pack), null, 2),
    '```',
    '- Evidence pack hunk_hints and contract_hints are orientation only; verify each hinted claim with GetFileDiff, Read, Grep, or Git before reporting it.',
    '- The evidence pack privacy boundary is metadata_only. Do not treat it as source text, a full diff, model output, or provider raw data.',
  ].join('\n');
}

function sharedContextCacheToPromptPayload(plan: ReviewTeamSharedContextCachePlan) {
  return {
    source: plan.source,
    strategy: plan.strategy,
    omitted_entry_count: plan.omittedEntryCount,
    entries: plan.entries.map((entry) => ({
      cache_key: entry.cacheKey,
      path: entry.path,
      workspace_area: entry.workspaceArea,
      recommended_tools: entry.recommendedTools,
      consumer_packet_ids: entry.consumerPacketIds,
    })),
  };
}

function formatSharedContextCacheBlock(plan: ReviewTeamSharedContextCachePlan): string {
  return [
    'Shared context cache plan:',
    '```json',
    JSON.stringify(sharedContextCacheToPromptPayload(plan), null, 2),
    '```',
  ].join('\n');
}

function formatScopeProfileBlock(profile?: DeepReviewScopeProfile): string {
  if (!profile) {
    return [
      'Scope profile:',
      '- none',
    ].join('\n');
  }

  return [
    'Scope profile:',
    `- review_depth: ${profile.reviewDepth}`,
    `- risk_focus_tags: ${profile.riskFocusTags.join(', ') || 'none'}`,
    `- max_dependency_hops: ${profile.maxDependencyHops}`,
    `- optional_reviewer_policy: ${profile.optionalReviewerPolicy}`,
    `- allow_broad_tool_exploration: ${profile.allowBroadToolExploration ? 'yes' : 'no'}`,
    `- coverage_expectation: ${profile.coverageExpectation}`,
    '- Reduced-depth profiles are not full-depth coverage. Keep changed files visible in coverage notes and do not describe quick or normal runs as full-depth reviews.',
    '- Reviewers and the judge must carry review_depth and coverage_expectation into their summaries. If review_depth is high_risk_only or risk_expanded, populate reliability_signals with reduced_scope in the final submit_code_review payload.',
  ].join('\n');
}

function incrementalReviewCacheToPromptPayload(plan: ReviewTeamIncrementalReviewCachePlan) {
  return {
    source: plan.source,
    strategy: plan.strategy,
    cache_key: plan.cacheKey,
    fingerprint: plan.fingerprint,
    file_paths: plan.filePaths,
    workspace_areas: plan.workspaceAreas,
    target_tags: plan.targetTags,
    reviewer_packet_ids: plan.reviewerPacketIds,
    ...(plan.lineCount !== undefined ? { line_count: plan.lineCount } : {}),
    line_count_source: plan.lineCountSource,
    invalidates_on: plan.invalidatesOn,
  };
}

function formatIncrementalReviewCacheBlock(plan: ReviewTeamIncrementalReviewCachePlan): string {
  return [
    'Incremental review cache plan:',
    '```json',
    JSON.stringify(incrementalReviewCacheToPromptPayload(plan), null, 2),
    '```',
  ].join('\n');
}

function formatTokenBudgetDecisionKinds(
  decisions: ReviewTeamTokenBudgetDecision[] = [],
): string {
  return decisions.length > 0
    ? decisions.map((decision) => decision.kind).join(', ')
    : 'none';
}

export function buildReviewTeamPromptBlockContent(
  team: ReviewTeam,
  manifest: ReviewTeamRunManifest,
): string {
  const activeSubagentIds = new Set([
    ...manifest.coreReviewers.map((member) => member.subagentId),
    ...manifest.enabledExtraReviewers.map((member) => member.subagentId),
    ...(manifest.qualityGateReviewer
      ? [manifest.qualityGateReviewer.subagentId]
      : []),
  ]);
  const activeManifestMembers = [
    ...manifest.coreReviewers,
    ...(manifest.qualityGateReviewer ? [manifest.qualityGateReviewer] : []),
    ...manifest.enabledExtraReviewers,
  ];
  const manifestMemberBySubagentId = new Map(
    activeManifestMembers.map((member) => [member.subagentId, member]),
  );
  const members = team.members
    .filter((member) => member.available && activeSubagentIds.has(member.subagentId))
    .map((member) => {
      const manifestMember =
        manifestMemberBySubagentId.get(member.subagentId) ?? toManifestMember(member);
      return [
        `- ${manifestMember.displayName}`,
        `  - subagent_type: ${manifestMember.subagentId}`,
        `  - preferred_task_label: ${manifestMember.displayName}`,
        `  - role: ${manifestMember.roleName}`,
        `  - locked_core_role: ${manifestMember.locked ? 'yes' : 'no'}`,
        `  - strategy: ${manifestMember.strategyLevel}`,
        `  - strategy_source: ${manifestMember.strategySource}`,
        `  - default_model_slot: ${manifestMember.defaultModelSlot}`,
        `  - model: ${manifestMember.model || DEFAULT_REVIEW_TEAM_MODEL}`,
        `  - model_id: ${manifestMember.model || DEFAULT_REVIEW_TEAM_MODEL}`,
        `  - configured_model: ${manifestMember.configuredModel || manifestMember.model || DEFAULT_REVIEW_TEAM_MODEL}`,
        ...(manifestMember.modelFallbackReason
          ? [`  - model_fallback: ${manifestMember.modelFallbackReason}`]
          : []),
        `  - prompt_directive: ${manifestMember.strategyDirective}`,
        '  - responsibilities:',
        formatResponsibilities(member.responsibilities),
      ].join('\n');
    })
    .join('\n');
  const executionPolicy = [
    `- reviewer_timeout_seconds: ${manifest.executionPolicy.reviewerTimeoutSeconds}`,
    `- judge_timeout_seconds: ${manifest.executionPolicy.judgeTimeoutSeconds}`,
    `- reviewer_file_split_threshold: ${manifest.executionPolicy.reviewerFileSplitThreshold}`,
    `- max_same_role_instances: ${manifest.executionPolicy.maxSameRoleInstances}`,
    `- max_retries_per_role: ${manifest.executionPolicy.maxRetriesPerRole}`,
  ].join('\n');
  const concurrencyPolicy = [
    `- max_parallel_instances: ${manifest.concurrencyPolicy.maxParallelInstances}`,
    `- stagger_seconds: ${manifest.concurrencyPolicy.staggerSeconds}`,
    `- max_queue_wait_seconds: ${manifest.concurrencyPolicy.maxQueueWaitSeconds}`,
    `- batch_extras_separately: ${manifest.concurrencyPolicy.batchExtrasSeparately ? 'yes' : 'no'}`,
    `- allow_provider_capacity_queue: ${manifest.concurrencyPolicy.allowProviderCapacityQueue ? 'yes' : 'no'}`,
    `- allow_bounded_auto_retry: ${manifest.concurrencyPolicy.allowBoundedAutoRetry ? 'yes' : 'no'}`,
    `- auto_retry_elapsed_guard_seconds: ${manifest.concurrencyPolicy.autoRetryElapsedGuardSeconds}`,
  ].join('\n');
  const targetLineCount =
    manifest.changeStats?.totalLinesChanged !== undefined
      ? `${manifest.changeStats.totalLinesChanged}`
      : 'unknown';
  const manifestBlock = [
    'Run manifest:',
    `- review_mode: ${manifest.reviewMode}`,
    `- team_strategy: ${manifest.strategyLevel}`,
    `- strategy_authority: ${manifest.strategyDecision.authority}`,
    `- final_strategy: ${manifest.strategyDecision.finalStrategy}`,
    `- frontend_recommended_strategy: ${manifest.strategyDecision.frontendRecommendation.strategyLevel}`,
    `- backend_recommended_strategy: ${manifest.strategyDecision.backendRecommendation.strategyLevel}`,
    `- strategy_user_override: ${manifest.strategyDecision.userOverride ?? 'none'}`,
    `- strategy_mismatch: ${manifest.strategyDecision.mismatch ? 'yes' : 'no'}`,
    `- strategy_mismatch_severity: ${manifest.strategyDecision.mismatchSeverity}`,
    `- max_cyclomatic_complexity_delta: ${manifest.strategyDecision.backendRecommendation.factors.maxCyclomaticComplexityDelta}`,
    `- max_cyclomatic_complexity_delta_source: ${manifest.strategyDecision.backendRecommendation.factors.maxCyclomaticComplexityDeltaSource}`,
    ...(manifest.strategyRecommendation
      ? [
        `- recommended_strategy: ${manifest.strategyRecommendation.strategyLevel}`,
        `- strategy_recommendation_score: ${manifest.strategyRecommendation.score}`,
        `- strategy_recommendation_rationale: ${manifest.strategyRecommendation.rationale}`,
      ]
      : []),
    `- workspace_path: ${manifest.workspacePath || 'inherited from current session'}`,
    `- policy_source: ${manifest.policySource}`,
    `- target_source: ${manifest.target.source}`,
    `- target_resolution: ${manifest.target.resolution}`,
    `- target_tags: ${manifest.target.tags.join(', ') || 'none'}`,
    `- target_warnings: ${manifest.target.warnings.map((warning) => warning.code).join(', ') || 'none'}`,
    `- target_file_count: ${manifest.changeStats?.fileCount ?? manifest.target.files.length}`,
    `- target_line_count: ${targetLineCount}`,
    `- target_line_count_source: ${manifest.changeStats?.lineCountSource ?? 'unknown'}`,
    `- token_budget_mode: ${manifest.tokenBudget.mode}`,
    `- estimated_reviewer_calls: ${manifest.tokenBudget.estimatedReviewerCalls}`,
    `- max_prompt_bytes_per_reviewer: ${manifest.tokenBudget.maxPromptBytesPerReviewer ?? 'none'}`,
    `- estimated_prompt_bytes_per_reviewer: ${manifest.tokenBudget.estimatedPromptBytesPerReviewer ?? 'unknown'}`,
    `- prompt_byte_estimate_source: ${manifest.tokenBudget.promptByteEstimateSource ?? 'none'}`,
    `- prompt_byte_limit_exceeded: ${manifest.tokenBudget.promptByteLimitExceeded ? 'yes' : 'no'}`,
    `- token_budget_decisions: ${formatTokenBudgetDecisionKinds(manifest.tokenBudget.decisions)}`,
    `- budget_limited_reviewers: ${manifest.tokenBudget.skippedReviewerIds.join(', ') || 'none'}`,
    `- core_reviewers: ${formatManifestList(manifest.coreReviewers, 'none')}`,
    `- quality_gate_reviewer: ${manifest.qualityGateReviewer?.subagentId || 'none'}`,
    `- enabled_extra_reviewers: ${formatManifestList(manifest.enabledExtraReviewers, 'none')}`,
    '- skipped_reviewers:',
    ...(manifest.skippedReviewers.length > 0
      ? manifest.skippedReviewers.map(
        (member) => `  - ${member.subagentId}: ${member.reason || 'skipped'}`,
      )
      : ['  - none']),
  ].join('\n');
  const strategyProfiles = team.definition?.strategyProfiles ?? REVIEW_STRATEGY_PROFILES;
  const strategyRules = REVIEW_STRATEGY_LEVELS.map((level) => {
    const definition = strategyProfiles[level];
    const roleEntries = Object.entries(definition.roleDirectives) as [ReviewRoleDirectiveKey, string][];
    const roleLines = roleEntries.map(
      ([role, directive]) => `    - ${role}: ${directive}`,
    );
    return [
      `- ${level}: ${definition.summary}`,
      `  - ${formatStrategyImpact(level, strategyProfiles)}`,
      `  - Default model slot: ${definition.defaultModelSlot}`,
      `  - Prompt directive (fallback): ${definition.promptDirective}`,
      `  - Role-specific directives:`,
      ...roleLines,
    ].join('\n');
  }).join('\n');
  const commonStrategyRules = REVIEW_STRATEGY_COMMON_RULES.reviewerPromptRules
    .map((rule) => `- ${rule}`)
    .join('\n');

  return [
    manifestBlock,
    formatScopeProfileBlock(manifest.scopeProfile),
    formatEvidencePackBlock(manifest.evidencePack),
    formatPreReviewSummaryBlock(manifest.preReviewSummary),
    formatSharedContextCacheBlock(manifest.sharedContextCache),
    formatIncrementalReviewCacheBlock(manifest.incrementalReviewCache),
    'Review work packets:',
    formatWorkPacketBlock(manifest.workPackets),
    'Work packet rules:',
    '- Each reviewer Task prompt must include the matching work packet verbatim.',
    '- Each reviewer and judge Task prompt must include the Scope profile review_depth, risk_focus_tags, max_dependency_hops, and coverage_expectation.',
    '- Include the packet_id in each Task description, for example "Security review [packet reviewer:ReviewSecurity:group-1-of-3]".',
    '- Each reviewer and judge response must echo packet_id and set status to completed, partial_timeout, timed_out, cancelled_by_user, failed, or skipped.',
    '- If the reviewer reports packet_id itself, mark reviewers[].packet_status_source as reported in the final submit_code_review payload.',
    '- If the reviewer omits packet_id but the Task was launched from a packet, infer the packet_id from the Task description or work packet and mark packet_status_source as inferred.',
    '- If packet_id cannot be reported or inferred, mark packet_status_source as missing and explain the confidence impact in coverage_notes.',
    '- If a reviewer response is missing packet_id or status, the judge must treat that reviewer output as lower confidence instead of discarding the whole review.',
    '- Use the pre-generated diff summary for initial orientation and token discipline, but verify claims against assigned files or diffs before reporting findings.',
    '- Evidence pack hunk_hints and contract_hints are orientation only; verify each hinted claim with GetFileDiff, Read, Grep, or Git before reporting it.',
    '- When prompt_byte_limit_exceeded is yes, use the pre-generated diff summary before detailed reads. Do not remove files from assigned_scope or hide unreviewed files; if a file cannot be covered, report it in coverage_notes and reliability_signals.',
    '- Use shared_context_cache entries to reuse read-only GetFileDiff/Read context by cache_key across reviewer packets. Do not duplicate full-file reads when a reusable cached diff or file summary already covers the same path.',
    '- Use incremental_review_cache only when the target fingerprint matches a prior run; preserve completed reviewer outputs by packet_id and rerun only missing, failed, timed-out, or stale packets. If any invalidates_on condition changed, ignore the cache and explain the fresh review boundary.',
    '- The assigned_scope is the default scope for that packet; only widen it when a critical cross-file dependency requires it and note the reason in coverage_notes.',
    'Configured code review team:',
    members || '- No team members available.',
    'Execution policy:',
    executionPolicy,
    'Concurrency policy:',
    concurrencyPolicy,
    'Team execution rules:',
    '- Run only reviewers listed in core_reviewers and enabled_extra_reviewers.',
    '- Do not launch skipped_reviewers.',
    '- If a skipped reviewer has reason not_applicable, mention it in coverage notes without treating it as reduced confidence.',
    '- If a skipped reviewer has reason budget_limited, mention the budget mode and the coverage tradeoff.',
    '- If a skipped reviewer has reason invalid_tooling, report it as a configuration issue and do not reduce confidence in the reviewers that did run.',
    '- If target_resolution is unknown, conditional reviewers may be activated conservatively; report that as coverage context.',
    `- Run the active core reviewer roles first: ${formatManifestList(manifest.coreReviewers, 'none')}.`,
    '- Launch reviewer Tasks by launch_batch. Do not launch a later reviewer batch until every reviewer Task in the earlier batch has completed, failed, timed out, or returned partial_timeout.',
    '- Never launch more reviewer Tasks in one batch than max_parallel_instances. If stagger_seconds is greater than 0, wait that many seconds before starting the next launch_batch.',
    '- Run ReviewJudge only after the reviewer batch finishes, as the quality-gate pass.',
    '- If other extra reviewers are configured and enabled, run them in parallel with the locked reviewers whenever possible.',
    '- When a configured member entry provides model_id, pass model_id with that value to the matching Task call.',
    '- If reviewer_timeout_seconds is greater than 0, pass timeout_seconds with that value to every reviewer Task call.',
    '- If judge_timeout_seconds is greater than 0, pass timeout_seconds with that value to the ReviewJudge Task call.',
    '- If a reviewer Task returns status partial_timeout, treat its output as partial evidence: preserve it in reviewers[].partial_output, mark the reviewer status partial_timeout, and mention the confidence impact in coverage_notes.',
    '- If a reviewer fails or times out without useful partial output, retry that same reviewer at most max_retries_per_role times: reduce its scope, downgrade strategy by one level when possible, use a shorter timeout, and set retry to true on the retry Task call.',
    '- In the final submit_code_review payload, populate reliability_signals for context_pressure, compression_preserved, partial_reviewer, reduced_scope, and user_decision when those conditions apply. Use severity info/warning/action, count when useful, and source runtime/manifest/report/inferred.',
    '- If reviewer_file_split_threshold is greater than 0 and the target file count exceeds it, split files across multiple same-role reviewer instances only up to the concurrency-capped max_same_role_instances for this run.',
    '- Prefer module/workspace-area coherent file groups when splitting reviewer work; avoid mixing unrelated workspace areas in the same packet when the group budget allows it.',
    '- When file splitting is active, each same-role instance must only review its assigned file group. Label instances in the Task description with both group and packet_id (e.g. "Security review [group 1/3] [packet reviewer:ReviewSecurity:group-1-of-3]").',
    '- Do not run ReviewFixer during the review pass.',
    '- Wait for explicit user approval before starting any remediation.',
    '- The Review Quality Inspector acts as a third-party arbiter: it primarily examines reviewer reports for logical consistency and evidence quality, and only uses code inspection tools for targeted spot-checks when a specific claim needs verification.',
    'Review strategy rules:',
    `- Team strategy: ${manifest.strategyLevel}. ${formatStrategyImpact(manifest.strategyLevel, strategyProfiles)}`,
    '- Risk recommendation is advisory; follow team_strategy, member strategy fields, and work-packet strategy for this run unless the user explicitly changes strategy.',
    commonStrategyRules,
    'Review strategy profiles:',
    strategyRules,
  ].join('\n');
}

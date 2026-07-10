import { DEFAULT_REVIEW_TEAM_MODEL } from './defaults';
import {
  REVIEW_STRATEGY_COMMON_RULES,
  REVIEW_STRATEGY_LEVELS,
  REVIEW_STRATEGY_PROFILES,
} from './strategy';
import { toManifestMember } from './manifestMembers';
import type { ReviewDomainTag } from '../reviewTargetClassifier';
import type {
  DeepReviewScopeProfile,
  ReviewRoleDirectiveKey,
  ReviewStrategyLevel,
  ReviewStrategyProfile,
  ReviewTargetEvidence,
  ReviewTeam,
  ReviewTeamManifestMember,
  ReviewTeamPreReviewSummary,
  ReviewTeamRunManifest,
  ReviewTeamTokenBudgetDecision,
  ReviewTeamWorkPacket,
} from './types';

// Prompt formatting consumes an already-built manifest. Keep launch policy and
// side effects in the manifest/service layers so this stays deterministic.
const LOCALE_ONLY_REVIEW_DISQUALIFYING_TAGS: ReviewDomainTag[] = [
  'frontend_ui',
  'frontend_style',
  'frontend_contract',
  'desktop_contract',
  'web_server_contract',
  'transport',
  'api_layer',
  'ai_adapter',
  'test',
  'docs',
  'config',
  'generated_or_lock',
  'unknown',
];

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
    '- Focused-scope profiles are not full-depth coverage. Keep changed files visible in coverage notes and do not describe quick or normal runs as full-depth reviews.',
    '- Reviewers and the judge must carry review_depth and coverage_expectation into their summaries. If review_depth is high_risk_only or risk_expanded, populate reliability_signals with reduced_scope in the final submit_code_review payload.',
  ].join('\n');
}

function formatReviewTargetEvidenceBlock(
  evidence: ReviewTargetEvidence | undefined,
): string {
  if (!evidence) {
    return [
      'Review target evidence:',
      '- unavailable (legacy launch); do not claim exact range or complete coverage',
    ].join('\n');
  }

  const freshnessGuidance = evidence.source === 'git_range'
    ? '- Treat the prepared base/head revisions and diff references as immutable target facts. Never guess or widen refs.'
    : '- Treat the fingerprint and diff references as preparation-time workspace facts. The live workspace may change; never describe it as immutable or silently widen scope.';

  return [
    'Review target evidence:',
    `- source: ${evidence.source}`,
    `- fingerprint: ${evidence.fingerprint}`,
    `- base_revision: ${evidence.baseRevision ?? 'unknown'}`,
    `- head_revision: ${evidence.headRevision ?? 'unknown'}`,
    `- completeness: ${evidence.completeness}`,
    `- workspace_binding: ${evidence.workspaceBinding}`,
    `- file_count: ${evidence.files.length}`,
    `- omitted_file_count: ${evidence.omittedFileCount ?? 0}`,
    `- limitations: ${evidence.limitations.join(', ') || 'none'}`,
    freshnessGuidance,
    '- If completeness is partial, unknown, or stale, preserve that limitation in coverage notes and never convert it into a clean result.',
  ].join('\n');
}

function isLocaleOnlyReviewTarget(manifest: ReviewTeamRunManifest): boolean {
  const includedFiles = manifest.target.files.filter((file) => !file.excluded);
  return includedFiles.length > 0 && includedFiles.every((file) =>
    file.tags.includes('frontend_i18n') &&
    !file.tags.some((tag) => LOCALE_ONLY_REVIEW_DISQUALIFYING_TAGS.includes(tag))
  );
}

function formatLocaleOnlyReviewGuardrail(manifest: ReviewTeamRunManifest): string | null {
  if (!isLocaleOnlyReviewTarget(manifest)) {
    return null;
  }

  return [
    'Locale-only review guardrail:',
    '- The assigned files are locale/i18n resources only.',
    '- Keep ReviewFrontend focused on changed keys, missing or stale translations, placeholder parity, ICU or Fluent syntax, component tag parity, accelerator or formatting consistency, and cross-locale meaning drift.',
    '- Do not broaden into React performance, accessibility, or frontend-backend API contract review unless the locale diff directly references a changed UI/API contract key that requires one-hop verification.',
    '- Prefer GetFileDiff and targeted key lookup before full-file reads. If a full-file read is necessary, explain the exact key family being verified.',
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
    `- review_strategy: ${manifest.strategyLevel}`,
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
  const localeOnlyReviewGuardrail = formatLocaleOnlyReviewGuardrail(manifest);

  return [
    manifestBlock,
    formatReviewTargetEvidenceBlock(manifest.evidencePack?.reviewTarget),
    formatScopeProfileBlock(manifest.scopeProfile),
    ...(localeOnlyReviewGuardrail ? [localeOnlyReviewGuardrail] : []),
    formatPreReviewSummaryBlock(manifest.preReviewSummary),
    'Review work packets:',
    formatWorkPacketBlock(manifest.workPackets),
    'Work packet rules:',
    '- Each reviewer LaunchReviewAgent prompt must include the matching work packet verbatim.',
    '- Each reviewer and judge LaunchReviewAgent prompt must include the Scope profile review_depth, risk_focus_tags, max_dependency_hops, and coverage_expectation.',
    '- Include the packet_id in each LaunchReviewAgent description, for example "Security review [packet reviewer:ReviewSecurity:group-1-of-3]".',
    '- Each reviewer and judge response must echo packet_id and set status to completed, partial_timeout, timed_out, cancelled_by_user, failed, or skipped.',
    '- If the reviewer reports packet_id itself, mark reviewers[].packet_status_source as reported in the final submit_code_review payload.',
    '- If the reviewer omits packet_id but the LaunchReviewAgent call was launched from a packet, infer the packet_id from the LaunchReviewAgent description or work packet and mark packet_status_source as inferred.',
    '- If packet_id cannot be reported or inferred, mark packet_status_source as missing and explain the confidence impact in coverage_notes.',
    '- If a reviewer response is missing packet_id or status, the judge must treat that reviewer output as lower confidence instead of discarding the whole review.',
    '- Use the pre-generated diff summary for initial orientation and token discipline, but verify claims against assigned files or diffs before reporting findings.',
    '- Reuse already-read diff context when possible. Do not repeat full-file reads without a concrete verification need.',
    '- The assigned_scope is the default scope for that packet; only widen it when a critical cross-file dependency requires it and note the reason in coverage_notes.',
    'Review execution plan:',
    members || '- No reviewers available.',
    'Execution policy:',
    executionPolicy,
    'Concurrency policy:',
    concurrencyPolicy,
    'Review execution rules:',
    '- Run only reviewers listed in core_reviewers and enabled_extra_reviewers.',
    '- Do not launch skipped_reviewers.',
    '- If a skipped reviewer has reason not_applicable, mention it in coverage notes without treating it as reduced confidence.',
    '- If a skipped reviewer has reason budget_limited, mention the budget mode and the coverage tradeoff.',
    '- If a skipped reviewer has reason invalid_tooling, report it as a configuration issue and do not reduce confidence in the reviewers that did run.',
    '- If target_resolution is unknown, conditional reviewers may be activated conservatively; report that as coverage context.',
    `- Run the active core reviewer roles first: ${formatManifestList(manifest.coreReviewers, 'none')}.`,
    '- Launch reviewer LaunchReviewAgent calls by launch_batch priority. Earlier batches get reviewer capacity first; queued later-batch calls may start automatically as soon as reviewer capacity frees.',
    '- Never launch more reviewer LaunchReviewAgent calls in one batch than max_parallel_instances. If stagger_seconds is greater than 0, wait that many seconds before starting the next launch_batch.',
    '- Run ReviewJudge only after the reviewer batch finishes, as the final quality-check pass.',
    '- If other extra reviewers are configured and enabled, run them in parallel with the locked reviewers whenever possible.',
    '- When a configured reviewer entry provides model_id, pass model_id with that value to the matching LaunchReviewAgent call.',
    '- If reviewer_timeout_seconds is greater than 0, pass timeout_seconds with that value to every reviewer LaunchReviewAgent call.',
    '- If judge_timeout_seconds is greater than 0, pass timeout_seconds with that value to the ReviewJudge LaunchReviewAgent call.',
    '- If a reviewer LaunchReviewAgent result returns status partial_timeout, treat its output as partial evidence: preserve it in reviewers[].partial_output, mark the reviewer status partial_timeout, and mention the confidence impact in coverage_notes.',
    '- If a reviewer fails or times out without useful partial output, retry that same reviewer at most max_retries_per_role times: focus its scope, use a lower-cost strategy when possible, use a shorter timeout, and set retry to true on the retry LaunchReviewAgent call.',
    '- In the final submit_code_review payload, populate reliability_signals for context_pressure, compression_preserved, partial_reviewer, target_evidence_limited, reduced_scope, and user_decision when those conditions apply. Use severity info/warning/action, count when useful, and source runtime/manifest/report/inferred.',
    '- If reviewer_file_split_threshold is greater than 0 and the target file count exceeds it, split files across multiple same-role reviewer instances only up to the concurrency-capped max_same_role_instances for this run.',
    '- Prefer module/workspace-area coherent file groups when splitting reviewer work; avoid mixing unrelated workspace areas in the same packet when the group budget allows it.',
    '- When file splitting is active, each same-role instance must only review its assigned file group. Label instances in the LaunchReviewAgent description with both group and packet_id (e.g. "Security review [group 1/3] [packet reviewer:ReviewSecurity:group-1-of-3]").',
    '- Do not run ReviewFixer during the review pass.',
    '- Wait for explicit user approval before starting any remediation.',
    '- The Review Quality Inspector acts as a third-party arbiter: it primarily examines reviewer reports for logical consistency and evidence quality, and only uses code inspection tools for targeted spot-checks when a specific claim needs verification.',
    'Review strategy rules:',
    `- Review strategy: ${manifest.strategyLevel}. ${formatStrategyImpact(manifest.strategyLevel, strategyProfiles)}`,
    '- Risk recommendation is advisory; follow review_strategy, reviewer strategy fields, and work-packet strategy for this run unless the user explicitly changes strategy.',
    commonStrategyRules,
    'Review strategy profiles:',
    strategyRules,
  ].join('\n');
}

import type {
  ReviewTargetEvidence,
  ReviewTeam,
  ReviewTeamRunManifest,
  ReviewTeamWorkPacket,
  ReviewTeamWorkPacketScope,
} from './types';

// The typed manifest remains the persistence/runtime contract. This formatter
// gives the orchestrator only the active execution facts it must act on; it
// deliberately omits inactive strategies, duplicated member descriptions, and
// advisory estimates.

interface PromptScopeGroup {
  scope_id: string;
  kind: ReviewTeamWorkPacketScope['kind'];
  target_source: ReviewTeamWorkPacketScope['targetSource'];
  target_resolution: ReviewTeamWorkPacketScope['targetResolution'];
  target_tags: ReviewTeamWorkPacketScope['targetTags'];
  files: string[];
  excluded_file_count: number;
  group_index?: number;
  group_count?: number;
}

function scopeIdentity(scope: ReviewTeamWorkPacketScope): string {
  return JSON.stringify({
    kind: scope.kind,
    targetSource: scope.targetSource,
    targetResolution: scope.targetResolution,
    targetTags: scope.targetTags,
    files: scope.files,
    excludedFileCount: scope.excludedFileCount,
    groupIndex: scope.groupIndex ?? null,
    groupCount: scope.groupCount ?? null,
  });
}

function compactExecutionPlan(workPackets: ReviewTeamWorkPacket[] = []): {
  scope_groups: PromptScopeGroup[];
  active_packets: Array<Record<string, unknown>>;
} {
  const scopeIds = new Map<string, string>();
  const scopeGroups: PromptScopeGroup[] = [];

  const activePackets = workPackets.map((packet) => {
    const identity = scopeIdentity(packet.assignedScope);
    let scopeId = scopeIds.get(identity);
    if (!scopeId) {
      scopeId = `scope-${scopeGroups.length + 1}`;
      scopeIds.set(identity, scopeId);
      scopeGroups.push({
        scope_id: scopeId,
        kind: packet.assignedScope.kind,
        target_source: packet.assignedScope.targetSource,
        target_resolution: packet.assignedScope.targetResolution,
        target_tags: packet.assignedScope.targetTags,
        files: packet.assignedScope.files,
        excluded_file_count: packet.assignedScope.excludedFileCount,
        ...(packet.assignedScope.groupIndex !== undefined
          ? { group_index: packet.assignedScope.groupIndex }
          : {}),
        ...(packet.assignedScope.groupCount !== undefined
          ? { group_count: packet.assignedScope.groupCount }
          : {}),
      });
    }

    return {
      packet_id: packet.packetId,
      display_name: packet.displayName,
      role: packet.roleName,
      phase: packet.phase,
      launch_batch: packet.launchBatch,
      subagent_type: packet.subagentId,
      scope_id: scopeId,
      allowed_tools: packet.allowedTools,
      timeout_seconds: packet.timeoutSeconds,
      required_output_fields: packet.requiredOutputFields,
      strategy: packet.strategyLevel,
      model_id: packet.model,
      prompt_directive: packet.strategyDirective,
    };
  });

  return {
    scope_groups: scopeGroups,
    active_packets: activePackets,
  };
}

function compactTargetEvidence(evidence: ReviewTargetEvidence | undefined) {
  if (!evidence) {
    return {
      status: 'unavailable_legacy_launch',
      instruction: 'Do not claim exact target or complete coverage.',
    };
  }

  return {
    source: evidence.source,
    fingerprint: evidence.fingerprint,
    base_revision: evidence.baseRevision ?? null,
    head_revision: evidence.headRevision ?? null,
    completeness: evidence.completeness,
    workspace_binding: evidence.workspaceBinding,
    file_count: evidence.files.length,
    omitted_file_count: evidence.omittedFileCount ?? 0,
    limitations: evidence.limitations,
  };
}

export function buildReviewTeamPromptBlockContent(
  _team: ReviewTeam,
  manifest: ReviewTeamRunManifest,
): string {
  const executionPlan = compactExecutionPlan(manifest.workPackets);
  const hasActivePackets = executionPlan.active_packets.length > 0;
  const knownTargetFiles = manifest.target.files
    .filter((file) => !file.excluded)
    .map((file) => file.normalizedPath);
  const plannedManagedFiles = new Set(
    manifest.managedReviewPlan
      ? (manifest.workPackets ?? []).flatMap((packet) => packet.assignedScope.files)
      : [],
  );
  const deferredManagedFiles = manifest.managedReviewPlan
    ? knownTargetFiles.filter((file) => !plannedManagedFiles.has(file))
    : [];
  const specialistPool = [
    ...manifest.coreReviewers,
    ...manifest.enabledExtraReviewers,
  ].map((member) => ({
    subagent_type: member.subagentId,
    role: member.roleName,
    model_id: member.model,
  }));
  const compactManifest = {
    review_mode: manifest.reviewMode,
    selected_strategy: manifest.strategyLevel,
    target: {
      source: manifest.target.source,
      resolution: manifest.target.resolution,
      tags: manifest.target.tags,
      file_count: manifest.changeStats?.fileCount ?? manifest.target.files.length,
      ...(manifest.managedReviewPlan ? {} : { files: knownTargetFiles }),
      changed_line_count: manifest.changeStats?.totalLinesChanged ?? null,
      changed_line_count_source: manifest.changeStats?.lineCountSource ?? 'unknown',
    },
    target_evidence: compactTargetEvidence(manifest.evidencePack?.reviewTarget),
    scope_profile: manifest.scopeProfile
      ? {
        review_depth: manifest.scopeProfile.reviewDepth,
        risk_focus_tags: manifest.scopeProfile.riskFocusTags,
        max_dependency_hops: manifest.scopeProfile.maxDependencyHops,
        coverage_expectation: manifest.scopeProfile.coverageExpectation,
      }
      : null,
    execution: {
      ...(hasActivePackets
        ? {
          max_parallel_instances: manifest.concurrencyPolicy.maxParallelInstances,
          max_retries_per_role: manifest.executionPolicy.maxRetriesPerRole,
        }
        : {
          max_specialist_calls: manifest.executionPolicy.maxReviewerCalls ?? 1,
          max_review_agent_executions: manifest.tokenBudget.maxReviewerCalls,
          specialist_timeout_seconds: manifest.executionPolicy.reviewerTimeoutSeconds,
          quality_inspector_timeout_seconds: manifest.executionPolicy.judgeTimeoutSeconds,
        }),
    },
    managed_review_plan: manifest.managedReviewPlan
      ? {
        total_file_count: manifest.managedReviewPlan.totalFileCount,
        planned_file_count: manifest.managedReviewPlan.plannedFileCount,
        deferred_file_count: manifest.managedReviewPlan.deferredFileCount,
        max_files_per_batch: manifest.managedReviewPlan.maxFilesPerBatch,
        max_batches: manifest.managedReviewPlan.maxBatches,
        max_parallel_instances: manifest.managedReviewPlan.maxParallelInstances,
        worker_timeout_seconds: manifest.managedReviewPlan.workerTimeoutSeconds,
        deferred_known_files: deferredManagedFiles,
        deferred_unresolved_file_count: Math.max(
          0,
          manifest.managedReviewPlan.deferredFileCount - deferredManagedFiles.length,
        ),
      }
      : null,
    specialist_pool: specialistPool,
    quality_inspector: manifest.qualityGateReviewer
      ? {
        subagent_type: manifest.qualityGateReviewer.subagentId,
        role: manifest.qualityGateReviewer.roleName,
        model_id: manifest.qualityGateReviewer.model,
      }
      : null,
    ...executionPlan,
  };

  const rules = [
    'Prepared Review execution plan (target already resolved):',
    '```json',
    JSON.stringify(compactManifest, null, 2),
    '```',
    'Execution rules:',
    '- Do not reinterpret, widen, or replace the prepared target.',
    '- Partial, unknown, stale, omitted, or exhausted evidence must remain an explicit coverage limitation.',
    '- Remain read-only. Do not launch ReviewFixer or start remediation without explicit user approval.',
  ];

  if (hasActivePackets) {
    rules.push(
      'Prepared packet execution:',
      '- Launch only active_packets and never exceed max_parallel_instances. launch_batch values are capacity groups, not runtime completion barriers; prefer their numeric order but do not claim a strict batch barrier.',
      '- Use each packet display_name as the user-facing LaunchReviewAgent description. Do not expose packet ids, agent type names, or internal tool names in narrative output.',
      '- Build each reviewer prompt from its packet and referenced scope_group; stay within allowed_tools and the assigned scope.',
      '- Run a judge packet only after all reviewer packets finish.',
      '- Retry only when evidence is still missing and within max_retries_per_role; do not invent additional packets.',
      '- Every packet result must report packet_id and status; preserve missing or inferred packet state in coverage notes.',
      '- LaunchReviewAgent waits in the owning review turn. Never convert managed packets to background Task calls.',
      '- When managed_review_plan.deferred_file_count is non-zero, report partial coverage and list the deferred scope; do not present a clean approval as full coverage.',
      '- Submit one structured final report after the prepared packet plan completes.',
    );
  } else {
    rules.push(
      '- Review the prepared target directly before considering delegation.',
      '- Launch at most one specialist, and only for a concrete uncertainty where a fresh focused pass can materially improve the result.',
      '- Do not use a specialist to repeat the primary review, divide files, or provide routine role coverage.',
      '- Run the quality inspector only when a high-severity finding, conflicting evidence, or low-confidence conclusion needs independent validation.',
      '- If no specialist or quality inspector is needed, complete the report directly.',
      '- Submit one structured final report after review and any justified validation complete.',
    );
  }

  return rules.join('\n');
}

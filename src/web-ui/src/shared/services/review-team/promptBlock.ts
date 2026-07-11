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
  const compactManifest = {
    review_mode: manifest.reviewMode,
    selected_strategy: manifest.strategyLevel,
    target: {
      source: manifest.target.source,
      resolution: manifest.target.resolution,
      tags: manifest.target.tags,
      file_count: manifest.changeStats?.fileCount ?? manifest.target.files.length,
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
      max_parallel_instances: manifest.concurrencyPolicy.maxParallelInstances,
      max_retries_per_role: manifest.executionPolicy.maxRetriesPerRole,
    },
    ...executionPlan,
  };

  return [
    'Prepared Review execution plan (target already resolved):',
    '```json',
    JSON.stringify(compactManifest, null, 2),
    '```',
    'Execution rules:',
    '- Do not reinterpret, widen, or replace the prepared target.',
    '- Launch only active_packets, in launch_batch order, and never exceed max_parallel_instances.',
    '- Build each reviewer prompt from its active packet plus the referenced scope_group; do not repeat unrelated scopes or policies.',
    '- Stay within allowed_tools and the referenced scope. Read one-hop context only when required to verify a concrete finding.',
    '- Run a judge packet only after all reviewer packets finish.',
    '- Every result must report packet_id and status. Infer missing packet_id only from the scheduled packet and mark it inferred.',
    '- Retry a failed or timed-out role only when evidence is still missing and within max_retries_per_role.',
    '- Partial, unknown, stale, omitted, or exhausted evidence must remain an explicit coverage limitation.',
    '- Remain read-only. Do not launch ReviewFixer or start remediation without explicit user approval.',
    '- Submit one structured final report after the active plan completes.',
  ].join('\n');
}

import type {
  ExternalApplicationHealthV2,
  ExternalApplicationRecoveryActionV2,
  ExternalApplicationSnapshotV2,
  ExternalIntegrationAccess,
  ExternalIntegrationMode,
  ExternalSourceCatalogSnapshot,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import type {
  ExternalSourceCapabilityCounts,
  ExternalSourcePresentationGroup,
} from '../../externalSourcePresentation';

/**
 * Application status shown on the overview.
 *
 * V1 has no application connection facts, so `connected` is derived from the
 * effective integration mode rather than a real connection lifecycle. The
 * design's "temporarily unavailable" state is intentionally absent: V1 cannot
 * separate "not installed" from "probe failed", and guessing would mislead.
 */
export type ExternalApplicationStatus =
  | 'needs_attention'
  | 'connected'
  | 'connected_custom'
  | 'configuration_available'
  | 'temporarily_unavailable'
  | 'discovered'
  | 'checking'
  | 'no_configuration';

export type ExternalApplicationAction =
  | 'connect'
  | 'manage'
  | 'review'
  | 'view'
  | 'retry'
  | 'view_reason'
  | 'none';

export interface ExternalApplicationCapabilityPlan {
  capabilityId: string;
  /** Access this capability reaches once the ecosystem switches to recommended. */
  recommendedAccess: ExternalIntegrationAccess;
  /** Access currently enforced by the authoritative effective policy. */
  effectiveAccess: ExternalIntegrationAccess;
  count: number;
}

export interface ExternalApplicationActiveCapability {
  capabilityId: string;
  count: number;
}

export interface ExternalApplicationView {
  applicationId?: string;
  ecosystemId: string;
  displayName: string;
  mode?: ExternalIntegrationMode;
  status: ExternalApplicationStatus;
  primaryAction: ExternalApplicationAction;
  enabled: boolean;
  /** Host-authoritative aggregate for V2; V1 continues to expose capability counts. */
  enabledCount?: number;
  /** Host-authoritative V2 secondary facts; omitted for the inferred V1 projection. */
  health?: ExternalApplicationHealthV2;
  blockedCount?: number;
  conflictCount?: number;
  recoveryActions?: ExternalApplicationRecoveryActionV2[];
  counts: ExternalSourceCapabilityCounts;
  activeCapabilities: ExternalApplicationActiveCapability[];
  sourceCount: number;
  locations: string[];
  /** Attention items that could be attributed to this ecosystem. */
  attentionCount: number;
  /** What switching to `recommended` would enable, used by the connect dialog. */
  connectPlan: ExternalApplicationCapabilityPlan[];
}

export interface ExternalApplicationsView {
  applications: ExternalApplicationView[];
  /** Attention items with no ecosystem identity (catalog diagnostics, policy). */
  unattributedAttentionCount: number;
  totalAttentionCount: number;
}

export function buildExternalApplicationsViewV2(
  snapshot: ExternalApplicationSnapshotV2,
): ExternalApplicationsView {
  return {
    applications: snapshot.applications.map((application) => ({
      applicationId: application.applicationId,
      ecosystemId: application.ecosystemId,
      displayName: application.displayName,
      status: application.effectiveStatus,
      primaryAction: application.primaryAction,
      enabled: application.connection === 'connected',
      enabledCount: application.enabledCount,
      health: application.health,
      blockedCount: application.blockedCount,
      conflictCount: application.conflictCount,
      recoveryActions: application.recoveryActions,
      counts: { commands: 0, tools: 0, agents: 0, mcps: 0 },
      activeCapabilities: [],
      sourceCount: 0,
      locations: [],
      attentionCount: application.pendingReviewCount,
      connectPlan: [],
    })),
    unattributedAttentionCount: 0,
    totalAttentionCount: snapshot.reviewSummary?.totalCount ?? 0,
  };
}

const CAPABILITY_COUNT_FIELD: Record<string, keyof ExternalSourceCapabilityCounts> = {
  command: 'commands',
  tool: 'tools',
  subagent: 'agents',
  mcp: 'mcps',
};

function sourcePairKey(providerId: string, sourceId: string): string {
  return `${providerId}\u0000${sourceId}`;
}

/**
 * Maps every discovered source pair to its ecosystem so attention items that
 * only carry a source identity can still be attributed to an application.
 */
function ecosystemBySourcePair(snapshot: ExternalSourceCatalogSnapshot): Map<string, string> {
  const bySource = new Map<string, string>();
  for (const source of snapshot.sources) {
    bySource.set(
      sourcePairKey(source.record.key.providerId, source.record.key.sourceId),
      source.record.ecosystemId,
    );
  }
  return bySource;
}

function addAttention(counts: Map<string, number>, ecosystemId: string | undefined): boolean {
  if (!ecosystemId) return false;
  counts.set(ecosystemId, (counts.get(ecosystemId) ?? 0) + 1);
  return true;
}

/**
 * Attributes pending approvals and unresolved conflicts to ecosystems.
 *
 * Items that cannot be attributed — catalog-level diagnostics, policy
 * incompatibility, conflict candidates without a source — are counted
 * separately instead of being spread across applications.
 */
function attentionByEcosystem(
  snapshot: ExternalSourceCatalogSnapshot,
): { byEcosystem: Map<string, number>; unattributed: number } {
  const byEcosystem = new Map<string, number>();
  const bySource = ecosystemBySourcePair(snapshot);
  // Diagnostics and policy incompatibility are system status, not user
  // decisions. They must not inflate the review count shown in the overview.
  let unattributed = 0;

  for (const request of snapshot.toolApprovalRequests ?? []) {
    const ecosystemId = bySource.get(sourcePairKey(
      request.targetId.source.providerId,
      request.targetId.source.sourceId,
    ));
    if (!addAttention(byEcosystem, ecosystemId)) unattributed += 1;
  }

  for (const request of snapshot.mcpApprovalRequests ?? []) {
    const ecosystemId = bySource.get(sourcePairKey(
      request.definition.id.source.providerId,
      request.definition.id.source.sourceId,
    ));
    if (!addAttention(byEcosystem, ecosystemId)) unattributed += 1;
  }

  const subagentById = new Map(
    (snapshot.subagents ?? []).map((agent) => [agent.candidateId, agent]),
  );
  for (const candidateId of snapshot.pendingSubagentApprovals ?? []) {
    const agent = subagentById.get(candidateId);
    // A subagent may span several sources; the first resolvable one owns the
    // item so a single approval is never counted twice.
    const ecosystemId = agent?.sourceKeys
      .map((key) => bySource.get(sourcePairKey(key.providerId, key.sourceId)))
      .find((value): value is string => Boolean(value));
    if (!addAttention(byEcosystem, ecosystemId)) unattributed += 1;
  }

  for (const conflict of snapshot.commandConflicts ?? []) {
    if (conflict.selectedCandidateId) continue;
    const ecosystemIds = new Set(conflict.candidates.map((candidate) => candidate.ecosystemId));
    if (ecosystemIds.size === 1) {
      addAttention(byEcosystem, [...ecosystemIds][0]);
    } else {
      // Cross-ecosystem collisions belong to no single application.
      unattributed += 1;
    }
  }

  for (const conflict of snapshot.toolConflicts ?? []) {
    if (conflict.selectedCandidateId) continue;
    const ecosystemIds = new Set(
      conflict.candidates
        .map((candidate) => (candidate.source
          ? bySource.get(sourcePairKey(candidate.source.providerId, candidate.source.sourceId))
          : undefined))
        .filter((value): value is string => Boolean(value)),
    );
    if (ecosystemIds.size === 1) {
      addAttention(byEcosystem, [...ecosystemIds][0]);
    } else {
      unattributed += 1;
    }
  }

  for (const conflict of snapshot.mcpConflicts ?? []) {
    if (conflict.selectedCandidateId) continue;
    const ecosystemIds = new Set(
      conflict.candidates
        .map((candidate) => (candidate.source
          ? bySource.get(sourcePairKey(candidate.source.providerId, candidate.source.sourceId))
          : undefined))
        .filter((value): value is string => Boolean(value)),
    );
    if (ecosystemIds.size === 1) {
      addAttention(byEcosystem, [...ecosystemIds][0]);
    } else {
      unattributed += 1;
    }
  }

  for (const conflict of snapshot.subagentConflicts ?? []) {
    if (conflict.selectedCandidateId) continue;
    // Subagent conflict candidates carry no source identity in V1.
    unattributed += 1;
  }

  return { byEcosystem, unattributed };
}

function statusFor(
  mode: ExternalIntegrationMode,
  sourceCount: number,
  attentionCount: number,
  discoveryPending: boolean,
): ExternalApplicationStatus {
  if (attentionCount > 0) return 'needs_attention';
  if (sourceCount === 0) return discoveryPending ? 'checking' : 'no_configuration';
  if (mode === 'recommended') return 'connected';
  if (mode === 'custom') return 'connected_custom';
  return 'discovered';
}

function actionFor(status: ExternalApplicationStatus): ExternalApplicationAction {
  switch (status) {
    case 'needs_attention':
      return 'review';
    case 'connected':
    case 'connected_custom':
      return 'manage';
    case 'discovered':
      return 'connect';
    default:
      return 'none';
  }
}

/**
 * Builds the application-level overview from a V1 snapshot.
 *
 * Pure derivation: no host calls, no policy decisions beyond reading the
 * effective mode the host already computed.
 */
export function buildExternalApplicationsView(
  snapshot: ExternalSourceCatalogSnapshot | null,
  groups: ExternalSourcePresentationGroup[],
  policyScope: 'user' | 'workspace',
): ExternalApplicationsView {
  if (!snapshot) {
    return { applications: [], unattributedAttentionCount: 0, totalAttentionCount: 0 };
  }

  const policy = snapshot.integrationPolicy;
  const effective = policyScope === 'workspace' ? policy.effective : policy.globalEffective;
  const { byEcosystem, unattributed } = attentionByEcosystem(snapshot);

  const applications = policy.registeredEcosystems.map((descriptor) => {
    const ecosystemId = descriptor.ecosystemId;
    const ecosystemGroups = groups.filter((group) => group.ecosystemId === ecosystemId);
    const sources = snapshot.sources.filter(
      (source) => source.record.ecosystemId === ecosystemId,
    );
    const counts = ecosystemGroups.reduce<ExternalSourceCapabilityCounts>((total, group) => ({
      commands: total.commands + group.counts.commands,
      tools: total.tools + group.counts.tools,
      agents: total.agents + group.counts.agents,
      mcps: total.mcps + group.counts.mcps,
    }), { commands: 0, tools: 0, agents: 0, mcps: 0 });

    const ecosystemPolicy = effective.ecosystems[ecosystemId];
    const mode = ecosystemPolicy?.mode ?? 'recommended';
    const attentionCount = byEcosystem.get(ecosystemId) ?? 0;
    const status = statusFor(mode, sources.length, attentionCount, snapshot.discoveryPending);
    const enabled = effective.enabled && (mode === 'recommended' || mode === 'custom');
    const activeCapabilities = descriptor.capabilities.flatMap((capability) => {
      const countField = CAPABILITY_COUNT_FIELD[capability.capabilityId];
      const count = countField ? counts[countField] : 0;
      const access = ecosystemPolicy?.capabilities?.[capability.capabilityId];
      return enabled && count > 0 && access !== 'disabled' && access !== 'discover_only'
        ? [{ capabilityId: capability.capabilityId, count }]
        : [];
    });

    return {
      ecosystemId,
      displayName: descriptor.displayName,
      mode,
      status,
      primaryAction: actionFor(status),
      enabled,
      counts,
      activeCapabilities,
      sourceCount: sources.length,
      locations: Array.from(new Set(sources.map((source) => source.record.location))),
      attentionCount,
      connectPlan: descriptor.capabilities.map((capability) => ({
        capabilityId: capability.capabilityId,
        recommendedAccess: capability.recommendedAccess,
        effectiveAccess: ecosystemPolicy?.capabilities?.[capability.capabilityId] ?? 'disabled',
        count: CAPABILITY_COUNT_FIELD[capability.capabilityId]
          ? counts[CAPABILITY_COUNT_FIELD[capability.capabilityId]]
          : 0,
      })),
    };
  });

  const totalAttentionCount = applications.reduce(
    (total, application) => total + application.attentionCount,
    unattributed,
  );

  return {
    applications,
    unattributedAttentionCount: unattributed,
    totalAttentionCount,
  };
}

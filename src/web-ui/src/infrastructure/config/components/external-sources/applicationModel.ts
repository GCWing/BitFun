import type {
  ExternalSourceCatalogSnapshot,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';

export interface ExternalApplicationView {
  ecosystemId: string;
  displayName: string;
  enabled: boolean;
  attentionCount: number;
}

function sourcePairKey(providerId: string, sourceId: string): string {
  return `${providerId}\u0000${sourceId}`;
}

function ecosystemBySourcePair(snapshot: ExternalSourceCatalogSnapshot): Map<string, string> {
  return new Map(snapshot.sources.map((source) => [
    sourcePairKey(source.record.key.providerId, source.record.key.sourceId),
    source.record.ecosystemId,
  ]));
}

function addAttention(counts: Map<string, number>, ecosystemId: string | undefined): void {
  if (!ecosystemId) return;
  counts.set(ecosystemId, (counts.get(ecosystemId) ?? 0) + 1);
}

function attentionByEcosystem(
  snapshot: ExternalSourceCatalogSnapshot,
): Map<string, number> {
  const byEcosystem = new Map<string, number>();
  const bySource = ecosystemBySourcePair(snapshot);

  for (const request of snapshot.toolApprovalRequests ?? []) {
    addAttention(byEcosystem, bySource.get(sourcePairKey(
      request.targetId.source.providerId,
      request.targetId.source.sourceId,
    )));
  }

  for (const request of snapshot.mcpApprovalRequests ?? []) {
    addAttention(byEcosystem, bySource.get(sourcePairKey(
      request.definition.id.source.providerId,
      request.definition.id.source.sourceId,
    )));
  }

  const subagentById = new Map(
    (snapshot.subagents ?? []).map((agent) => [agent.candidateId, agent]),
  );
  for (const candidateId of snapshot.pendingSubagentApprovals ?? []) {
    const ecosystemId = subagentById.get(candidateId)?.sourceKeys
      .map((key) => bySource.get(sourcePairKey(key.providerId, key.sourceId)))
      .find((value): value is string => Boolean(value));
    addAttention(byEcosystem, ecosystemId);
  }

  const attributeConflict = (ecosystemIds: Set<string>) => {
    if (ecosystemIds.size === 1) {
      addAttention(byEcosystem, [...ecosystemIds][0]);
    }
  };

  for (const conflict of snapshot.commandConflicts ?? []) {
    if (!conflict.selectedCandidateId) {
      attributeConflict(new Set(conflict.candidates.map((candidate) => candidate.ecosystemId)));
    }
  }

  for (const conflict of snapshot.toolConflicts ?? []) {
    if (!conflict.selectedCandidateId) {
      attributeConflict(new Set(conflict.candidates.flatMap((candidate) => {
        if (!candidate.source) return [];
        const ecosystemId = bySource.get(sourcePairKey(
          candidate.source.providerId,
          candidate.source.sourceId,
        ));
        return ecosystemId ? [ecosystemId] : [];
      })));
    }
  }

  for (const conflict of snapshot.mcpConflicts ?? []) {
    if (!conflict.selectedCandidateId) {
      attributeConflict(new Set(conflict.candidates.flatMap((candidate) => {
        if (!candidate.source) return [];
        const ecosystemId = bySource.get(sourcePairKey(
          candidate.source.providerId,
          candidate.source.sourceId,
        ));
        return ecosystemId ? [ecosystemId] : [];
      })));
    }
  }

  return byEcosystem;
}

/**
 * Builds the application overview from the existing source catalog. Registration
 * alone is not user-visible: an application appears only after discovery, an
 * owner action, or an explicit user policy exists for it.
 */
export function buildExternalApplicationsView(
  snapshot: ExternalSourceCatalogSnapshot | null,
  policyScope: 'user' | 'workspace',
): ExternalApplicationView[] {
  if (!snapshot) return [];

  const policy = snapshot.integrationPolicy;
  const effective = policyScope === 'workspace' ? policy.effective : policy.globalEffective;
  const byEcosystem = attentionByEcosystem(snapshot);

  const applications = policy.registeredEcosystems.flatMap((descriptor) => {
    const ecosystemId = descriptor.ecosystemId;
    const discovered = snapshot.sources.some(
      (source) => source.record.ecosystemId === ecosystemId,
    );
    const explicitlyConfigured = Boolean(
      policy.userDefaults.ecosystems?.[ecosystemId]
      || policy.workspaceOverride?.ecosystems?.[ecosystemId],
    );
    const attentionCount = byEcosystem.get(ecosystemId) ?? 0;
    if (!discovered && !explicitlyConfigured && attentionCount === 0) return [];

    const ecosystemPolicy = effective.ecosystems[ecosystemId];
    const mode = ecosystemPolicy?.mode ?? 'recommended';
    return [{
      ecosystemId,
      displayName: descriptor.displayName,
      enabled: effective.enabled && (mode === 'recommended' || mode === 'custom'),
      attentionCount,
    }];
  });

  return applications;
}

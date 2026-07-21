import type {
  ExternalSourceCatalogSnapshot,
  ExternalSourceLifecycle,
  ExternalSourceRecord,
  ExternalSourceScope,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';

export interface ExternalSourceCapabilityCounts {
  commands: number;
  tools: number;
  agents: number;
  mcps: number;
}

export type ExternalSourceCapability = 'command' | 'tool' | 'subagent' | 'mcp' | 'source';

export interface ExternalSourcePresentationMember {
  stableKey: string;
  lifecycle: ExternalSourceLifecycle;
  scope: ExternalSourceScope;
  capability: ExternalSourceCapability;
  enabled: boolean;
  mutable: boolean;
}

export interface ExternalSourcePresentationGroup {
  key: string;
  ecosystemId: string;
  scopes: ExternalSourceScope[];
  displayName: string;
  location: string;
  lifecycle: ExternalSourceLifecycle;
  members: ExternalSourcePresentationMember[];
  counts: ExternalSourceCapabilityCounts;
  diagnostics: NonNullable<ExternalSourceRecord['diagnostics']>;
}

type ExternalSourceDiagnostic = NonNullable<ExternalSourceRecord['diagnostics']>[number];

function sourcePairKey(providerId: string, sourceId: string): string {
  return `${providerId}\u0000${sourceId}`;
}

function normalizeLocation(location: string): string {
  const normalized = location.trim().replace(/\\/g, '/');
  return normalized.length > 1 ? normalized.replace(/\/+$/, '') : normalized;
}

function presentationKey(source: ExternalSourceCatalogSnapshot['sources'][number]): string {
  // Older Peer Hosts do not provide a server-issued group id. Keeping those
  // entries separate is safer than coalescing distinct raw paths that happen
  // to share the same redacted display location.
  return source.presentationGroupId ?? source.stableKey;
}

function commandCountsBySource(
  snapshot: ExternalSourceCatalogSnapshot,
): Map<string, number> {
  const namesBySource = new Map<string, Set<string>>();
  const add = (providerId: string, sourceId: string, commandName: string) => {
    const key = sourcePairKey(providerId, sourceId);
    const names = namesBySource.get(key) ?? new Set<string>();
    names.add(commandName.toLowerCase());
    namesBySource.set(key, names);
  };

  for (const command of snapshot.commands) {
    const source = command.definition.id.source;
    add(source.providerId, source.sourceId, command.definition.name);
  }
  for (const conflict of snapshot.commandConflicts ?? []) {
    for (const candidate of conflict.candidates) {
      add(candidate.source.providerId, candidate.source.sourceId, conflict.commandName);
    }
  }

  return new Map(Array.from(namesBySource, ([key, names]) => [key, names.size]));
}

function increment(counts: Map<string, number>, key: string): void {
  counts.set(key, (counts.get(key) ?? 0) + 1);
}

function capabilityCountsBySource(snapshot: ExternalSourceCatalogSnapshot): {
  commands: Map<string, number>;
  tools: Map<string, number>;
  agents: Map<string, number>;
  mcps: Map<string, number>;
} {
  const tools = new Map<string, number>();
  for (const tool of snapshot.tools ?? []) {
    const source = tool.definition.id.target.source;
    increment(tools, sourcePairKey(source.providerId, source.sourceId));
  }

  const agents = new Map<string, number>();
  for (const agent of snapshot.subagents ?? []) {
    for (const source of agent.sourceKeys) {
      increment(agents, sourcePairKey(source.providerId, source.sourceId));
    }
  }

  const mcps = new Map<string, number>();
  for (const server of snapshot.mcpServers ?? []) {
    const source = server.definition.id.source;
    increment(mcps, sourcePairKey(source.providerId, source.sourceId));
  }

  return {
    commands: commandCountsBySource(snapshot),
    tools,
    agents,
    mcps,
  };
}

function countsForSource(
  counts: ReturnType<typeof capabilityCountsBySource>,
  source: ExternalSourceCatalogSnapshot['sources'][number],
): ExternalSourceCapabilityCounts {
  const pair = sourcePairKey(source.record.key.providerId, source.record.key.sourceId);
  return {
    commands: counts.commands.get(pair) ?? 0,
    tools: counts.tools.get(pair) ?? 0,
    agents: counts.agents.get(pair) ?? 0,
    mcps: counts.mcps.get(pair) ?? 0,
  };
}

function sourceCapability(
  source: ExternalSourceCatalogSnapshot['sources'][number],
  counts: ExternalSourceCapabilityCounts,
): ExternalSourceCapability {
  const populated = (Object.entries(counts) as Array<[keyof ExternalSourceCapabilityCounts, number]>)
    .filter(([, count]) => count > 0)
    .map(([capability]) => capability);
  if (populated.length === 1) {
    return {
      commands: 'command',
      tools: 'tool',
      agents: 'subagent',
      mcps: 'mcp',
    }[populated[0]] as ExternalSourceCapability;
  }

  const identity = `${source.record.key.providerId} ${source.record.sourceKind}`.toLowerCase();
  if (identity.includes('mcp')) return 'mcp';
  if (identity.includes('subagent') || identity.includes('agent')) return 'subagent';
  if (identity.includes('tool')) return 'tool';
  if (identity.includes('command') || identity.includes('prompt')) return 'command';
  return 'source';
}

function combinedLifecycle(members: ExternalSourcePresentationMember[]): ExternalSourceLifecycle {
  const lifecycles = new Set(members.map((member) => member.lifecycle));
  if (lifecycles.size === 1) {
    return members[0]?.lifecycle ?? 'unavailable';
  }
  if (lifecycles.has('unavailable')) return 'unavailable';
  if (lifecycles.has('degraded') || lifecycles.has('suppressed') || lifecycles.has('removed')) {
    return 'degraded';
  }
  if (lifecycles.has('restricted')) return 'restricted';
  if (lifecycles.has('using_last_valid_version')) return 'using_last_valid_version';
  return 'available';
}

export function externalSourceDiagnosticKey(diagnostic: ExternalSourceDiagnostic): string {
  return JSON.stringify([
    diagnostic.severity.toLowerCase(),
    diagnostic.assetKind ?? '',
    diagnostic.code,
    diagnostic.message,
  ]);
}

function deduplicateDiagnostics(
  sources: ExternalSourceCatalogSnapshot['sources'],
): NonNullable<ExternalSourceRecord['diagnostics']> {
  const diagnostics = new Map<string, ExternalSourceDiagnostic>();
  for (const source of sources) {
    for (const diagnostic of source.record.diagnostics ?? []) {
      if (diagnostic.severity.toLowerCase() === 'info') continue;
      diagnostics.set(externalSourceDiagnosticKey(diagnostic), diagnostic);
    }
  }
  return Array.from(diagnostics.values());
}

export function catalogDiagnosticsWithoutSourceDuplicates(
  snapshot: ExternalSourceCatalogSnapshot,
  groups: ExternalSourcePresentationGroup[],
): NonNullable<ExternalSourceCatalogSnapshot['diagnostics']> {
  const sourceDiagnosticKeys = new Set(groups.flatMap((group) => (
    group.diagnostics.map(externalSourceDiagnosticKey)
  )));
  const diagnostics = new Map<string, ExternalSourceDiagnostic>();
  for (const diagnostic of snapshot.diagnostics ?? []) {
    const key = externalSourceDiagnosticKey(diagnostic);
    if (!sourceDiagnosticKeys.has(key)) diagnostics.set(key, diagnostic);
  }
  return Array.from(diagnostics.values());
}

const SCOPE_ORDER: ExternalSourceScope[] = [
  'user_global',
  'remote_user',
  'project',
  'remote_project',
  'workspace_local',
];

export function buildExternalSourcePresentationGroups(
  snapshot: ExternalSourceCatalogSnapshot,
): ExternalSourcePresentationGroup[] {
  const countsBySource = capabilityCountsBySource(snapshot);
  const groupedSources = new Map<string, ExternalSourceCatalogSnapshot['sources']>();

  for (const source of snapshot.sources) {
    const key = presentationKey(source);
    const sources = groupedSources.get(key) ?? [];
    sources.push(source);
    groupedSources.set(key, sources);
  }

  const groups: ExternalSourcePresentationGroup[] = [];
  for (const [key, sources] of groupedSources) {
    const representative = sources[0];
    if (!representative) continue;

    const sourceCounts = new Map(sources.map((source) => [
      source.stableKey,
      countsForSource(countsBySource, source),
    ]));
    const counts = Array.from(sourceCounts.values()).reduce<ExternalSourceCapabilityCounts>(
      (total, current) => ({
        commands: total.commands + current.commands,
        tools: total.tools + current.tools,
        agents: total.agents + current.agents,
        mcps: total.mcps + current.mcps,
      }),
      { commands: 0, tools: 0, agents: 0, mcps: 0 },
    );
    const diagnostics = deduplicateDiagnostics(sources);
    const totalAssets = Object.values(counts).reduce((total, count) => total + count, 0);
    const hasActionableLifecycle = sources.some((source) => source.lifecycle !== 'available');
    if (totalAssets === 0 && diagnostics.length === 0 && !hasActionableLifecycle) continue;

    const members = sources
      .filter((source) => {
        const memberCounts = sourceCounts.get(source.stableKey);
        const assetCount = memberCounts
          ? Object.values(memberCounts).reduce((total, count) => total + count, 0)
          : 0;
        const hasDiagnostic = (source.record.diagnostics ?? [])
          .some((diagnostic) => diagnostic.severity.toLowerCase() !== 'info');
        return assetCount > 0 || hasDiagnostic || source.lifecycle !== 'available';
      })
      .map((source) => ({
        stableKey: source.stableKey,
        lifecycle: source.lifecycle,
        scope: source.record.scope,
        capability: sourceCapability(
          source,
          sourceCounts.get(source.stableKey) ?? { commands: 0, tools: 0, agents: 0, mcps: 0 },
        ),
        enabled: source.lifecycle !== 'suppressed' && source.lifecycle !== 'removed',
        mutable: source.lifecycle !== 'removed',
      }))
      .sort((left, right) => left.stableKey.localeCompare(right.stableKey));
    const scopes = Array.from(new Set(sources.map((source) => source.record.scope)))
      .sort((left, right) => SCOPE_ORDER.indexOf(left) - SCOPE_ORDER.indexOf(right));

    groups.push({
      key,
      ecosystemId: representative.record.ecosystemId,
      scopes,
      displayName: representative.record.displayName,
      location: normalizeLocation(representative.record.location),
      lifecycle: combinedLifecycle(members),
      members,
      counts,
      diagnostics,
    });
  }

  return groups;
}

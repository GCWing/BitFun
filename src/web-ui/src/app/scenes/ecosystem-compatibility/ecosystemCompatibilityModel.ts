import type { AcpClientInfo } from '@/infrastructure/api/service-api/ACPClientAPI';
import type { ExternalSourceCatalogSnapshot } from '@/infrastructure/api/service-api/ExternalSourcesAPI';

export type EcosystemProductId =
  | 'claude-code'
  | 'codex'
  | 'pi'
  | 'dsh'
  | 'opencode';

export type CompatibilityCapabilityId =
  | 'command'
  | 'tool'
  | 'subagent'
  | 'mcp'
  | 'runtime';

export type EcosystemProductStatus =
  | 'connected'
  | 'detected'
  | 'configured'
  | 'available'
  | 'development';

export type EcosystemProductGroup = 'connected' | 'available' | 'other';

export interface EcosystemProductSpec {
  id: EcosystemProductId;
  name: string;
  ecosystemId?: string;
  acpClientId?: string;
  development?: boolean;
  searchTerms: readonly string[];
}

export interface CompatibilityCapabilityCounts {
  command: number;
  tool: number;
  subagent: number;
  mcp: number;
  runtime: number;
}

export interface EcosystemProductRuntime {
  spec: EcosystemProductSpec;
  status: EcosystemProductStatus;
  group: EcosystemProductGroup;
  sources: ExternalSourceCatalogSnapshot['sources'];
  acpClients: AcpClientInfo[];
  acpClient?: AcpClientInfo;
  capabilityIds: CompatibilityCapabilityId[];
  capabilityCounts: CompatibilityCapabilityCounts;
  adapterRevision?: string;
  sourceLocation?: string;
  executionDomainId?: string;
}

/**
 * Product-level objects that can participate in import and reuse. This catalog
 * is intentionally broader than the external-runtime capability contract: the
 * UI keeps every applicable object type in the compatibility catalog visible
 * even when the selected product exposes no candidates or no direct import
 * bridge yet. Object types absent from the upstream product are omitted.
 */
export type EcosystemImportItemKind =
  | 'account'
  | 'settings'
  | 'command'
  | 'tool'
  | 'subagent'
  | 'skill'
  | 'mcp'
  | 'hook'
  | 'memory'
  | 'plugin'
  | 'pet';

export type EcosystemCompatibilitySupport =
  | 'adapted'
  | 'notAdapted'
  | 'notApplicable';

export type EcosystemCompatibilityDetection = 'catalog' | 'owner';

export interface EcosystemImportItem {
  id: string;
  kind: EcosystemImportItemKind;
  name: string;
  description?: string;
  sourceName: string;
  sourceLocation?: string;
  candidateId?: string;
  support: EcosystemCompatibilitySupport;
  detection: EcosystemCompatibilityDetection;
  nativeImportSupported: boolean;
  discovered: boolean;
}

export const ECOSYSTEM_IMPORT_ITEM_KINDS: readonly EcosystemImportItemKind[] = [
  'account',
  'settings',
  'command',
  'tool',
  'subagent',
  'skill',
  'mcp',
  'hook',
  'memory',
  'plugin',
  'pet',
];

/**
 * Current product-to-BitFun adaptation facts. These are deliberately explicit:
 * a missing discovery result must not imply that every upstream product offers
 * every object type. Catalog-backed kinds are discovered by this page; Skill
 * and Hook reuse stays with their existing capability owners.
 */
const PRODUCT_ADAPTED_KINDS = {
  'claude-code': ['command', 'subagent', 'skill', 'mcp', 'hook'],
  codex: ['subagent', 'skill', 'mcp', 'hook'],
  pi: [],
  dsh: [],
  opencode: ['command', 'tool', 'subagent', 'skill', 'mcp', 'hook'],
} as const satisfies Record<EcosystemProductId, readonly EcosystemImportItemKind[]>;

const PRODUCT_NOT_APPLICABLE_KINDS = {
  'claude-code': ['tool', 'pet'],
  codex: ['command', 'tool'],
  pi: ['pet'],
  dsh: ['pet'],
  opencode: ['pet'],
} as const satisfies Record<EcosystemProductId, readonly EcosystemImportItemKind[]>;

const OWNER_DETECTED_KINDS = new Set<EcosystemImportItemKind>(['skill', 'hook']);

export function ecosystemCompatibilitySupport(
  productId: EcosystemProductId,
  kind: EcosystemImportItemKind,
): EcosystemCompatibilitySupport {
  const adaptedKinds = PRODUCT_ADAPTED_KINDS[productId] as readonly EcosystemImportItemKind[];
  if (adaptedKinds.includes(kind)) return 'adapted';

  const notApplicableKinds = PRODUCT_NOT_APPLICABLE_KINDS[productId] as readonly EcosystemImportItemKind[];
  return notApplicableKinds.includes(kind) ? 'notApplicable' : 'notAdapted';
}

/**
 * Presentation catalog for product families already represented by a shipped
 * adapter or ACP preset. Pi stays explicit because it appears in the product
 * concept, but is marked as development rather than being presented as support.
 */
export const ECOSYSTEM_PRODUCT_SPECS: readonly EcosystemProductSpec[] = [
  {
    id: 'claude-code',
    name: 'Claude Code',
    ecosystemId: 'claude-code',
    acpClientId: 'claude-code',
    searchTerms: ['claude', 'anthropic', 'agent', 'mcp', 'command', 'acp'],
  },
  {
    id: 'codex',
    name: 'Codex',
    ecosystemId: 'codex',
    acpClientId: 'codex',
    searchTerms: ['openai', 'agent', 'mcp', 'acp'],
  },
  {
    id: 'pi',
    name: 'Pi',
    development: true,
    searchTerms: ['pi', 'agent'],
  },
  {
    id: 'dsh',
    name: 'DeepSeek Harness',
    acpClientId: 'dsh',
    searchTerms: ['deepseek', 'harness', 'dsh', 'agent', 'acp'],
  },
  {
    id: 'opencode',
    name: 'OpenCode',
    ecosystemId: 'opencode',
    acpClientId: 'opencode',
    searchTerms: ['open code', 'agent', 'command', 'tool', 'mcp', 'acp'],
  },
] as const;

function sourcePairKey(providerId: string, sourceId: string): string {
  return `${providerId}\u0000${sourceId}`;
}

function itemSource(
  sources: ExternalSourceCatalogSnapshot['sources'],
  source: { providerId: string; sourceId: string },
) {
  const key = sourcePairKey(source.providerId, source.sourceId);
  return sources.find((candidate) => sourcePairKey(
    candidate.record.key.providerId,
    candidate.record.key.sourceId,
  ) === key);
}

function productSources(
  snapshot: ExternalSourceCatalogSnapshot | null,
  ecosystemId: string | undefined,
): ExternalSourceCatalogSnapshot['sources'] {
  if (!snapshot || !ecosystemId) return [];
  return snapshot.sources.filter((source) => source.record.ecosystemId === ecosystemId);
}

function capabilityCounts(
  snapshot: ExternalSourceCatalogSnapshot | null,
  sources: ExternalSourceCatalogSnapshot['sources'],
  acpClients: AcpClientInfo[],
): CompatibilityCapabilityCounts {
  if (!snapshot) {
    return {
      command: 0,
      tool: 0,
      subagent: 0,
      mcp: 0,
      runtime: acpClients.filter((client) => client.enabled).length,
    };
  }

  const sourcePairs = new Set(sources.map((source) => sourcePairKey(
    source.record.key.providerId,
    source.record.key.sourceId,
  )));
  const belongsToProduct = (source: { providerId: string; sourceId: string }): boolean => (
    sourcePairs.has(sourcePairKey(source.providerId, source.sourceId))
  );

  return {
    command: snapshot.commands.filter((command) => belongsToProduct(command.definition.id.source)).length,
    tool: (snapshot.tools ?? []).filter((tool) => (
      belongsToProduct(tool.definition.id.target.source)
    )).length,
    subagent: (snapshot.subagents ?? []).filter((agent) => (
      agent.sourceKeys.some(belongsToProduct)
    )).length,
    mcp: (snapshot.mcpServers ?? []).filter((server) => (
      belongsToProduct(server.definition.id.source)
    )).length,
    runtime: acpClients.filter((client) => client.enabled).length,
  };
}

function runtimeStatus(
  spec: EcosystemProductSpec,
  sources: ExternalSourceCatalogSnapshot['sources'],
  acpClients: AcpClientInfo[],
): EcosystemProductStatus {
  if (spec.development) return 'development';
  if (acpClients.some((client) => client.status === 'running')) return 'connected';
  if (sources.length > 0) return 'detected';
  if (acpClients.some((client) => client.enabled)) return 'configured';
  return 'available';
}

function runtimeGroup(status: EcosystemProductStatus): EcosystemProductGroup {
  if (status === 'connected' || status === 'detected' || status === 'configured') {
    return 'connected';
  }
  if (status === 'development') return 'other';
  return 'available';
}

function knownCapabilityId(value: string): value is Exclude<CompatibilityCapabilityId, 'runtime'> {
  return value === 'command'
    || value === 'tool'
    || value === 'subagent'
    || value === 'mcp';
}

export function buildEcosystemProductRuntimes(
  snapshot: ExternalSourceCatalogSnapshot | null,
  clients: readonly AcpClientInfo[],
): EcosystemProductRuntime[] {
  return ECOSYSTEM_PRODUCT_SPECS.map((spec) => {
    const sources = productSources(snapshot, spec.ecosystemId);
    const descriptor = spec.ecosystemId
      ? snapshot?.integrationPolicy.registeredEcosystems.find(
          (candidate) => candidate.ecosystemId === spec.ecosystemId,
        )
      : undefined;
    const acpClients = spec.acpClientId
      ? clients.filter((client) => client.id === spec.acpClientId)
      : [];
    const acpClient = spec.acpClientId
      ? acpClients.find((client) => client.id === spec.acpClientId)
      : undefined;
    const capabilityIds: CompatibilityCapabilityId[] = descriptor?.capabilities
      .map((capability) => capability.capabilityId)
      .filter(knownCapabilityId) ?? [];
    if (spec.acpClientId && !capabilityIds.includes('runtime')) {
      capabilityIds.push('runtime');
    }
    const status = runtimeStatus(spec, sources, acpClients);

    return {
      spec,
      status,
      group: runtimeGroup(status),
      sources,
      acpClients,
      acpClient,
      capabilityIds,
      capabilityCounts: capabilityCounts(snapshot, sources, acpClients),
      adapterRevision: descriptor?.adapterRevision,
      sourceLocation: sources[0]?.record.location ?? acpClients[0]?.command,
      executionDomainId: sources[0]?.record.executionDomainId,
    };
  });
}

export function buildEcosystemImportItems(
  snapshot: ExternalSourceCatalogSnapshot | null,
  runtime: EcosystemProductRuntime,
): EcosystemImportItem[] {
  const belongsToProduct = (source: { providerId: string; sourceId: string }): boolean => (
    itemSource(runtime.sources, source) !== undefined
  );
  const sourceFacts = (source: { providerId: string; sourceId: string }) => {
    const match = itemSource(runtime.sources, source);
    return {
      sourceName: match?.record.displayName ?? runtime.spec.name,
      sourceLocation: match?.record.location,
    };
  };
  const items: EcosystemImportItem[] = [];

  for (const command of snapshot?.commands ?? []) {
    const source = command.definition.id.source;
    if (!belongsToProduct(source)) continue;
    items.push({
      id: `command:${command.candidateId ?? `${source.providerId}/${source.sourceId}:${command.definition.id.localId}`}`,
      kind: 'command',
      name: command.definition.name,
      description: command.definition.description,
      ...sourceFacts(source),
      support: 'adapted',
      detection: 'catalog',
      nativeImportSupported: false,
      discovered: true,
    });
  }

  for (const tool of snapshot?.tools ?? []) {
    const source = tool.definition.id.target.source;
    if (!belongsToProduct(source)) continue;
    items.push({
      id: `tool:${source.providerId}/${source.sourceId}:${tool.definition.id.exportId}`,
      kind: 'tool',
      name: tool.definition.name,
      description: tool.definition.descriptionPreview,
      ...sourceFacts(source),
      support: 'adapted',
      detection: 'catalog',
      nativeImportSupported: false,
      discovered: true,
    });
  }

  for (const agent of snapshot?.subagents ?? []) {
    const source = agent.sourceKeys.find(belongsToProduct);
    if (!source) continue;
    items.push({
      id: `subagent:${agent.candidateId}`,
      kind: 'subagent',
      name: agent.displayName,
      description: agent.description,
      ...sourceFacts(source),
      support: 'adapted',
      detection: 'catalog',
      nativeImportSupported: false,
      discovered: true,
    });
  }

  for (const server of snapshot?.mcpServers ?? []) {
    const source = server.definition.id.source;
    if (!belongsToProduct(source)) continue;
    items.push({
      id: `mcp:${server.candidateId}`,
      kind: 'mcp',
      name: server.definition.name,
      ...sourceFacts(source),
      candidateId: server.candidateId,
      support: 'adapted',
      detection: 'catalog',
      nativeImportSupported: true,
      discovered: true,
    });
  }

  for (const kind of ECOSYSTEM_IMPORT_ITEM_KINDS) {
    if (items.some((item) => item.kind === kind)) continue;
    const support = ecosystemCompatibilitySupport(runtime.spec.id, kind);
    if (support === 'notApplicable') continue;
    items.push({
      id: `undetected:${kind}`,
      kind,
      name: kind,
      sourceName: runtime.spec.name,
      support,
      detection: support === 'adapted' && OWNER_DETECTED_KINDS.has(kind)
        ? 'owner'
        : 'catalog',
      nativeImportSupported: support === 'adapted' && kind === 'mcp',
      discovered: false,
    });
  }

  const kindOrder: Record<EcosystemImportItemKind, number> = {
    account: 0,
    settings: 1,
    command: 2,
    tool: 3,
    subagent: 4,
    skill: 5,
    mcp: 6,
    hook: 7,
    memory: 8,
    plugin: 9,
    pet: 10,
  };
  return items.sort((left, right) => (
    kindOrder[left.kind] - kindOrder[right.kind]
    || left.name.localeCompare(right.name)
  ));
}

export function totalDiscoveredAssets(counts: CompatibilityCapabilityCounts): number {
  return counts.command + counts.tool + counts.subagent + counts.mcp;
}

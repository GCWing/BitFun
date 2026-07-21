import { api } from './ApiClient';

export type ExternalSourceScope =
  | 'user_global'
  | 'project'
  | 'workspace_local'
  | 'remote_user'
  | 'remote_project';

export type ExternalSourceLifecycle =
  | 'available'
  | 'restricted'
  | 'degraded'
  | 'unavailable'
  | 'removed'
  | 'suppressed'
  | 'using_last_valid_version';

export type ExternalIntegrationMode =
  | 'recommended'
  | 'discover_only'
  | 'disabled'
  | 'custom'
  | (string & {});

export type ExternalIntegrationAccess =
  | 'disabled'
  | 'discover_only'
  | 'ask_before_use'
  | 'auto'
  | (string & {});

export interface ExternalEcosystemPolicy {
  mode: ExternalIntegrationMode;
  capabilityOverrides?: Record<string, ExternalIntegrationAccess>;
}

export interface ExternalEcosystemPolicyOverride {
  mode?: ExternalIntegrationMode;
  capabilityOverrides?: Record<string, ExternalIntegrationAccess>;
}

export interface ExternalIntegrationPolicySnapshot {
  schemaMajor: number;
  status: 'compatible' | 'incompatible_schema' | (string & {});
  userDefaults: {
    enabled: boolean;
    ecosystems?: Record<string, ExternalEcosystemPolicy>;
  };
  workspaceOverride?: {
    enabled?: boolean;
    ecosystems?: Record<string, ExternalEcosystemPolicyOverride>;
  };
  globalEffective: EffectiveExternalIntegrationPolicy;
  effective: EffectiveExternalIntegrationPolicy;
  registeredEcosystems: Array<{
    ecosystemId: string;
    displayName: string;
    adapterRevision: string;
    capabilities: Array<{
      capabilityId: string;
      recommendedAccess: ExternalIntegrationAccess;
      safetyCeiling: ExternalIntegrationAccess;
    }>;
  }>;
}

export interface EffectiveExternalIntegrationPolicy {
    enabled: boolean;
    ecosystems: Record<
      string,
      {
        ecosystemId: string;
        mode: ExternalIntegrationMode;
        capabilities: Record<string, ExternalIntegrationAccess>;
        policyLimitedCapabilities?: string[];
      }
    >;
}

export type ExternalIntegrationPolicyMutation = {
  expectedPreferenceRevision: number;
  scope: 'user' | 'workspace';
  change:
    | { operation: 'set_enabled'; enabled: boolean }
    | {
        operation: 'set_ecosystem_mode';
        ecosystemId: string;
        mode: ExternalIntegrationMode;
      }
    | {
        operation: 'set_capability_access';
        ecosystemId: string;
        capabilityId: string;
        access: ExternalIntegrationAccess;
      }
    | { operation: 'reset_workspace' }
    | { operation: 'reset_incompatible_policy' };
};

export type PromptCommandAvailability =
  | { state: 'available' }
  | { state: 'restricted'; reason: string; required_capabilities: string[] }
  | { state: 'invalid'; reason: string };

export interface ExternalSourceRecord {
  key: { providerId: string; sourceId: string };
  ecosystemId: string;
  displayName: string;
  sourceKind: string;
  scope: ExternalSourceScope;
  location: string;
  executionDomainId: string;
  health: 'available' | 'partial' | 'degraded' | 'unavailable';
  contentVersion: string;
  diagnostics?: Array<{
    severity: string;
    assetKind?: 'source' | 'command' | 'tool' | 'subagent' | 'mcp';
    code: string;
    message: string;
  }>;
}

export interface ExternalSourceCatalogSnapshot {
  hostCapabilities: {
    canRefresh: boolean;
    canMutatePolicy: boolean;
    canManageSources: boolean;
    canApproveRuntime: boolean;
    canExecuteExternalAssets: boolean;
  };
  generation: number;
  discoveryPending: boolean;
  sources: Array<{
    stableKey: string;
    presentationGroupId?: string;
    record: ExternalSourceRecord;
    lifecycle: ExternalSourceLifecycle;
  }>;
  commands: Array<{
    definition: {
      id: {
        source: { providerId: string; sourceId: string };
        localId: string;
      };
      name: string;
      description: string;
      availability: PromptCommandAvailability;
      contentVersion: string;
    };
  }>;
  commandConflicts?: Array<{
    conflictKey: string;
    commandName: string;
    selectedCandidateId?: string;
    candidates: Array<{
      candidateId: string;
      source: { providerId: string; sourceId: string };
      sourceDisplayName: string;
      ecosystemId: string;
      contentVersion: string;
      commandDescription: string;
      sourceScope: ExternalSourceScope;
      sourceLocation: string;
      availability: PromptCommandAvailability;
    }>;
  }>;
  tools?: ExternalToolCatalogEntry[];
  toolApprovalRequests?: ExternalToolApprovalRequest[];
  toolConflicts?: ExternalToolConflict[];
  mcpGeneration?: number;
  mcpServers?: ExternalMcpCatalogEntry[];
  mcpApprovalRequests?: ExternalMcpApprovalRequest[];
  mcpConflicts?: ExternalMcpConflict[];
  subagentGeneration?: number;
  preferenceRevision?: number;
  subagents?: ExternalSubagentSummary[];
  subagentConflicts?: ExternalSubagentConflict[];
  pendingSubagentApprovals?: string[];
  integrationPolicy: ExternalIntegrationPolicySnapshot;
  diagnostics?: Array<{
    severity: string;
    assetKind?: 'source' | 'command' | 'tool' | 'subagent' | 'mcp';
    code: string;
    message: string;
  }>;
}

export type ExternalSubagentActivation =
  | { state: 'approval_required' }
  | { state: 'declined' }
  | { state: 'disabled' }
  | { state: 'active' }
  | { state: 'conflict' }
  | { state: 'blocked' }
  | { state: 'unavailable' };

export interface ExternalSubagentSummary {
  candidateId: string;
  logicalId: string;
  displayName: string;
  description: string;
  providerLabel: string;
  scope: ExternalSourceScope;
  sourceKeys: Array<{ providerId: string; sourceId: string }>;
  sourceLocationLabels: string[];
  sourceCount: number;
  effectiveModelLabel?: string;
  effectiveToolLabels: string[];
  supportsFollowUp: boolean;
  compatibilityState: 'ready' | 'ready_with_degradation' | 'blocked' | 'invalid';
  diagnostics: Array<{ code: string; blocksActivation: boolean }>;
  activationState: ExternalSubagentActivation;
  decisionKey: string;
}

export interface ExternalSubagentConflict {
  conflictKey: string;
  logicalId: string;
  selectedCandidateId?: string;
  candidates: Array<{
    candidateId: string;
    displayName: string;
    sourceLabel: string;
    external: boolean;
  }>;
}

export type ExternalToolCapability = 'file_system' | 'network' | 'process' | 'environment';
export type ExternalToolActivation =
  | { state: 'approval_required' }
  | { state: 'disabled' }
  | { state: 'active' }
  | { state: 'conflict' }
  | { state: 'unsupported'; reason: string }
  | { state: 'runtime_unavailable'; reason: string }
  | { state: 'load_failed'; reason: string };

export interface ExternalToolDefinition {
  id: {
    target: {
      source: { providerId: string; sourceId: string };
      localId: string;
    };
    exportId: string;
  };
  name: string;
  descriptionPreview: string;
  modulePath: string;
  workingDirectory: string;
  runtimeKind: 'java_script' | 'type_script';
  capabilities: ExternalToolCapability[];
  contentVersion: string;
  staticStatus:
    | { state: 'ready' }
    | { state: 'unsupported'; reason: string }
    | { state: 'invalid'; reason: string };
}

export interface ExternalToolCatalogEntry {
  definition: ExternalToolDefinition;
  approvalKey: string;
  decisionKey: string;
  activation: ExternalToolActivation;
}

export interface ExternalToolApprovalRequest {
  approvalKey: string;
  decisionKey: string;
  targetId: {
    source: { providerId: string; sourceId: string };
    localId: string;
  };
  sourceDisplayName: string;
  sourceScope: ExternalSourceScope;
  sourceLocation: string;
  workingDirectory: string;
  runtimeKind: 'java_script' | 'type_script';
  capabilities: ExternalToolCapability[];
  contentVersion: string;
  toolNames: string[];
}

export interface ExternalToolConflict {
  conflictKey: string;
  toolName: string;
  selectedCandidateId?: string;
  candidates: Array<{
    candidateId: string;
    displayName: string;
    kind: 'built_in' | 'mcp' | 'external';
    providerId: string;
    contentVersion: string;
    source?: { providerId: string; sourceId: string };
    sourceLocation?: string;
  }>;
}

export type ExternalMcpActivation =
  | { state: 'approval_required' }
  | { state: 'starting' }
  | { state: 'active' }
  | { state: 'declined' }
  | { state: 'conflict' }
  | { state: 'covered'; selected_candidate_id: string }
  | { state: 'source_disabled' }
  | { state: 'configuration_changed' }
  | { state: 'unsupported'; reason: string }
  | { state: 'runtime_unavailable'; reason: string }
  | { state: 'removed' };

export interface ExternalMcpDefinition {
  id: {
    source: { providerId: string; sourceId: string };
    localId: string;
  };
  provenance: Array<{ providerId: string; sourceId: string }>;
  name: string;
  transport: 'local_stdio' | 'streamable_http';
  commandPreview?: string;
  argumentCount: number;
  workingDirectory?: string;
  environmentKeys: string[];
  environmentReferenceNames?: string[];
  remoteUrlPreview?: string;
  headerNames: string[];
  sourceEnabled: boolean;
  behaviorVersion: string;
  staticStatus:
    | { state: 'ready' }
    | { state: 'disabled_by_source' }
    | { state: 'unsupported'; reason: string }
    | { state: 'invalid'; reason: string };
}

export interface ExternalMcpCatalogEntry {
  candidateId: string;
  definition: ExternalMcpDefinition;
  approvalKey: string;
  decisionKey: string;
  runtimeId?: string;
  activationState: ExternalMcpActivation;
}

export interface ExternalMcpApprovalRequest {
  candidateId: string;
  approvalKey: string;
  decisionKey: string;
  definition: ExternalMcpDefinition;
}

export interface ExternalMcpConflict {
  conflictKey: string;
  serverName: string;
  selectedCandidateId?: string;
  candidates: Array<{
    candidateId: string;
    displayName: string;
    external: boolean;
    source?: { providerId: string; sourceId: string };
    behaviorVersion: string;
    available: boolean;
    unavailableReason?: string;
  }>;
}

export type ExternalSourceOperationErrorCode =
  | 'invalid_request'
  | 'host_unavailable'
  | 'host_capability_unavailable'
  | 'policy_incompatible'
  | 'policy_limited'
  | 'stale_revision'
  | 'conflict'
  | 'not_found'
  | 'unavailable'
  | 'internal';

export class ExternalSourceApiError extends Error {
  constructor(
    public readonly code: ExternalSourceOperationErrorCode,
    public readonly detail: string,
    public readonly retryable: boolean,
    public readonly correlationId?: string,
  ) {
    super(detail);
    this.name = 'ExternalSourceApiError';
  }
}

const READ_ONLY_HOST_CAPABILITIES: ExternalSourceCatalogSnapshot['hostCapabilities'] = {
  canRefresh: false,
  canMutatePolicy: false,
  canManageSources: false,
  canApproveRuntime: false,
  canExecuteExternalAssets: false,
};

function safePolicySnapshot(
  status: ExternalIntegrationPolicySnapshot['status'] = 'unknown',
  schemaMajor = 0,
): ExternalIntegrationPolicySnapshot {
  const safelyOff: EffectiveExternalIntegrationPolicy = {
    enabled: false,
    ecosystems: {},
  };
  return {
    schemaMajor,
    status,
    userDefaults: { enabled: false, ecosystems: {} },
    globalEffective: safelyOff,
    effective: safelyOff,
    registeredEcosystems: [],
  };
}

function normalizeOptionalArray<T>(value: unknown): T[] {
  if (value === undefined || value === null) return [];
  if (Array.isArray(value)) return value;
  throw new ExternalSourceApiError(
    'internal',
    'External source response included an invalid collection',
    true,
  );
}

function normalizePolicySnapshot(value: unknown): ExternalIntegrationPolicySnapshot {
  if (!value || typeof value !== 'object') return safePolicySnapshot();
  const candidate = value as Partial<ExternalIntegrationPolicySnapshot>;
  const schemaMajor = typeof candidate.schemaMajor === 'number' ? candidate.schemaMajor : 0;
  if (candidate.status === 'incompatible_schema') {
    return safePolicySnapshot('incompatible_schema', schemaMajor);
  }
  if (
    candidate.status !== 'compatible'
    || !candidate.userDefaults
    || typeof candidate.userDefaults.enabled !== 'boolean'
    || !candidate.globalEffective
    || typeof candidate.globalEffective.enabled !== 'boolean'
    || !candidate.globalEffective.ecosystems
    || !candidate.effective
    || typeof candidate.effective.enabled !== 'boolean'
    || !candidate.effective.ecosystems
    || !Array.isArray(candidate.registeredEcosystems)
  ) {
    return safePolicySnapshot(
      typeof candidate.status === 'string' ? candidate.status : 'unknown',
      schemaMajor,
    );
  }
  return {
    ...candidate,
    registeredEcosystems: candidate.registeredEcosystems.map((ecosystem) => ({
      ...ecosystem,
      capabilities: normalizeOptionalArray(ecosystem.capabilities),
    })),
  } as ExternalIntegrationPolicySnapshot;
}

function normalizeMcpDefinition(definition: ExternalMcpDefinition): ExternalMcpDefinition {
  return {
    ...definition,
    provenance: normalizeOptionalArray(definition.provenance),
    environmentKeys: normalizeOptionalArray(definition.environmentKeys),
    environmentReferenceNames: normalizeOptionalArray(definition.environmentReferenceNames),
    headerNames: normalizeOptionalArray(definition.headerNames),
  };
}

function normalizeSnapshot(value: unknown): ExternalSourceCatalogSnapshot {
  if (!value || typeof value !== 'object') {
    throw new ExternalSourceApiError('internal', 'External source response was not usable', true);
  }
  const candidate = value as ExternalSourceCatalogSnapshot & {
    hostCapabilities?: Partial<ExternalSourceCatalogSnapshot['hostCapabilities']>;
    integrationPolicy?: unknown;
  };
  const capabilities = candidate.hostCapabilities;
  return {
    ...candidate,
    generation: typeof candidate.generation === 'number' ? candidate.generation : 0,
    discoveryPending: candidate.discoveryPending === true,
    sources: normalizeOptionalArray<ExternalSourceCatalogSnapshot['sources'][number]>(candidate.sources).map((source) => ({
      ...source,
      record: {
        ...source.record,
        diagnostics: normalizeOptionalArray(source.record.diagnostics),
      },
    })),
    commands: normalizeOptionalArray<ExternalSourceCatalogSnapshot['commands'][number]>(candidate.commands),
    commandConflicts: normalizeOptionalArray<NonNullable<ExternalSourceCatalogSnapshot['commandConflicts']>[number]>(candidate.commandConflicts).map((conflict) => ({
      ...conflict,
      candidates: normalizeOptionalArray(conflict.candidates),
    })),
    tools: normalizeOptionalArray<ExternalToolCatalogEntry>(candidate.tools).map((entry) => ({
      ...entry,
      definition: {
        ...entry.definition,
        capabilities: normalizeOptionalArray(entry.definition.capabilities),
      },
    })),
    toolApprovalRequests: normalizeOptionalArray<ExternalToolApprovalRequest>(candidate.toolApprovalRequests).map((request) => ({
      ...request,
      capabilities: normalizeOptionalArray(request.capabilities),
      toolNames: normalizeOptionalArray(request.toolNames),
    })),
    toolConflicts: normalizeOptionalArray<ExternalToolConflict>(candidate.toolConflicts).map((conflict) => ({
      ...conflict,
      candidates: normalizeOptionalArray(conflict.candidates),
    })),
    mcpServers: normalizeOptionalArray<ExternalMcpCatalogEntry>(candidate.mcpServers).map((entry) => ({
      ...entry,
      definition: normalizeMcpDefinition(entry.definition),
    })),
    mcpApprovalRequests: normalizeOptionalArray<ExternalMcpApprovalRequest>(candidate.mcpApprovalRequests).map((request) => ({
      ...request,
      definition: normalizeMcpDefinition(request.definition),
    })),
    mcpConflicts: normalizeOptionalArray<ExternalMcpConflict>(candidate.mcpConflicts).map((conflict) => ({
      ...conflict,
      candidates: normalizeOptionalArray(conflict.candidates),
    })),
    subagents: normalizeOptionalArray<ExternalSubagentSummary>(candidate.subagents).map((subagent) => ({
      ...subagent,
      sourceKeys: normalizeOptionalArray(subagent.sourceKeys),
      sourceLocationLabels: normalizeOptionalArray(subagent.sourceLocationLabels),
      effectiveToolLabels: normalizeOptionalArray(subagent.effectiveToolLabels),
      diagnostics: normalizeOptionalArray(subagent.diagnostics),
    })),
    subagentConflicts: normalizeOptionalArray<ExternalSubagentConflict>(candidate.subagentConflicts).map((conflict) => ({
      ...conflict,
      candidates: normalizeOptionalArray(conflict.candidates),
    })),
    pendingSubagentApprovals: normalizeOptionalArray(candidate.pendingSubagentApprovals),
    diagnostics: normalizeOptionalArray(candidate.diagnostics),
    hostCapabilities: {
      ...READ_ONLY_HOST_CAPABILITIES,
      canRefresh: capabilities?.canRefresh === true,
      canMutatePolicy: capabilities?.canMutatePolicy === true,
      canManageSources: capabilities?.canManageSources === true,
      canApproveRuntime: capabilities?.canApproveRuntime === true,
      canExecuteExternalAssets: capabilities?.canExecuteExternalAssets === true,
    },
    integrationPolicy: normalizePolicySnapshot(candidate.integrationPolicy),
  };
}

const OPERATION_ERROR_CODES = new Set<ExternalSourceOperationErrorCode>([
  'invalid_request',
  'host_unavailable',
  'host_capability_unavailable',
  'policy_incompatible',
  'policy_limited',
  'stale_revision',
  'conflict',
  'not_found',
  'unavailable',
  'internal',
]);

function parseOperationError(value: unknown, visited = new Set<unknown>()): ExternalSourceApiError | null {
  if (value === null || value === undefined || visited.has(value)) return null;
  visited.add(value);
  if (typeof value === 'string') {
    try {
      return parseOperationError(JSON.parse(value), visited);
    } catch {
      return null;
    }
  }
  if (typeof value !== 'object') return null;
  const record = value as Record<string, unknown>;
  if (
    typeof record.code === 'string' &&
    OPERATION_ERROR_CODES.has(record.code as ExternalSourceOperationErrorCode) &&
    typeof record.detail === 'string'
  ) {
    return new ExternalSourceApiError(
      record.code as ExternalSourceOperationErrorCode,
      record.detail,
      record.retryable === true,
      typeof record.correlationId === 'string' ? record.correlationId : undefined,
    );
  }
  for (const candidate of [
    record.originalError,
    record.error,
    record.data,
    record.details,
    (record.context as Record<string, unknown> | undefined)?.originalError,
    (record.details as Record<string, unknown> | undefined)?.originalError,
  ]) {
    const parsed = parseOperationError(candidate, visited);
    if (parsed) return parsed;
  }
  return null;
}

async function invokeExternal<T>(command: string, args: Record<string, unknown>): Promise<T> {
  try {
    return await api.invoke<T>(command, args);
  } catch (error) {
    throw parseOperationError(error) ?? new ExternalSourceApiError(
      'internal',
      'External source operation failed',
      false,
    );
  }
}

async function invokeSnapshot(
  command: string,
  args: Record<string, unknown>,
): Promise<ExternalSourceCatalogSnapshot> {
  return normalizeSnapshot(await invokeExternal<unknown>(command, args));
}

function normalizeOptionalWorkspacePath(workspacePath: string | undefined): string | undefined {
  return workspacePath?.trim() ? workspacePath : undefined;
}

export const externalSourcesAPI = {
  getSnapshot(workspacePath?: string, forceRefresh = false) {
    return invokeSnapshot('get_external_source_snapshot', {
      request: { workspacePath: normalizeOptionalWorkspacePath(workspacePath), forceRefresh },
    });
  },

  revealSourceLocation(workspacePath: string | undefined, sourceKey: string) {
    return invokeExternal<void>('reveal_external_source_location', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        sourceKey,
      },
    });
  },

  setSourceEnabled(
    workspacePath: string | undefined,
    sourceKey: string,
    enabled: boolean,
    expectedPreferenceRevision: number,
  ) {
    return invokeSnapshot('set_external_source_enabled_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        sourceKey,
        enabled,
        expectedPreferenceRevision,
      },
    });
  },

  setConflictChoice(
    workspacePath: string | undefined,
    conflictKey: string,
    candidateId: string,
    expectedPreferenceRevision: number,
  ) {
    return invokeSnapshot('set_external_source_conflict_choice_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        conflictKey,
        candidateId,
        expectedPreferenceRevision,
      },
    });
  },

  setToolTargetDecision(
    workspacePath: string | undefined,
    approvalKey: string,
    decisionKey: string,
    approved: boolean,
    expectedPreferenceRevision: number,
  ) {
    return invokeSnapshot('set_external_tool_target_decision_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        approvalKey,
        decisionKey,
        approved,
        expectedPreferenceRevision,
      },
    });
  },

  setToolConflictChoice(
    workspacePath: string | undefined,
    conflictKey: string,
    candidateId: string,
    expectedPreferenceRevision: number,
  ) {
    return invokeSnapshot('set_external_tool_conflict_choice_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        conflictKey,
        candidateId,
        expectedPreferenceRevision,
      },
    });
  },

  setSubagentActivation(
    workspacePath: string | undefined,
    candidateId: string,
    approved: boolean,
    expectedSubagentGeneration: number,
    expectedPreferenceRevision: number,
    decisionKey: string,
  ) {
    return invokeSnapshot('set_external_subagent_activation_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        candidateId,
        approved,
        expectedSubagentGeneration,
        expectedPreferenceRevision,
        decisionKey,
      },
    });
  },

  chooseSubagentConflict(
    workspacePath: string | undefined,
    conflictKey: string,
    candidateId: string,
    approveExternal: boolean,
    expectedSubagentGeneration: number,
    expectedPreferenceRevision: number,
  ) {
    return invokeSnapshot('choose_external_subagent_conflict_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        conflictKey,
        candidateId,
        approveExternal,
        expectedSubagentGeneration,
        expectedPreferenceRevision,
      },
    });
  },

  setMcpServerDecision(
    workspacePath: string | undefined,
    candidateId: string,
    decisionKey: string,
    approved: boolean,
    expectedMcpGeneration: number,
    expectedPreferenceRevision: number,
  ) {
    return invokeSnapshot('set_external_mcp_server_decision_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        candidateId,
        decisionKey,
        approved,
        expectedMcpGeneration,
        expectedPreferenceRevision,
      },
    });
  },

  chooseMcpConflict(
    workspacePath: string | undefined,
    conflictKey: string,
    candidateId: string,
    approveExternal: boolean,
    expectedMcpGeneration: number,
    expectedPreferenceRevision: number,
  ) {
    return invokeSnapshot('choose_external_mcp_conflict_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        conflictKey,
        candidateId,
        approveExternal,
        expectedMcpGeneration,
        expectedPreferenceRevision,
      },
    });
  },

  updateIntegrationPolicy(
    workspacePath: string | undefined,
    mutation: ExternalIntegrationPolicyMutation,
  ) {
    return invokeSnapshot(
      'update_external_integration_policy_command',
      { request: { workspacePath: normalizeOptionalWorkspacePath(workspacePath), mutation } },
    );
  },
};

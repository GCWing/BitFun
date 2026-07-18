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
  generation: number;
  discoveryPending: boolean;
  sources: Array<{
    stableKey: string;
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

export const externalSourcesAPI = {
  getSnapshot(workspacePath?: string, forceRefresh = false) {
    return api.invoke<ExternalSourceCatalogSnapshot>('get_external_source_snapshot', {
      request: { workspacePath, forceRefresh },
    });
  },

  setSourceEnabled(workspacePath: string | undefined, sourceKey: string, enabled: boolean) {
    return api.invoke<ExternalSourceCatalogSnapshot>('set_external_source_enabled_command', {
      request: { workspacePath, sourceKey, enabled },
    });
  },

  setConflictChoice(workspacePath: string | undefined, conflictKey: string, candidateId: string) {
    return api.invoke<ExternalSourceCatalogSnapshot>('set_external_source_conflict_choice_command', {
      request: { workspacePath, conflictKey, candidateId },
    });
  },

  setToolTargetDecision(
    workspacePath: string | undefined,
    approvalKey: string,
    decisionKey: string,
    approved: boolean,
  ) {
    return api.invoke<ExternalSourceCatalogSnapshot>('set_external_tool_target_decision_command', {
      request: { workspacePath, approvalKey, decisionKey, approved },
    });
  },

  setToolConflictChoice(
    workspacePath: string | undefined,
    conflictKey: string,
    candidateId: string,
  ) {
    return api.invoke<ExternalSourceCatalogSnapshot>('set_external_tool_conflict_choice_command', {
      request: { workspacePath, conflictKey, candidateId },
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
    return api.invoke<ExternalSourceCatalogSnapshot>('set_external_subagent_activation_command', {
      request: {
        workspacePath,
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
    return api.invoke<ExternalSourceCatalogSnapshot>('choose_external_subagent_conflict_command', {
      request: {
        workspacePath,
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
    return api.invoke<ExternalSourceCatalogSnapshot>('set_external_mcp_server_decision_command', {
      request: {
        workspacePath,
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
    return api.invoke<ExternalSourceCatalogSnapshot>('choose_external_mcp_conflict_command', {
      request: {
        workspacePath,
        conflictKey,
        candidateId,
        approveExternal,
        expectedMcpGeneration,
        expectedPreferenceRevision,
      },
    });
  },
};

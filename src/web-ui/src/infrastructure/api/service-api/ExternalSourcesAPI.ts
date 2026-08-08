import { api } from './ApiClient';
import { globalEventBus } from '@/infrastructure/event-bus';

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
    assetKind?: 'source' | 'command' | 'tool' | 'subagent' | 'mcp' | 'reference';
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
    canSetSafeMode: boolean;
    canRevealSourceLocation: boolean;
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
    /** Opaque host identity used for guarded expansion; absent on legacy hosts. */
    candidateId?: string;
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
      executionTarget?: PromptCommandExecutionTarget;
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
  subagentModelBindingGroups?: ExternalSubagentModelBindingGroup[];
  subagentModelBindingOptions?: ExternalSubagentModelBindingOption[];
  subagentConflicts?: ExternalSubagentConflict[];
  pendingSubagentApprovals?: string[];
  integrationPolicy: ExternalIntegrationPolicySnapshot;
  diagnostics?: Array<{
    severity: string;
    assetKind?: 'source' | 'command' | 'tool' | 'subagent' | 'mcp' | 'reference';
    code: string;
    message: string;
  }>;
  /** Frontend view of the atomic control+catalog response. Legacy hosts omit it. */
  control?: ExternalSourceControlSnapshot;
}

export type ExternalApplicationTargetScopeV2 = 'user_default' | 'workspace_override';
export type ExternalApplicationDesiredConnectionV2 =
  | 'unspecified'
  | 'connected'
  | 'disconnected'
  | 'deferred'
  | 'needs_review';
export type ExternalApplicationUserDecisionV2 =
  | 'none'
  | 'connected'
  | 'disconnected'
  | 'deferred'
  | 'needs_review';
export type ExternalApplicationDiscoveryStateV2 = 'not_discovered' | 'discovered';
export type ExternalApplicationConnectionStateV2 = 'disconnected' | 'connected';
export type ExternalApplicationHealthV2 = 'healthy' | 'degraded' | 'unavailable';
export type ExternalApplicationEffectiveStatusV2 =
  | 'connected'
  | 'configuration_available'
  | 'no_configuration'
  | 'needs_attention'
  | 'temporarily_unavailable';
export type ExternalApplicationPrimaryActionV2 =
  | 'none'
  | 'view'
  | 'connect'
  | 'review'
  | 'retry'
  | 'view_reason';
export type ExternalApplicationDefaultConnectionPolicyV2 =
  | 'connect'
  | 'discover_only'
  | 'unsupported';
export type ExternalApplicationRiskLevelV2 = 'low' | 'moderate' | 'high';
export type ExternalApplicationSafetyCeilingV2 = 'blocked' | 'review_required' | 'automatic';
export type ExternalApplicationRecoveryActionV2 = {
  type:
    | 'refresh'
    | 'retry'
    | 'reconnect_host'
    | 'review'
    | 'upgrade_host'
    | 'view_reason'
    | 'exit_safe_mode'
    | 'resolve_conflict'
    | 'install_runtime';
};

export interface ExternalApplicationHostCapabilitiesV2 {
  canReadSnapshot: boolean;
  canReadReview: boolean;
  canMutate: boolean;
  canManageUserDefault: boolean;
  canManageWorkspaceOverride: boolean;
  canRefresh: boolean;
  canSetSafeMode: boolean;
}

export interface ExternalApplicationRiskSummaryV2 {
  highestLevel?: ExternalApplicationRiskLevelV2;
  reasonCodes: string[];
}

export type ExternalApplicationReviewItemKindV2 =
  | 'command'
  | 'tool'
  | 'subagent'
  | 'mcp'
  | 'conflict';

export interface ExternalApplicationReviewSummaryV2 {
  reviewId: string;
  totalCount: number;
  categoryCounts: Array<{ kind: ExternalApplicationReviewItemKindV2; count: number }>;
  maxSelectionCount: number;
  riskSummary: ExternalApplicationRiskSummaryV2;
  recommendationSummary: {
    recommendedCount: number;
    optionalCount: number;
    blockedCount: number;
  };
  safetyCeiling: ExternalApplicationSafetyCeilingV2;
}

export interface ExternalApplicationReviewItemRefV2 {
  kind: ExternalApplicationReviewItemKindV2;
  stableId: string;
}

export interface ExternalApplicationOwnerGenerationV2 {
  owner: ExternalApplicationReviewItemKindV2;
  generation: number;
}

export interface ExternalApplicationReviewItemV2 {
  itemRef: ExternalApplicationReviewItemRefV2;
  displayName: string;
  displaySummary: string;
  riskLevel: ExternalApplicationRiskLevelV2;
  riskReasonCodes: string[];
  recommended: boolean;
  safetyCeiling: ExternalApplicationSafetyCeilingV2;
}

export interface ExternalApplicationReviewPageRequestV2 {
  schemaVersion: 2;
  executionDomainId: string;
  workspaceScopeId?: string;
  targetScope: ExternalApplicationTargetScopeV2;
  reviewId: string;
  preferenceRevision: number;
  expectedGenerations: ExternalApplicationOwnerGenerationV2[];
  cursor?: string;
  pageSize: number;
}

export interface ExternalApplicationReviewPageV2 {
  schemaVersion: 2;
  executionDomainId: string;
  workspaceScopeId?: string;
  targetScope: ExternalApplicationTargetScopeV2;
  reviewId: string;
  preferenceRevision: number;
  expectedGenerations: ExternalApplicationOwnerGenerationV2[];
  cursor?: string;
  nextCursor?: string;
  totalCount: number;
  items: ExternalApplicationReviewItemV2[];
}

export type ExternalApplicationReviewSelectionBaselineV2 = 'recommended' | 'none';

export interface ExternalApplicationReviewSelectionOverrideV2 {
  itemRef: ExternalApplicationReviewItemRefV2;
  selected: boolean;
}

export type ExternalApplicationControlActionV2 =
  | { type: 'connect_application'; applicationId: string }
  | { type: 'disconnect_application'; applicationId: string }
  | { type: 'set_application_deferred'; applicationId: string }
  | {
      type: 'submit_application_review';
      reviewId: string;
      expectedGenerations: ExternalApplicationOwnerGenerationV2[];
      selectionBaseline: ExternalApplicationReviewSelectionBaselineV2;
      selectionOverrides: ExternalApplicationReviewSelectionOverrideV2[];
    }
  | { type: 'refresh' }
  | { type: 'set_source_enabled'; sourceKey: string; enabled: boolean }
  | { type: 'set_safe_mode'; enabled: boolean };

export interface ExternalApplicationControlRequestV2 {
  schemaVersion: 2;
  executionDomainId: string;
  workspaceScopeId?: string;
  targetScope: ExternalApplicationTargetScopeV2;
  operationId: string;
  expectedPreferenceRevision: number;
  action: ExternalApplicationControlActionV2;
}

export type ExternalApplicationOperationOutcomeV2 =
  | 'applied'
  | 'rejected'
  | 'blocked'
  | 'stale'
  | 'failed';

export interface ExternalApplicationReviewItemResultV2 {
  itemRef: ExternalApplicationReviewItemRefV2;
  outcome: ExternalApplicationOperationOutcomeV2;
  reasonCode?: string;
  recoveryActions: ExternalApplicationRecoveryActionV2[];
}

export interface ExternalApplicationControlResultV2 {
  schemaVersion: 2;
  operationId: string;
  preferenceRevision: number;
  outcome: ExternalApplicationOperationOutcomeV2;
  itemResults: ExternalApplicationReviewItemResultV2[];
}

export interface ExternalApplicationSummaryV2 {
  applicationId: string;
  ecosystemId: string;
  displayName: string;
  discovery: ExternalApplicationDiscoveryStateV2;
  connection: ExternalApplicationConnectionStateV2;
  desiredConnection: ExternalApplicationDesiredConnectionV2;
  health: ExternalApplicationHealthV2;
  effectiveStatus: ExternalApplicationEffectiveStatusV2;
  primaryAction: ExternalApplicationPrimaryActionV2;
  defaultConnectionPolicy: ExternalApplicationDefaultConnectionPolicyV2;
  defaultConnectionReason: string;
  enabledCount: number;
  pendingReviewCount: number;
  blockedCount: number;
  conflictCount: number;
  riskSummary: ExternalApplicationRiskSummaryV2;
  noticeKey?: string;
  userDecision: ExternalApplicationUserDecisionV2;
  recoveryActions: ExternalApplicationRecoveryActionV2[];
}

export interface ExternalApplicationSnapshotV2 {
  schemaVersion: 2;
  executionDomainId: string;
  workspaceScopeId?: string;
  effectiveConnectionScope: ExternalApplicationTargetScopeV2;
  refreshGeneration: number;
  preferenceRevision: number;
  safeMode: boolean;
  hostCapabilities: ExternalApplicationHostCapabilitiesV2;
  applications: ExternalApplicationSummaryV2[];
  reviewSummary?: ExternalApplicationReviewSummaryV2;
}

export interface PromptCommandShellReviewPlan {
  schemaVersion: number;
  planFingerprint: string;
  sourceDisplayName: string;
  workingDirectory: string;
  shellDisplayName: string;
  shellExecutable: string;
  commands: string[];
  canRemember: boolean;
  preferenceRevision: number;
}

export interface PromptCommandShellReviewDecision {
  planFingerprint: string;
  mode: 'run_once' | 'remember';
  expectedPreferenceRevision: number;
}

export type PromptCommandExecutionTarget =
  | { kind: 'inline' }
  | {
      kind: 'fresh_external_subagent';
      ecosystemId: string;
      logicalId: string;
    };

export type ExternalPromptCommandInvocationOutcome =
  | {
      state: 'ready';
      content: string;
      executionTarget: PromptCommandExecutionTarget;
    }
  | { state: 'review_required'; review: PromptCommandShellReviewPlan };

export type ExternalSubagentActivation =
  | { state: 'approval_required' }
  | { state: 'declined' }
  | { state: 'disabled' }
  | { state: 'active' }
  | { state: 'conflict' }
  | { state: 'blocked' }
  | { state: 'unavailable' };

export type ExternalSubagentModelRequest =
  | { kind: 'default' }
  | { kind: 'inherit' }
  | { kind: 'reference'; providerHint?: string; modelName: string };

export type ExternalSubagentModelProfileRequest =
  | { kind: 'named_variant'; name: string }
  | { kind: 'reasoning_effort'; value: string };

export type ExternalSubagentModelBindingTarget =
  | { kind: 'primary' }
  | { kind: 'fast' }
  | { kind: 'model'; modelId: string };

export type ExternalSubagentModelBindingMethod =
  | 'default'
  | 'inherit'
  | 'exact'
  | 'explicit'
  | 'binding_required'
  | 'binding_unavailable';

export interface ExternalSubagentModelBindingOption {
  target: ExternalSubagentModelBindingTarget;
  effectiveModelLabel: string;
  configuredReasoningEffort?: string;
}

export interface ExternalSubagentModelBindingGroup {
  bindingKey: string;
  request: ExternalSubagentModelRequest;
  profileRequest?: ExternalSubagentModelProfileRequest;
  scope: ExternalSourceScope;
  method: ExternalSubagentModelBindingMethod;
  selectedTarget?: ExternalSubagentModelBindingTarget;
  effectiveModelLabel?: string;
  affectedCandidateIds: string[];
}

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
  mode: 'primary' | 'subagent' | 'all';
  requestedModel: ExternalSubagentModelRequest;
  requestedModelProfile?: ExternalSubagentModelProfileRequest;
  modelBindingMethod: ExternalSubagentModelBindingMethod;
  modelBindingKey?: string;
  effectiveModelLabel?: string;
  effectiveToolLabels: string[];
  unavailableToolLabels: string[];
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
  | { state: 'declined' }
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

export interface ExternalMcpTimeouts {
  startupMs?: number;
  catalogMs?: number;
  executionMs?: number;
}

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
  timeouts?: ExternalMcpTimeouts;
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

export type ExternalMcpImportDispositionV1 =
  | 'eligible'
  | 'automatic_rename'
  | 'already_imported'
  | 'unavailable';

export interface ExternalMcpImportPlanV1 {
  schemaVersion: 1;
  planFingerprint: string;
  items: Array<{
    candidateId: string;
    displayName: string;
    transport: 'local_stdio' | 'streamable_http';
    proposedNativeId?: string;
    disposition: ExternalMcpImportDispositionV1;
    reasonCode?: string;
  }>;
}

export interface ExternalMcpImportSelectionV1 {
  candidateId: string;
  requestedNativeId?: string;
}

export interface ExternalMcpImportApplyResultV1 {
  schemaVersion: 1;
  outcome:
    | { status: 'applied'; imported: Array<{ candidateId: string; nativeId: string }> }
    | { status: 'stale'; refreshedPlan: ExternalMcpImportPlanV1 };
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

export type ExternalSourceOperationStage =
  | 'validate_request'
  | 'discover'
  | 'reconcile'
  | 'apply_preference'
  | 'activate_runtime'
  | 'project_response'
  | 'execute_remote';

export type ExternalSourceRecoveryActionType =
  | 'refresh'
  | 'retry'
  | 'review'
  | 'resolve_conflict'
  | 'install_runtime'
  | 'reconnect_host'
  | 'exit_safe_mode';

export interface ExternalSourceRecoveryAction {
  type: ExternalSourceRecoveryActionType;
}

export type ExternalSourceControlAction =
  | { type: 'refresh' }
  | { type: 'set_source_enabled'; sourceKey: string; enabled: boolean }
  | { type: 'set_safe_mode'; enabled: boolean };

export interface ExternalSourceControlRequest {
  schemaVersion: 1;
  operationId: string;
  expectedPreferenceRevision?: number;
  action: ExternalSourceControlAction;
}

export type ExternalSourceRuntimeState =
  | 'not_applicable'
  | 'inactive'
  | 'starting'
  | 'active'
  | 'degraded'
  | 'quarantined'
  | 'unsupported';

export interface ExternalSourceControlSnapshot {
  schemaVersion: 1;
  executionDomainId: string;
  refreshGeneration: number;
  preferenceRevision: number;
  safeMode: boolean;
  hostCapabilities: ExternalSourceCatalogSnapshot['hostCapabilities'];
  sources: Array<{
    stableKey: string;
    ecosystemId: string;
    displayName: string;
    scope: ExternalSourceScope;
    contentVersion: string;
    discovery: 'pending' | 'current' | 'last_known_good' | 'failed' | 'removed';
    desired: 'enabled' | 'disabled';
    review:
      | { state: 'not_required' }
      | { state: 'required'; contentVersion: string };
    runtime: ExternalSourceRuntimeState;
    support: 'supported' | 'partial' | 'unsupported' | 'unavailable';
    effectiveStatus:
      | 'discovering'
      | 'disabled'
      | 'review_required'
      | 'conflict'
      | 'active'
      | 'degraded'
      | 'unsupported'
      | 'available'
      | 'removed';
  }>;
  capabilities: Array<{
    kind: 'command' | 'tool' | 'subagent' | 'mcp';
    revision: number;
    itemCount: number;
    pendingReviewCount: number;
    unresolvedConflictCount: number;
    runtime: ExternalSourceRuntimeState;
    support: 'supported' | 'partial' | 'unsupported' | 'unavailable';
  }>;
  diagnostics: NonNullable<ExternalSourceCatalogSnapshot['diagnostics']>;
  recoveryActions: ExternalSourceRecoveryAction[];
}

export interface ExternalSourceSurfaceSnapshot {
  control: ExternalSourceControlSnapshot;
  catalog: ExternalSourceCatalogSnapshot;
}

export type ExternalSourceOperationErrorCode =
  | 'invalid_request'
  | 'host_unavailable'
  | 'host_capability_unavailable'
  | 'trust_required'
  | 'policy_incompatible'
  | 'policy_limited'
  | 'stale_revision'
  | 'conflict'
  | 'not_found'
  | 'unavailable'
  | 'runtime_unavailable'
  | 'unsupported'
  | 'incompatible_version'
  | 'dependency_failed'
  | 'timeout'
  | 'cancelled'
  | 'overloaded'
  | 'process_lost'
  | 'invalid_response'
  | 'temporarily_unavailable'
  | 'internal';

export class ExternalSourceApiError extends Error {
  constructor(
    public readonly code: ExternalSourceOperationErrorCode,
    public readonly detail: string,
    public readonly retryable: boolean,
    public readonly correlationId?: string,
    public readonly causationId?: string,
    public readonly stage?: ExternalSourceOperationStage,
    public readonly recoveryActions: ExternalSourceRecoveryAction[] = [],
  ) {
    super(detail);
    this.name = 'ExternalSourceApiError';
  }
}

function normalizePromptCommandInvocationOutcome(
  value: unknown,
): ExternalPromptCommandInvocationOutcome {
  if (!value || typeof value !== 'object') {
    throw new ExternalSourceApiError(
      'invalid_response',
      'Prompt command expansion response was invalid',
      false,
    );
  }
  const outcome = value as Record<string, unknown>;
  if (outcome.state === 'review_required' && outcome.review && typeof outcome.review === 'object') {
    return value as ExternalPromptCommandInvocationOutcome;
  }
  if (outcome.state !== 'ready' || typeof outcome.content !== 'string') {
    throw new ExternalSourceApiError(
      'invalid_response',
      'Prompt command expansion response was invalid',
      false,
    );
  }
  const target = outcome.executionTarget;
  if (!target || typeof target !== 'object') {
    throw new ExternalSourceApiError(
      'invalid_response',
      'Prompt command expansion execution target was invalid',
      false,
    );
  }
  const targetRecord = target as Record<string, unknown>;
  const validInline = targetRecord.kind === 'inline';
  const validExternalSubagent = targetRecord.kind === 'fresh_external_subagent'
    && typeof targetRecord.ecosystemId === 'string'
    && targetRecord.ecosystemId.trim().length > 0
    && typeof targetRecord.logicalId === 'string'
    && targetRecord.logicalId.trim().length > 0;
  if (!validInline && !validExternalSubagent) {
    throw new ExternalSourceApiError(
      'invalid_response',
      'Prompt command expansion execution target was invalid',
      false,
    );
  }
  return value as ExternalPromptCommandInvocationOutcome;
}

export interface NativePromptCommandDescriptor {
  commandName: string;
  candidateId: string;
  behaviorVersion: string;
}

export interface NativePromptCommandConflictSnapshot {
  preferenceRevision: number;
  conflicts: Array<{
    commandName: string;
    externalCandidateId: string;
    conflictKey: string;
    selectedCandidateId?: string;
  }>;
  reconfirmations?: Array<{
    commandName: string;
    nativeCandidateId: string;
  }>;
}

export interface WorkspaceReferenceEntry {
  stableKey: string;
  alias?: string;
  path: string;
  description?: string;
  hidden: boolean;
  origin: 'native' | 'external';
  ecosystemId?: string;
  sourceDisplayName?: string;
  sourceScope?: ExternalSourceScope;
}

export interface WorkspaceReferenceSnapshot {
  generation: number;
  discoveryPending: boolean;
  references: WorkspaceReferenceEntry[];
  diagnostics?: Array<{
    severity: string;
    assetKind?: 'reference';
    code: string;
    message: string;
  }>;
}

const READ_ONLY_HOST_CAPABILITIES: ExternalSourceCatalogSnapshot['hostCapabilities'] = {
  canRefresh: false,
  canMutatePolicy: false,
  canManageSources: false,
  canApproveRuntime: false,
  canExecuteExternalAssets: false,
  canSetSafeMode: false,
  canRevealSourceLocation: false,
};

const CONTROL_DISCOVERY_STATES = new Set(['pending', 'current', 'last_known_good', 'failed', 'removed']);
const CONTROL_DESIRED_STATES = new Set(['enabled', 'disabled']);
const CONTROL_RUNTIME_STATES = new Set<ExternalSourceRuntimeState>([
  'not_applicable', 'inactive', 'starting', 'active', 'degraded', 'quarantined', 'unsupported',
]);
const CONTROL_SUPPORT_STATES = new Set(['supported', 'partial', 'unsupported', 'unavailable']);
const CONTROL_EFFECTIVE_STATUSES = new Set([
  'discovering', 'disabled', 'review_required', 'conflict', 'active', 'degraded', 'unsupported', 'available', 'removed',
]);
const CONTROL_CAPABILITY_KINDS = new Set(['command', 'tool', 'subagent', 'mcp']);
const EXTERNAL_SOURCE_SCOPES = new Set<ExternalSourceScope>([
  'user_global', 'project', 'workspace_local', 'remote_user', 'remote_project',
]);
const OPERATION_STAGES = new Set<ExternalSourceOperationStage>([
  'validate_request', 'discover', 'reconcile', 'apply_preference', 'activate_runtime', 'project_response', 'execute_remote',
]);
const RECOVERY_ACTION_TYPES = new Set<ExternalSourceRecoveryActionType>([
  'refresh', 'retry', 'review', 'resolve_conflict', 'install_runtime', 'reconnect_host', 'exit_safe_mode',
]);
const HOST_CAPABILITY_KEYS = [
  'canRefresh',
  'canMutatePolicy',
  'canManageSources',
  'canApproveRuntime',
  'canExecuteExternalAssets',
  'canSetSafeMode',
  'canRevealSourceLocation',
] as const;
const REQUIRED_HOST_CAPABILITY_KEYS = HOST_CAPABILITY_KEYS.filter(
  (key) => key !== 'canRevealSourceLocation',
);

function isOneOf<T extends string>(value: unknown, values: ReadonlySet<T>): value is T {
  return typeof value === 'string' && values.has(value as T);
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

const APPLICATION_TARGET_SCOPES = new Set<ExternalApplicationTargetScopeV2>([
  'user_default', 'workspace_override',
]);
const APPLICATION_DESIRED_CONNECTIONS = new Set<ExternalApplicationDesiredConnectionV2>([
  'unspecified', 'connected', 'disconnected', 'deferred', 'needs_review',
]);
const APPLICATION_USER_DECISIONS = new Set<ExternalApplicationUserDecisionV2>([
  'none', 'connected', 'disconnected', 'deferred', 'needs_review',
]);
const APPLICATION_DISCOVERY_STATES = new Set<ExternalApplicationDiscoveryStateV2>([
  'not_discovered', 'discovered',
]);
const APPLICATION_CONNECTION_STATES = new Set<ExternalApplicationConnectionStateV2>([
  'disconnected', 'connected',
]);
const APPLICATION_HEALTH_STATES = new Set<ExternalApplicationHealthV2>([
  'healthy', 'degraded', 'unavailable',
]);
const APPLICATION_EFFECTIVE_STATUSES = new Set<ExternalApplicationEffectiveStatusV2>([
  'connected',
  'configuration_available',
  'no_configuration',
  'needs_attention',
  'temporarily_unavailable',
]);
const APPLICATION_PRIMARY_ACTIONS = new Set<ExternalApplicationPrimaryActionV2>([
  'none', 'view', 'connect', 'review', 'retry', 'view_reason',
]);
const APPLICATION_DEFAULT_POLICIES = new Set<ExternalApplicationDefaultConnectionPolicyV2>([
  'connect', 'discover_only', 'unsupported',
]);
const APPLICATION_RISK_LEVELS = new Set<ExternalApplicationRiskLevelV2>([
  'low', 'moderate', 'high',
]);
const APPLICATION_SAFETY_CEILINGS = new Set<ExternalApplicationSafetyCeilingV2>([
  'blocked', 'review_required', 'automatic',
]);
const APPLICATION_REVIEW_KINDS = new Set<ExternalApplicationReviewItemKindV2>([
  'command', 'tool', 'subagent', 'mcp', 'conflict',
]);
const APPLICATION_RECOVERY_ACTIONS = new Set<ExternalApplicationRecoveryActionV2['type']>([
  'refresh',
  'retry',
  'reconnect_host',
  'review',
  'upgrade_host',
  'view_reason',
  'exit_safe_mode',
  'resolve_conflict',
  'install_runtime',
]);
const APPLICATION_OPERATION_OUTCOMES = new Set<ExternalApplicationOperationOutcomeV2>([
  'applied', 'rejected', 'blocked', 'stale', 'failed',
]);

function isExactRecord(
  value: unknown,
  allowedKeys: readonly string[],
  requiredKeys: readonly string[] = allowedKeys,
): value is Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  return Object.keys(record).every((key) => allowedKeys.includes(key))
    && requiredKeys.every((key) => Object.prototype.hasOwnProperty.call(record, key));
}

function isApplicationRiskSummary(value: unknown): value is ExternalApplicationRiskSummaryV2 {
  if (!isExactRecord(value, ['highestLevel', 'reasonCodes'], [])) return false;
  return (value.highestLevel === undefined
      || value.highestLevel === null
      || isOneOf(value.highestLevel, APPLICATION_RISK_LEVELS))
    && (value.reasonCodes === undefined
      || (Array.isArray(value.reasonCodes)
        && value.reasonCodes.every((reason) => typeof reason === 'string')));
}

function isApplicationRecoveryAction(
  value: unknown,
): value is ExternalApplicationRecoveryActionV2 {
  return isExactRecord(value, ['type']) && isOneOf(value.type, APPLICATION_RECOVERY_ACTIONS);
}

function isApplicationReviewItemRef(
  value: unknown,
): value is ExternalApplicationReviewItemRefV2 {
  return isExactRecord(value, ['kind', 'stableId'])
    && isOneOf(value.kind, APPLICATION_REVIEW_KINDS)
    && typeof value.stableId === 'string';
}

function isApplicationOwnerGeneration(
  value: unknown,
): value is ExternalApplicationOwnerGenerationV2 {
  return isExactRecord(value, ['owner', 'generation'])
    && isOneOf(value.owner, APPLICATION_REVIEW_KINDS)
    && isNonNegativeInteger(value.generation);
}

function isApplicationReviewItem(value: unknown): value is ExternalApplicationReviewItemV2 {
  return isExactRecord(value, [
    'itemRef',
    'displayName',
    'displaySummary',
    'riskLevel',
    'riskReasonCodes',
    'recommended',
    'safetyCeiling',
  ])
    && isApplicationReviewItemRef(value.itemRef)
    && typeof value.displayName === 'string'
    && typeof value.displaySummary === 'string'
    && isOneOf(value.riskLevel, APPLICATION_RISK_LEVELS)
    && Array.isArray(value.riskReasonCodes)
    && value.riskReasonCodes.every((reason) => typeof reason === 'string')
    && typeof value.recommended === 'boolean'
    && isOneOf(value.safetyCeiling, APPLICATION_SAFETY_CEILINGS);
}

function isApplicationHostCapabilities(
  value: unknown,
): value is ExternalApplicationHostCapabilitiesV2 {
  const keys = [
    'canReadSnapshot',
    'canReadReview',
    'canMutate',
    'canManageUserDefault',
    'canManageWorkspaceOverride',
    'canRefresh',
    'canSetSafeMode',
  ] as const;
  return isExactRecord(value, keys) && keys.every((key) => typeof value[key] === 'boolean');
}

function isApplicationSummary(value: unknown): value is ExternalApplicationSummaryV2 {
  const keys = [
    'applicationId',
    'ecosystemId',
    'displayName',
    'discovery',
    'connection',
    'desiredConnection',
    'health',
    'effectiveStatus',
    'primaryAction',
    'defaultConnectionPolicy',
    'defaultConnectionReason',
    'enabledCount',
    'pendingReviewCount',
    'blockedCount',
    'conflictCount',
    'riskSummary',
    'noticeKey',
    'userDecision',
    'recoveryActions',
  ] as const;
  if (!isExactRecord(value, keys, keys.filter((key) => key !== 'noticeKey'))) return false;
  return typeof value.applicationId === 'string'
    && typeof value.ecosystemId === 'string'
    && typeof value.displayName === 'string'
    && isOneOf(value.discovery, APPLICATION_DISCOVERY_STATES)
    && isOneOf(value.connection, APPLICATION_CONNECTION_STATES)
    && isOneOf(value.desiredConnection, APPLICATION_DESIRED_CONNECTIONS)
    && isOneOf(value.health, APPLICATION_HEALTH_STATES)
    && isOneOf(value.effectiveStatus, APPLICATION_EFFECTIVE_STATUSES)
    && isOneOf(value.primaryAction, APPLICATION_PRIMARY_ACTIONS)
    && isOneOf(value.defaultConnectionPolicy, APPLICATION_DEFAULT_POLICIES)
    && typeof value.defaultConnectionReason === 'string'
    && isNonNegativeInteger(value.enabledCount)
    && isNonNegativeInteger(value.pendingReviewCount)
    && isNonNegativeInteger(value.blockedCount)
    && isNonNegativeInteger(value.conflictCount)
    && isApplicationRiskSummary(value.riskSummary)
    && (value.noticeKey === undefined || value.noticeKey === null || typeof value.noticeKey === 'string')
    && isOneOf(value.userDecision, APPLICATION_USER_DECISIONS)
    && Array.isArray(value.recoveryActions)
    && value.recoveryActions.every(isApplicationRecoveryAction);
}

function isApplicationReviewSummary(
  value: unknown,
): value is ExternalApplicationReviewSummaryV2 {
  const keys = [
    'reviewId',
    'totalCount',
    'categoryCounts',
    'maxSelectionCount',
    'riskSummary',
    'recommendationSummary',
    'safetyCeiling',
  ] as const;
  if (!isExactRecord(value, keys)
    || typeof value.reviewId !== 'string'
    || !isNonNegativeInteger(value.totalCount)
    || !isNonNegativeInteger(value.maxSelectionCount)
    || value.maxSelectionCount > value.totalCount
    || !isApplicationRiskSummary(value.riskSummary)
    || !isOneOf(value.safetyCeiling, APPLICATION_SAFETY_CEILINGS)
    || !Array.isArray(value.categoryCounts)
    || !value.categoryCounts.every((entry) => (
      isExactRecord(entry, ['kind', 'count'])
      && isOneOf(entry.kind, APPLICATION_REVIEW_KINDS)
      && isNonNegativeInteger(entry.count)
    ))
    || !isExactRecord(
      value.recommendationSummary,
      ['recommendedCount', 'optionalCount', 'blockedCount'],
    )) return false;
  return isNonNegativeInteger(value.recommendationSummary.recommendedCount)
    && isNonNegativeInteger(value.recommendationSummary.optionalCount)
    && isNonNegativeInteger(value.recommendationSummary.blockedCount);
}

export function normalizeExternalApplicationSnapshotV2(
  value: unknown,
): ExternalApplicationSnapshotV2 {
  const keys = [
    'schemaVersion',
    'executionDomainId',
    'workspaceScopeId',
    'effectiveConnectionScope',
    'refreshGeneration',
    'preferenceRevision',
    'safeMode',
    'hostCapabilities',
    'applications',
    'reviewSummary',
  ] as const;
  const required = keys.filter(
    (key) => key !== 'workspaceScopeId' && key !== 'reviewSummary',
  );
  if (!isExactRecord(value, keys, required)
    || value.schemaVersion !== 2
    || typeof value.executionDomainId !== 'string'
    || (value.workspaceScopeId !== undefined
      && value.workspaceScopeId !== null
      && typeof value.workspaceScopeId !== 'string')
    || !isOneOf(value.effectiveConnectionScope, APPLICATION_TARGET_SCOPES)
    || !isNonNegativeInteger(value.refreshGeneration)
    || !isNonNegativeInteger(value.preferenceRevision)
    || typeof value.safeMode !== 'boolean'
    || !isApplicationHostCapabilities(value.hostCapabilities)
    || !Array.isArray(value.applications)
    || !value.applications.every(isApplicationSummary)
    || (value.reviewSummary !== undefined
      && value.reviewSummary !== null
      && !isApplicationReviewSummary(value.reviewSummary))) {
    throw new ExternalSourceApiError(
      'invalid_response',
      'External application V2 snapshot schema was invalid',
      false,
    );
  }
  return {
    ...value,
    workspaceScopeId: typeof value.workspaceScopeId === 'string'
      ? value.workspaceScopeId
      : undefined,
    applications: value.applications.map((application) => ({
      ...application,
      noticeKey: typeof application.noticeKey === 'string' ? application.noticeKey : undefined,
      riskSummary: {
        ...(application.riskSummary.highestLevel
          ? { highestLevel: application.riskSummary.highestLevel }
          : {}),
        reasonCodes: [...(application.riskSummary.reasonCodes ?? [])],
      },
      recoveryActions: [...application.recoveryActions],
    })),
    reviewSummary: value.reviewSummary && isApplicationReviewSummary(value.reviewSummary)
      ? {
          ...value.reviewSummary,
          categoryCounts: [...value.reviewSummary.categoryCounts],
          riskSummary: {
            ...(value.reviewSummary.riskSummary.highestLevel
              ? { highestLevel: value.reviewSummary.riskSummary.highestLevel }
              : {}),
            reasonCodes: [...(value.reviewSummary.riskSummary.reasonCodes ?? [])],
          },
          recommendationSummary: { ...value.reviewSummary.recommendationSummary },
        }
      : undefined,
  } as ExternalApplicationSnapshotV2;
}

export function normalizeExternalApplicationReviewPageV2(
  value: unknown,
): ExternalApplicationReviewPageV2 {
  const keys = [
    'schemaVersion',
    'executionDomainId',
    'workspaceScopeId',
    'targetScope',
    'reviewId',
    'preferenceRevision',
    'expectedGenerations',
    'cursor',
    'nextCursor',
    'totalCount',
    'items',
  ] as const;
  const required = keys.filter(
    (key) => key !== 'workspaceScopeId' && key !== 'cursor' && key !== 'nextCursor',
  );
  if (!isExactRecord(value, keys, required)
    || value.schemaVersion !== 2
    || typeof value.executionDomainId !== 'string'
    || (value.workspaceScopeId !== undefined
      && value.workspaceScopeId !== null
      && typeof value.workspaceScopeId !== 'string')
    || !isOneOf(value.targetScope, APPLICATION_TARGET_SCOPES)
    || (value.targetScope === 'workspace_override' && typeof value.workspaceScopeId !== 'string')
    || (value.targetScope === 'user_default' && value.workspaceScopeId != null)
    || typeof value.reviewId !== 'string'
    || !isNonNegativeInteger(value.preferenceRevision)
    || !Array.isArray(value.expectedGenerations)
    || !value.expectedGenerations.every(isApplicationOwnerGeneration)
    || (value.cursor !== undefined && value.cursor !== null && typeof value.cursor !== 'string')
    || (value.nextCursor !== undefined
      && value.nextCursor !== null
      && typeof value.nextCursor !== 'string')
    || !isNonNegativeInteger(value.totalCount)
    || !Array.isArray(value.items)
    || value.items.length > 128
    || value.items.length > value.totalCount
    || !value.items.every(isApplicationReviewItem)) {
    throw new ExternalSourceApiError(
      'invalid_response',
      'External application V2 review page schema was invalid',
      false,
    );
  }
  return {
    ...value,
    workspaceScopeId: typeof value.workspaceScopeId === 'string'
      ? value.workspaceScopeId
      : undefined,
    cursor: typeof value.cursor === 'string' ? value.cursor : undefined,
    nextCursor: typeof value.nextCursor === 'string' ? value.nextCursor : undefined,
    expectedGenerations: value.expectedGenerations.map((entry) => ({ ...entry })),
    items: value.items.map((item) => ({
      ...item,
      itemRef: { ...item.itemRef },
      riskReasonCodes: [...item.riskReasonCodes],
    })),
  } as ExternalApplicationReviewPageV2;
}

export function normalizeExternalApplicationControlResultV2(
  value: unknown,
): ExternalApplicationControlResultV2 {
  if (!isExactRecord(value, [
    'schemaVersion',
    'operationId',
    'preferenceRevision',
    'outcome',
    'itemResults',
  ])
    || value.schemaVersion !== 2
    || typeof value.operationId !== 'string'
    || !isNonNegativeInteger(value.preferenceRevision)
    || !isOneOf(value.outcome, APPLICATION_OPERATION_OUTCOMES)
    || !Array.isArray(value.itemResults)
    || !value.itemResults.every((result) => (
      isExactRecord(
        result,
        ['itemRef', 'outcome', 'reasonCode', 'recoveryActions'],
        ['itemRef', 'outcome', 'recoveryActions'],
      )
      && isApplicationReviewItemRef(result.itemRef)
      && isOneOf(result.outcome, APPLICATION_OPERATION_OUTCOMES)
      && (result.reasonCode === undefined
        || result.reasonCode === null
        || typeof result.reasonCode === 'string')
      && Array.isArray(result.recoveryActions)
      && result.recoveryActions.every(isApplicationRecoveryAction)
    ))) {
    throw new ExternalSourceApiError(
      'invalid_response',
      'External application V2 action result schema was invalid',
      false,
    );
  }
  return {
    ...value,
    itemResults: value.itemResults.map((result) => ({
      ...result,
      itemRef: { ...result.itemRef },
      reasonCode: typeof result.reasonCode === 'string' ? result.reasonCode : undefined,
      recoveryActions: [...result.recoveryActions],
    })),
  } as ExternalApplicationControlResultV2;
}

function isHostCapabilities(
  value: unknown,
): value is ExternalSourceCatalogSnapshot['hostCapabilities'] {
  if (!value || typeof value !== 'object') return false;
  const record = value as Record<string, unknown>;
  return Object.keys(record).every(
    (key) => (HOST_CAPABILITY_KEYS as readonly string[]).includes(key),
  )
    && REQUIRED_HOST_CAPABILITY_KEYS.every((key) => typeof record[key] === 'boolean')
    && (record.canRevealSourceLocation === undefined
      || typeof record.canRevealSourceLocation === 'boolean');
}

function hostCapabilitiesEqual(
  left: ExternalSourceCatalogSnapshot['hostCapabilities'],
  right: ExternalSourceCatalogSnapshot['hostCapabilities'],
): boolean {
  return HOST_CAPABILITY_KEYS.every((key) => left[key] === right[key]);
}

function normalizeHostCapabilities(
  value: unknown,
): ExternalSourceCatalogSnapshot['hostCapabilities'] {
  const capabilities = value && typeof value === 'object'
    ? value as Partial<ExternalSourceCatalogSnapshot['hostCapabilities']>
    : undefined;
  return {
    ...READ_ONLY_HOST_CAPABILITIES,
    canRefresh: capabilities?.canRefresh === true,
    canMutatePolicy: capabilities?.canMutatePolicy === true,
    canManageSources: capabilities?.canManageSources === true,
    canApproveRuntime: capabilities?.canApproveRuntime === true,
    canExecuteExternalAssets: capabilities?.canExecuteExternalAssets === true,
    canSetSafeMode: capabilities?.canSetSafeMode === true,
    canRevealSourceLocation: capabilities?.canRevealSourceLocation === true,
  };
}

function isRecoveryAction(value: unknown): value is ExternalSourceRecoveryAction {
  return Boolean(value)
    && typeof value === 'object'
    && isOneOf((value as { type?: unknown }).type, RECOVERY_ACTION_TYPES);
}

function normalizeRecoveryActions(value: unknown, strict: boolean): ExternalSourceRecoveryAction[] {
  const actions = normalizeOptionalArray<unknown>(value);
  if (strict && !actions.every(isRecoveryAction)) {
    throw new ExternalSourceApiError(
      'invalid_response',
      'External source recovery actions were invalid',
      false,
    );
  }
  return actions.filter(isRecoveryAction);
}

function isControlReview(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false;
  const review = value as Record<string, unknown>;
  switch (review.state) {
    case 'not_required':
      return true;
    case 'required':
      return typeof review.contentVersion === 'string';
    default:
      return false;
  }
}

function isControlSource(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false;
  const source = value as Record<string, unknown>;
  return typeof source.stableKey === 'string'
    && typeof source.ecosystemId === 'string'
    && typeof source.displayName === 'string'
    && isOneOf(source.scope, EXTERNAL_SOURCE_SCOPES)
    && typeof source.contentVersion === 'string'
    && isOneOf(source.discovery, CONTROL_DISCOVERY_STATES)
    && isOneOf(source.desired, CONTROL_DESIRED_STATES)
    && isControlReview(source.review)
    && isOneOf(source.runtime, CONTROL_RUNTIME_STATES)
    && isOneOf(source.support, CONTROL_SUPPORT_STATES)
    && isOneOf(source.effectiveStatus, CONTROL_EFFECTIVE_STATUSES);
}

function isCapabilityControl(value: unknown): boolean {
  if (!value || typeof value !== 'object') return false;
  const capability = value as Record<string, unknown>;
  return isOneOf(capability.kind, CONTROL_CAPABILITY_KINDS)
    && isNonNegativeInteger(capability.revision)
    && isNonNegativeInteger(capability.itemCount)
    && isNonNegativeInteger(capability.pendingReviewCount)
    && isNonNegativeInteger(capability.unresolvedConflictCount)
    && isOneOf(capability.runtime, CONTROL_RUNTIME_STATES)
    && isOneOf(capability.support, CONTROL_SUPPORT_STATES);
}

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
  const rawTimeouts = definition.timeouts;
  const timeouts = rawTimeouts && typeof rawTimeouts === 'object'
    ? Object.fromEntries(
      (['startupMs', 'catalogMs', 'executionMs'] as const)
        .map((key) => [key, rawTimeouts[key]] as const)
        .filter(([, value]) => Number.isSafeInteger(value) && (value ?? 0) > 0),
    ) as ExternalMcpTimeouts
    : undefined;
  return {
    ...definition,
    provenance: normalizeOptionalArray(definition.provenance),
    environmentKeys: normalizeOptionalArray(definition.environmentKeys),
    environmentReferenceNames: normalizeOptionalArray(definition.environmentReferenceNames),
    headerNames: normalizeOptionalArray(definition.headerNames),
    timeouts: timeouts && Object.keys(timeouts).length > 0 ? timeouts : undefined,
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
      mode: subagent.mode ?? 'subagent',
      requestedModel: subagent.requestedModel ?? { kind: 'default' },
      modelBindingMethod: subagent.modelBindingMethod ?? 'default',
      sourceKeys: normalizeOptionalArray(subagent.sourceKeys),
      sourceLocationLabels: normalizeOptionalArray(subagent.sourceLocationLabels),
      effectiveToolLabels: normalizeOptionalArray(subagent.effectiveToolLabels),
      unavailableToolLabels: normalizeOptionalArray(subagent.unavailableToolLabels),
      diagnostics: normalizeOptionalArray(subagent.diagnostics),
    })),
    subagentModelBindingGroups: normalizeOptionalArray<ExternalSubagentModelBindingGroup>(
      candidate.subagentModelBindingGroups,
    ).map((group) => ({
      ...group,
      affectedCandidateIds: normalizeOptionalArray(group.affectedCandidateIds),
    })),
    subagentModelBindingOptions: normalizeOptionalArray<ExternalSubagentModelBindingOption>(
      candidate.subagentModelBindingOptions,
    ),
    subagentConflicts: normalizeOptionalArray<ExternalSubagentConflict>(candidate.subagentConflicts).map((conflict) => ({
      ...conflict,
      candidates: normalizeOptionalArray(conflict.candidates),
    })),
    pendingSubagentApprovals: normalizeOptionalArray(candidate.pendingSubagentApprovals),
    diagnostics: normalizeOptionalArray(candidate.diagnostics),
    hostCapabilities: normalizeHostCapabilities(capabilities),
    integrationPolicy: normalizePolicySnapshot(candidate.integrationPolicy),
  };
}

function normalizeControlSnapshot(value: unknown): ExternalSourceControlSnapshot {
  if (!value || typeof value !== 'object') {
    throw new ExternalSourceApiError('invalid_response', 'External source control response was not usable', true);
  }
  const candidate = value as Partial<ExternalSourceControlSnapshot>;
  if (candidate.schemaVersion !== 1
    || typeof candidate.executionDomainId !== 'string'
    || !isNonNegativeInteger(candidate.refreshGeneration)
    || !isNonNegativeInteger(candidate.preferenceRevision)
    || typeof candidate.safeMode !== 'boolean'
    || !isHostCapabilities(candidate.hostCapabilities)
    || !Array.isArray(candidate.sources)
    || !Array.isArray(candidate.capabilities)
    || !candidate.sources.every(isControlSource)
    || !candidate.capabilities.every(isCapabilityControl)) {
    throw new ExternalSourceApiError('invalid_response', 'External source control schema was invalid', false);
  }
  return {
    ...candidate,
    hostCapabilities: normalizeHostCapabilities(candidate.hostCapabilities),
    diagnostics: normalizeOptionalArray(candidate.diagnostics),
    recoveryActions: normalizeRecoveryActions(candidate.recoveryActions, true),
  } as ExternalSourceControlSnapshot;
}

function normalizeSurfaceSnapshot(value: unknown): ExternalSourceSurfaceSnapshot {
  if (!value || typeof value !== 'object') {
    throw new ExternalSourceApiError('invalid_response', 'External source surface response was not usable', true);
  }
  const candidate = value as Partial<ExternalSourceSurfaceSnapshot>;
  const control = normalizeControlSnapshot(candidate.control);
  const rawCatalog = candidate.catalog;
  const catalog = normalizeSnapshot(candidate.catalog);
  if (catalog.generation !== control.refreshGeneration) {
    throw new ExternalSourceApiError(
      'invalid_response',
      'External source control and catalog generations did not match',
      true,
    );
  }
  if ((catalog.preferenceRevision ?? 0) !== control.preferenceRevision) {
    throw new ExternalSourceApiError(
      'invalid_response',
      'External source control and catalog preference revisions did not match',
      true,
    );
  }
  if (rawCatalog && typeof rawCatalog === 'object'
    && Object.prototype.hasOwnProperty.call(rawCatalog, 'hostCapabilities')) {
    const rawCapabilities = (rawCatalog as { hostCapabilities?: unknown }).hostCapabilities;
    if (!isHostCapabilities(rawCapabilities)
      || !hostCapabilitiesEqual(
        control.hostCapabilities,
        normalizeHostCapabilities(rawCapabilities),
      )) {
      throw new ExternalSourceApiError(
        'invalid_response',
        'External source control and catalog Host capabilities did not match',
        false,
      );
    }
  }
  return { control, catalog: { ...catalog, control } };
}

const OPERATION_ERROR_CODES = new Set<ExternalSourceOperationErrorCode>([
  'invalid_request',
  'host_unavailable',
  'host_capability_unavailable',
  'trust_required',
  'policy_incompatible',
  'policy_limited',
  'stale_revision',
  'conflict',
  'not_found',
  'unavailable',
  'runtime_unavailable',
  'unsupported',
  'incompatible_version',
  'dependency_failed',
  'timeout',
  'cancelled',
  'overloaded',
  'process_lost',
  'invalid_response',
  'temporarily_unavailable',
  'internal',
]);

function normalizeOperationReference(value: unknown): string | undefined {
  if (typeof value !== 'string'
    || value.length === 0
    || value.length > 160
    || value.trim() !== value
    || Array.from(value).some(isControlCharacter)) {
    return undefined;
  }
  return value;
}

function normalizeOperationDetail(value: string): string {
  const bounded = Array.from(value)
    .slice(0, 4096)
    .map((character) => isControlCharacter(character) ? ' ' : character)
    .join('');
  return bounded.trim() ? bounded : 'External source operation failed';
}

function isControlCharacter(character: string): boolean {
  const codePoint = character.codePointAt(0);
  return codePoint !== undefined && (
    codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f)
  );
}

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
      normalizeOperationDetail(record.detail),
      record.retryable === true,
      normalizeOperationReference(record.correlationId),
      normalizeOperationReference(record.causationId),
      isOneOf(record.stage, OPERATION_STAGES) ? record.stage : undefined,
      normalizeRecoveryActions(record.recoveryActions, false),
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

export async function invokeExternalSourceCommand<T>(
  command: string,
  args: Record<string, unknown>,
): Promise<T> {
  try {
    return await api.invoke<T>(command, args);
  } catch (error) {
    const parsed = parseOperationError(error);
    let raw = typeof error === 'string'
      ? error
      : error instanceof Error
        ? error.message
        : '';
    if (!raw) {
      try {
        raw = JSON.stringify(error) ?? '';
      } catch {
        raw = '';
      }
    }
    const normalized = raw.toLowerCase();
    const legacyPeerMissingCommand = raw.includes(command)
      && normalized.includes('not supported on cli peer host');
    const legacyServerMissingCommand = parsed?.code === 'host_capability_unavailable'
      && parsed.detail === 'Unknown Server Host operation';
    if (legacyPeerMissingCommand
      || legacyServerMissingCommand
      || (raw.includes(command)
      && (normalized.includes('unknown command')
        || normalized.includes('not found')
        || normalized.includes('not registered')))) {
      throw new ExternalSourceApiError(
        'incompatible_version',
        'The connected Host does not support this external source command',
        false,
      );
    }
    throw parsed ?? new ExternalSourceApiError(
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
  await invokeExternalSourceCommand<unknown>(command, args);
  const request = args.request && typeof args.request === 'object'
    ? args.request as Record<string, unknown>
    : {};
  return (await invokeCompatibleSurfaceSnapshot({
    request: {
      workspacePath: request.workspacePath,
      forceRefresh: false,
    },
  })).catalog;
}

async function invokeSurfaceSnapshot(
  command: string,
  args: Record<string, unknown>,
): Promise<ExternalSourceSurfaceSnapshot> {
  return normalizeSurfaceSnapshot(await invokeExternalSourceCommand<unknown>(command, args));
}

function legacySurfaceSnapshot(catalog: ExternalSourceCatalogSnapshot): ExternalSourceSurfaceSnapshot {
  const hostCapabilities = {
    ...catalog.hostCapabilities,
    canSetSafeMode: false,
    canRevealSourceLocation: false,
  };
  const control: ExternalSourceControlSnapshot = {
    schemaVersion: 1,
    executionDomainId: catalog.sources[0]?.record.executionDomainId ?? 'legacy-host',
    refreshGeneration: catalog.generation,
    preferenceRevision: catalog.preferenceRevision ?? 0,
    safeMode: false,
    hostCapabilities,
    sources: [],
    capabilities: [],
    diagnostics: catalog.diagnostics ?? [],
    recoveryActions: [{ type: 'reconnect_host' }],
  };
  return {
    control,
    catalog: { ...catalog, hostCapabilities, control },
  };
}

async function invokeCompatibleSurfaceSnapshot(
  args: Record<string, unknown>,
): Promise<ExternalSourceSurfaceSnapshot> {
  try {
    return await invokeSurfaceSnapshot('get_external_source_control_snapshot', args);
  } catch (error) {
    if (!(error instanceof ExternalSourceApiError) || error.code !== 'incompatible_version') {
      throw error;
    }
    const catalog = normalizeSnapshot(await invokeExternalSourceCommand<unknown>(
      'get_external_source_snapshot',
      args,
    ));
    return legacySurfaceSnapshot(catalog);
  }
}

export function normalizeOptionalWorkspacePath(
  workspacePath: string | undefined,
): string | undefined {
  const normalized = workspacePath?.trim();
  return normalized || undefined;
}

let operationSequence = 0;

function nextOperationId(action: ExternalSourceControlAction['type']): string {
  operationSequence += 1;
  return `web-${action}-${Date.now().toString(36)}-${operationSequence.toString(36)}`;
}

function controlRequest(
  action: ExternalSourceControlAction,
  expectedPreferenceRevision?: number,
): ExternalSourceControlRequest {
  return {
    schemaVersion: 1,
    operationId: nextOperationId(action.type),
    ...(expectedPreferenceRevision === undefined ? {} : { expectedPreferenceRevision }),
    action,
  };
}

function emitExternalAgentCatalogUpdated(workspacePath?: string) {
  globalEventBus.emit('mode:config:updated', {
    reason: 'external-agent-catalog-updated',
    workspacePath: normalizeOptionalWorkspacePath(workspacePath),
  });
}

export const externalSourcesAPI = {
  async getApplicationReviewPage(
    workspacePath: string | undefined,
    request: ExternalApplicationReviewPageRequestV2,
  ) {
    const page = normalizeExternalApplicationReviewPageV2(
      await invokeExternalSourceCommand<unknown>(
        'get_external_application_review_page_v2',
        {
          request: {
            workspacePath: normalizeOptionalWorkspacePath(workspacePath),
            request,
          },
        },
      ),
    );
    if (page.executionDomainId !== request.executionDomainId
      || page.workspaceScopeId !== request.workspaceScopeId
      || page.targetScope !== request.targetScope
      || page.reviewId !== request.reviewId
      || page.preferenceRevision !== request.preferenceRevision
      || page.cursor !== request.cursor) {
      throw new ExternalSourceApiError(
        'invalid_response',
        'External application V2 review page did not match its request',
        false,
      );
    }
    return page;
  },

  async applyApplicationAction(
    workspacePath: string | undefined,
    request: ExternalApplicationControlRequestV2,
  ) {
    const result = normalizeExternalApplicationControlResultV2(
      await invokeExternalSourceCommand<unknown>(
        'apply_external_application_action_v2',
        {
          request: {
            workspacePath: normalizeOptionalWorkspacePath(workspacePath),
            request,
          },
        },
      ),
    );
    if (result.operationId !== request.operationId) {
      throw new ExternalSourceApiError(
        'invalid_response',
        'External application V2 action result did not match its operation',
        false,
      );
    }
    return result;
  },

  async getApplicationSurface(workspacePath?: string, forceRefresh = false) {
    const request = {
      workspacePath: normalizeOptionalWorkspacePath(workspacePath),
      forceRefresh,
    };
    try {
      const snapshot = normalizeExternalApplicationSnapshotV2(
        await invokeExternalSourceCommand<unknown>(
          'get_external_application_snapshot_v2',
          { request },
        ),
      );
      return { protocol: 'v2' as const, snapshot };
    } catch (error) {
      if (!(error instanceof ExternalSourceApiError) || error.code !== 'incompatible_version') {
        throw error;
      }
      const snapshot = (await invokeCompatibleSurfaceSnapshot({ request })).catalog;
      return { protocol: 'v1' as const, snapshot };
    }
  },

  planMcpImport(workspacePath?: string) {
    return invokeExternalSourceCommand<ExternalMcpImportPlanV1>(
      'plan_external_mcp_import_command',
      { request: { workspacePath: normalizeOptionalWorkspacePath(workspacePath) } },
    );
  },

  applyMcpImport(
    workspacePath: string | undefined,
    plan: ExternalMcpImportPlanV1,
    selections: ExternalMcpImportSelectionV1[],
  ) {
    return invokeExternalSourceCommand<ExternalMcpImportApplyResultV1>(
      'apply_external_mcp_import_command',
      {
        request: {
          workspacePath: normalizeOptionalWorkspacePath(workspacePath),
          importRequest: {
            schemaVersion: 1,
            planFingerprint: plan.planFingerprint,
            selections,
          },
        },
      },
    );
  },

  async getControlSnapshot(workspacePath?: string, forceRefresh = false) {
    return invokeCompatibleSurfaceSnapshot({
      request: { workspacePath: normalizeOptionalWorkspacePath(workspacePath), forceRefresh },
    });
  },

  revealSourceLocation(workspacePath: string | undefined, sourceKey: string) {
    return invokeExternalSourceCommand<void>('reveal_external_source_location', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        sourceKey,
      },
    });
  },

  async getSnapshot(workspacePath?: string, forceRefresh = false) {
    return (await invokeCompatibleSurfaceSnapshot({
      request: { workspacePath: normalizeOptionalWorkspacePath(workspacePath), forceRefresh },
    })).catalog;
  },

  async getWorkspaceReferences(
    workspacePath: string,
    workspaceId?: string,
    forceRefresh = false,
  ) {
    const snapshot = await invokeExternalSourceCommand<WorkspaceReferenceSnapshot>(
      'get_workspace_reference_snapshot',
      {
        request: {
          workspacePath: normalizeOptionalWorkspacePath(workspacePath),
          workspaceId: workspaceId?.trim() || undefined,
          forceRefresh,
        },
      },
    );
    return {
      ...snapshot,
      references: normalizeOptionalArray<WorkspaceReferenceEntry>(snapshot.references),
      diagnostics: normalizeOptionalArray<NonNullable<WorkspaceReferenceSnapshot['diagnostics']>[number]>(
        snapshot.diagnostics,
      ),
    };
  },

  async expandPromptCommand(
    workspacePath: string | undefined,
    name: string,
    argumentsText: string,
    candidateId: string,
    expectedContentVersion: string,
    nativeCommands: NativePromptCommandDescriptor[],
    nativeConflictGuard?: {
      conflictKey: string;
      expectedPreferenceRevision: number;
    },
    shellReviewDecision?: PromptCommandShellReviewDecision,
  ) {
    const outcome = await invokeExternalSourceCommand<unknown>(
      'expand_external_prompt_command_command',
      {
        request: {
          workspacePath: normalizeOptionalWorkspacePath(workspacePath),
          name,
          arguments: argumentsText,
          nativeCommands,
          candidateId,
          expectedContentVersion,
          ...(nativeConflictGuard ? {
            expectedNativeConflictKey: nativeConflictGuard.conflictKey,
            expectedPreferenceRevision: nativeConflictGuard.expectedPreferenceRevision,
          } : {}),
          ...(shellReviewDecision ? { shellReviewDecision } : {}),
        },
      },
    );
    return normalizePromptCommandInvocationOutcome(outcome);
  },

  getNativePromptCommandConflicts(
    workspacePath: string | undefined,
    nativeCommands: NativePromptCommandDescriptor[],
  ) {
    return invokeExternalSourceCommand<NativePromptCommandConflictSnapshot>(
      'get_native_prompt_command_conflicts_command',
      {
        request: {
          workspacePath: normalizeOptionalWorkspacePath(workspacePath),
          nativeCommands,
        },
      },
    );
  },

  setNativePromptCommandConflictChoice(
    workspacePath: string | undefined,
    nativeCommands: NativePromptCommandDescriptor[],
    selectedCandidateId: string,
    expectedPreferenceRevision: number,
  ) {
    return invokeExternalSourceCommand<NativePromptCommandConflictSnapshot>(
      'set_native_prompt_command_conflict_choice_command',
      {
        request: {
          workspacePath: normalizeOptionalWorkspacePath(workspacePath),
          nativeCommands,
          selectedCandidateId,
          expectedPreferenceRevision,
        },
      },
    );
  },

  async setSourceEnabled(
    workspacePath: string | undefined,
    sourceKey: string,
    enabled: boolean,
    expectedPreferenceRevision: number,
  ) {
    const normalizedWorkspacePath = normalizeOptionalWorkspacePath(workspacePath);
    try {
      const surface = await invokeSurfaceSnapshot('apply_external_source_control_action_command', {
        request: {
          workspacePath: normalizedWorkspacePath,
          control: controlRequest(
            { type: 'set_source_enabled', sourceKey, enabled },
            expectedPreferenceRevision,
          ),
        },
      });
      emitExternalAgentCatalogUpdated(workspacePath);
      return surface.catalog;
    } catch (error) {
      if (!(error instanceof ExternalSourceApiError) || error.code !== 'incompatible_version') {
        throw error;
      }
      const catalog = await invokeSnapshot('set_external_source_enabled_command', {
        request: {
          workspacePath: normalizedWorkspacePath,
          sourceKey,
          enabled,
          expectedPreferenceRevision,
        },
      });
      emitExternalAgentCatalogUpdated(workspacePath);
      return catalog;
    }
  },

  async setSafeMode(
    workspacePath: string | undefined,
    enabled: boolean,
    expectedPreferenceRevision: number,
  ) {
    const surface = await invokeSurfaceSnapshot('apply_external_source_control_action_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        control: controlRequest(
          { type: 'set_safe_mode', enabled },
          expectedPreferenceRevision,
        ),
      },
    });
    return surface.catalog;
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

  async setSubagentActivation(
    workspacePath: string | undefined,
    candidateId: string,
    approved: boolean,
    expectedSubagentGeneration: number,
    expectedPreferenceRevision: number,
    decisionKey: string,
  ) {
    const catalog = await invokeSnapshot('set_external_subagent_activation_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        candidateId,
        approved,
        expectedSubagentGeneration,
        expectedPreferenceRevision,
        decisionKey,
      },
    });
    emitExternalAgentCatalogUpdated(workspacePath);
    return catalog;
  },

  async setSubagentModelBinding(
    workspacePath: string | undefined,
    bindingKey: string,
    target: ExternalSubagentModelBindingTarget | undefined,
    expectedSubagentGeneration: number,
    expectedPreferenceRevision: number,
  ) {
    const catalog = await invokeSnapshot('set_external_subagent_model_binding_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        bindingKey,
        target,
        expectedSubagentGeneration,
        expectedPreferenceRevision,
      },
    });
    emitExternalAgentCatalogUpdated(workspacePath);
    return catalog;
  },

  async chooseSubagentConflict(
    workspacePath: string | undefined,
    conflictKey: string,
    candidateId: string,
    approveExternal: boolean,
    expectedSubagentGeneration: number,
    expectedPreferenceRevision: number,
  ) {
    const catalog = await invokeSnapshot('choose_external_subagent_conflict_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        conflictKey,
        candidateId,
        approveExternal,
        expectedSubagentGeneration,
        expectedPreferenceRevision,
      },
    });
    emitExternalAgentCatalogUpdated(workspacePath);
    return catalog;
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

  async updateIntegrationPolicy(
    workspacePath: string | undefined,
    mutation: ExternalIntegrationPolicyMutation,
  ) {
    const catalog = await invokeSnapshot(
      'update_external_integration_policy_command',
      { request: { workspacePath: normalizeOptionalWorkspacePath(workspacePath), mutation } },
    );
    emitExternalAgentCatalogUpdated(workspacePath);
    return catalog;
  },

  /**
   * External applications found on this host that the user has never been told
   * about. The host owns this derivation so the desktop and the TUI cannot
   * disagree about what counts as new.
   */
  async getEcosystemAwareness(workspacePath?: string): Promise<string[]> {
    const response = await invokeExternalSourceCommand<{
      unacknowledgedEcosystemIds?: unknown;
    }>('get_external_ecosystem_awareness_command', {
      request: { workspacePath: normalizeOptionalWorkspacePath(workspacePath) },
    });
    return normalizeOptionalArray<string>(response.unacknowledgedEcosystemIds)
      .filter((ecosystemId): ecosystemId is string => typeof ecosystemId === 'string');
  },

  /** Clears the "new external application" hint for these ecosystems. */
  acknowledgeEcosystems(workspacePath: string | undefined, ecosystemIds: string[]) {
    return invokeExternalSourceCommand<void>('acknowledge_external_ecosystems_command', {
      request: {
        workspacePath: normalizeOptionalWorkspacePath(workspacePath),
        ecosystemIds,
      },
    });
  },
};

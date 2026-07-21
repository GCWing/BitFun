import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import {
  AlertTriangle,
  CheckCircle2,
  CircleDashed,
  FolderKanban,
  Globe2,
  MinusCircle,
  RefreshCw,
  Settings2,
  ShieldCheck,
} from 'lucide-react';
import {
  Button,
  ConfigPageLoading,
  ConfirmDialog,
  Select,
  Switch,
  Tooltip,
} from '@/component-library';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/PeerDeviceContext';
import { WorkspaceKind } from '@/shared/types';
import {
  externalSourcesAPI,
  type ExternalIntegrationAccess,
  type ExternalIntegrationMode,
  type ExternalIntegrationPolicyMutation,
  type ExternalSourceCatalogSnapshot,
  type ExternalSubagentSummary,
  type ExternalToolCatalogEntry,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageRow,
  ConfigPageSection,
} from './common';
import {
  buildExternalSourcePresentationGroups,
  catalogDiagnosticsWithoutSourceDuplicates,
  externalSourceDiagnosticKey,
} from '../externalSourcePresentation';
import { externalSourceRequestScopeKey } from './externalSourceRequestScope';
import './ExternalSourcesConfig.scss';

const DISCOVERY_POLL_DELAYS_MS = [750, 1_500, 3_000, 5_000] as const;
const SOURCE_COUNT_LABELS = [
  ['commands', 'sources.commandCount'],
  ['tools', 'sources.toolCount'],
  ['agents', 'sources.agentCount'],
  ['mcps', 'sources.mcpCount'],
] as const;

type SnapshotLoadResult =
  | { status: 'accepted'; snapshot: ExternalSourceCatalogSnapshot }
  | { status: 'ignored' }
  | { status: 'error' };

function abbreviatedLocation(location: string): string {
  const normalized = location.replace(/\\/g, '/');
  const segments = normalized.split('/').filter(Boolean);
  return segments.length <= 3 ? normalized : `…/${segments.slice(-3).join('/')}`;
}

function matchesToolSource(
  source: ExternalSourceCatalogSnapshot['sources'][number],
  tool: ExternalToolCatalogEntry,
): boolean {
  return source.record.key.providerId === tool.definition.id.target.source.providerId
    && source.record.key.sourceId === tool.definition.id.target.source.sourceId;
}

function agentDiagnosticCategory(code: string, blocksActivation: boolean): string {
  if (code.includes('configuration_unavailable')) return 'configurationUnavailable';
  if (code.includes('model_unavailable')) return 'modelUnavailable';
  if (code.includes('tool_unavailable')) return 'toolUnavailable';
  if (code.includes('type_invalid') || code.includes('definition_invalid')
    || code.endsWith('_invalid')) {
    return 'invalidDefinition';
  }
  return blocksActivation ? 'unsupportedBehavior' : 'ignoredOption';
}

function sourceDiagnosticCategory(code: string): string {
  if (code.includes('preference_read_failed')) return 'confirmationStateUnavailable';
  if (code.includes('conflict_history_write_failed')) return 'conflictHistoryUnavailable';
  if (code.includes('discovery_in_progress')) return 'checkInProgress';
  if (code.includes('timeout')) return 'checkTimedOut';
  if (code.includes('trust_required')) return 'confirmationRequired';
  if (code.includes('too_large') || code.includes('file_limit') || code.includes('bytes_limit')) {
    return 'sourceTooLarge';
  }
  if (code.includes('invalid') || code.includes('parse') || code.includes('definition')
    || code.includes('export_missing') || code.includes('name_unsupported')) {
    return 'invalidSettings';
  }
  if (code.includes('unreadable') || code.includes('read_failed')
    || code.includes('metadata_failed') || code.includes('directory_')) {
    return 'unreadableSource';
  }
  if (code.includes('projection_only') || code.includes('unsupported')
    || code.includes('restricted')) {
    return 'notSupported';
  }
  if (code.includes('failed')) return 'checkFailed';
  return 'sourceIssue';
}

function externalAgentModelLabel(model: string | undefined, t: TFunction): string {
  return model || t('agents.modelUnavailable');
}

function executionLocationLabel(t: TFunction, executionDomainId?: string): string {
  if (executionDomainId?.startsWith('local')) return t('executionLocation.local');
  if (executionDomainId?.startsWith('remote')) return t('executionLocation.remote');
  return t('executionLocation.unknown');
}

type ExternalSourcesError = {
  kind: 'load' | 'mutation';
  code?: string;
  detail: string;
  retryable: boolean;
  correlationId?: string;
};

type AgentChangeNotice = {
  key: string;
  candidateIds: string[];
  message: string;
};

function externalOperationErrorFacts(error: unknown): Pick<
ExternalSourcesError,
'code' | 'detail' | 'retryable' | 'correlationId'
> {
  if (error && typeof error === 'object') {
    const candidate = error as {
      code?: unknown;
      message?: unknown;
      retryable?: unknown;
      correlationId?: unknown;
    };
    const code = typeof candidate.code === 'string' ? candidate.code : undefined;
    return {
      code,
      detail: code && code !== 'internal' && typeof candidate.message === 'string'
        ? candidate.message
        : 'External source operation failed',
      retryable: candidate.retryable === true,
      correlationId: typeof candidate.correlationId === 'string'
        ? candidate.correlationId
        : undefined,
    };
  }
  return {
    detail: 'External source operation failed',
    retryable: false,
  };
}

function externalErrorMessageKey(error: ExternalSourcesError, hasSnapshot: boolean): string {
  if (['host_capability_unavailable', 'policy_limited', 'invalid_request', 'not_found']
    .includes(error.code ?? '')) return 'operationErrors.rejected';
  if (['stale_revision', 'conflict'].includes(error.code ?? '')) {
    return 'operationErrors.refreshRequired';
  }
  if (error.code === 'policy_incompatible') return 'operationErrors.policyIncompatible';
  if (error.code === 'internal') return 'operationErrors.internal';
  if (['unavailable', 'host_unavailable'].includes(error.code ?? '') || error.retryable) {
    return 'operationErrors.unavailableRetry';
  }
  if (error.kind === 'mutation') return 'errors.mutationUnknown';
  return hasSnapshot ? 'errors.refreshFailed' : 'errors.loadFailed';
}

const DISABLED_SUBAGENT_CONFLICT_CHOICE = '__bitfun_disabled__';
const KNOWN_INTEGRATION_MODES = new Set(['recommended', 'discover_only', 'disabled', 'custom']);
const KNOWN_INTEGRATION_ACCESS = new Set([
  'disabled',
  'discover_only',
  'ask_before_use',
  'auto',
]);

function unresolvedFirst<T extends { selectedCandidateId?: string }>(items: T[]): T[] {
  return [
    ...items.filter((item) => !item.selectedCandidateId),
    ...items.filter((item) => item.selectedCandidateId),
  ];
}

function activeAgentAvailabilityChanges(
  previous: ExternalSourceCatalogSnapshot | null,
  next: ExternalSourceCatalogSnapshot,
): Array<{ previous: ExternalSubagentSummary; state: string; decisionKey: string }> {
  if (!previous) return [];
  const nextById = new Map((next.subagents ?? []).map((agent) => [agent.candidateId, agent]));
  return (previous.subagents ?? [])
    .filter((agent) => agent.activationState.state === 'active')
    .flatMap((agent) => {
      const current = nextById.get(agent.candidateId);
      if (current?.activationState.state === 'active') return [];
      return [{
        previous: agent,
        state: current?.activationState.state ?? 'removed',
        decisionKey: current?.decisionKey ?? 'removed',
      }];
    });
}

const ExternalSourcesConfig: React.FC = () => {
  const { t } = useTranslation('settings/external-sources');
  const { workspace, workspacePath } = useCurrentWorkspace();
  const peerDevice = usePeerDeviceModeOptional();
  const translateRef = useRef(t);
  translateRef.current = t;
  const peerDeviceId = peerDevice?.peerMode.active ? peerDevice.peerMode.deviceId : undefined;
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [reviewingToolKey, setReviewingToolKey] = useState<string | null>(null);
  const [reviewingAgentKey, setReviewingAgentKey] = useState<string | null>(null);
  const [reviewingMcpKey, setReviewingMcpKey] = useState<string | null>(null);
  const [reviewingMcpConflictKey, setReviewingMcpConflictKey] = useState<string | null>(null);
  const [error, setError] = useState<ExternalSourcesError | null>(null);
  const [operationStatus, setOperationStatus] = useState<string | null>(null);
  const [policyScope, setPolicyScope] = useState<'user' | 'workspace'>(
    workspacePath ? 'workspace' : 'user',
  );
  const [expandedEcosystems, setExpandedEcosystems] = useState<Set<string>>(() => new Set());
  const [resetPolicyConfirmation, setResetPolicyConfirmation] = useState<{
    requestScope: string;
    workspacePath?: string;
    preferenceRevision: number;
  } | null>(null);
  const [agentChangeNotice, setAgentChangeNotice] = useState<AgentChangeNotice | null>(null);
  const snapshotRef = useRef<ExternalSourceCatalogSnapshot | null>(null);
  const agentChangeNoticeRef = useRef<AgentChangeNotice | null>(null);
  const requestSequence = useRef(0);
  const acceptedSequence = useRef(0);
  const pendingMutations = useRef(new Map<number, string>());
  const latestMutationByScope = useRef(new Map<string, number>());
  const activeMutation = useRef<{ scope: string; sequence: number } | null>(null);
  const foregroundSequence = useRef<number | null>(null);
  const requestScope = externalSourceRequestScopeKey({
    peerDeviceId,
    workspaceId: workspace?.id,
    workspaceKind: workspace?.workspaceKind,
    remoteConnectionId: workspace?.connectionId,
    remoteHost: workspace?.sshHost,
    workspacePath,
  });
  const [snapshotState, setSnapshotState] = useState<{
    scope: string;
    snapshot: ExternalSourceCatalogSnapshot;
  } | null>(null);
  const snapshot = snapshotState?.scope === requestScope ? snapshotState.snapshot : null;
  const requestScopeRef = useRef(requestScope);
  useLayoutEffect(() => {
    if (requestScopeRef.current !== requestScope) {
      requestScopeRef.current = requestScope;
      requestSequence.current += 1;
      acceptedSequence.current = requestSequence.current;
      snapshotRef.current = null;
      agentChangeNoticeRef.current = null;
    }
  }, [requestScope]);

  const applySnapshot = useCallback((
    next: ExternalSourceCatalogSnapshot,
    scope: string,
    partition: 'all' | 'subagents' = 'all',
    origin: 'read' | 'mutation' = 'read',
  ) => {
    const current = snapshotRef.current;
    let selected = next;
    if (current && next.generation < current.generation) {
      if (partition !== 'subagents') return;
      if ((current.subagentGeneration ?? 0) > (next.subagentGeneration ?? 0)
        || (current.preferenceRevision ?? 0) > (next.preferenceRevision ?? 0)) {
        return;
      }
      selected = {
        ...current,
        subagentGeneration: next.subagentGeneration,
        preferenceRevision: next.preferenceRevision,
        subagents: next.subagents,
        subagentConflicts: next.subagentConflicts,
        pendingSubagentApprovals: next.pendingSubagentApprovals,
      };
    }

    if (origin === 'read') {
      const changes = activeAgentAvailabilityChanges(current, selected);
      if (changes.length > 0) {
        const key = changes
          .map((change) => `${change.previous.candidateId}:${change.state}:${change.decisionKey}`)
          .sort()
          .join('|');
        if (agentChangeNoticeRef.current?.key !== key) {
          const message = changes.length === 1
            ? translateRef.current('agentChanges.unavailable', {
                name: changes[0].previous.displayName,
                state: changes[0].state === 'removed'
                  ? translateRef.current('agentChanges.removedState')
                  : translateRef.current(`agentState.${changes[0].state}`),
              })
            : translateRef.current('agentChanges.unavailableMany', { count: changes.length });
          const notice = {
            key,
            candidateIds: changes.map((change) => change.previous.candidateId),
            message,
          };
          agentChangeNoticeRef.current = notice;
          setAgentChangeNotice(notice);
        }
      } else if (agentChangeNoticeRef.current) {
        const currentById = new Map(
          (selected.subagents ?? []).map((agent) => [agent.candidateId, agent]),
        );
        const recovered = agentChangeNoticeRef.current.candidateIds.every(
          (candidateId) => currentById.get(candidateId)?.activationState.state === 'active',
        );
        if (recovered) {
          agentChangeNoticeRef.current = null;
          setAgentChangeNotice(null);
        }
      }
    }

    snapshotRef.current = selected;
    setSnapshotState({ scope, snapshot: selected });
  }, []);

  const acceptReadSnapshot = useCallback((
    next: ExternalSourceCatalogSnapshot,
    scope: string,
    sequence: number,
  ): boolean => {
    if (requestScopeRef.current !== scope || sequence < acceptedSequence.current) return false;
    if (Array.from(pendingMutations.current.values()).includes(scope)) return false;
    acceptedSequence.current = sequence;
    applySnapshot(next, scope);
    return true;
  }, [applySnapshot]);

  const acceptMutationSnapshot = useCallback((
    next: ExternalSourceCatalogSnapshot,
    scope: string,
    sequence: number,
    partition: 'all' | 'subagents',
  ): boolean => {
    if (requestScopeRef.current !== scope) return false;
    if ((latestMutationByScope.current.get(scope) ?? sequence) > sequence) return false;
    acceptedSequence.current = Math.max(acceptedSequence.current, sequence);
    applySnapshot(next, scope, partition, 'mutation');
    return true;
  }, [applySnapshot]);

  const loadSnapshot = useCallback(async (
    forceRefresh: boolean,
    foreground: boolean,
  ): Promise<SnapshotLoadResult> => {
    const scope = requestScope;
    const sequence = ++requestSequence.current;
    if (foreground) {
      foregroundSequence.current = sequence;
      setRefreshing(true);
    }
    try {
      const next = await externalSourcesAPI.getSnapshot(workspacePath, forceRefresh);
      if (!acceptReadSnapshot(next, scope, sequence)) return { status: 'ignored' };
      setError(null);
      return { status: 'accepted', snapshot: next };
    } catch (loadError) {
      if (requestScopeRef.current !== scope
        || sequence < acceptedSequence.current
        || Array.from(pendingMutations.current.values()).includes(scope)) {
        return { status: 'ignored' };
      }
      acceptedSequence.current = sequence;
      setError({ kind: 'load', ...externalOperationErrorFacts(loadError) });
      return { status: 'error' };
    } finally {
      if (requestScopeRef.current === scope) {
        if (sequence >= acceptedSequence.current) setLoading(false);
        if (foregroundSequence.current === sequence) {
          foregroundSequence.current = null;
          setRefreshing(false);
        }
      }
    }
  }, [acceptReadSnapshot, requestScope, workspacePath]);

  useEffect(() => {
    setSnapshotState(null);
    snapshotRef.current = null;
    agentChangeNoticeRef.current = null;
    setAgentChangeNotice(null);
    setError(null);
    setOperationStatus(null);
    setBusyKey(null);
    setReviewingToolKey(null);
    setReviewingAgentKey(null);
    setReviewingMcpKey(null);
    setReviewingMcpConflictKey(null);
    setResetPolicyConfirmation(null);
    setLoading(true);
    setPolicyScope(workspacePath ? 'workspace' : 'user');
    void loadSnapshot(false, false);
    const refreshWhenActive = () => {
      if (document.visibilityState === 'visible') {
        void loadSnapshot(false, false);
      }
    };
    window.addEventListener('focus', refreshWhenActive);
    document.addEventListener('visibilitychange', refreshWhenActive);
    return () => {
      window.removeEventListener('focus', refreshWhenActive);
      document.removeEventListener('visibilitychange', refreshWhenActive);
    };
  }, [loadSnapshot, requestScope, workspacePath]);

  useEffect(() => {
    if (!snapshot?.discoveryPending) return undefined;
    let cancelled = false;
    let timer: number | undefined;
    let attempt = 0;
    const schedulePoll = () => {
      const delay = DISCOVERY_POLL_DELAYS_MS[
        Math.min(attempt, DISCOVERY_POLL_DELAYS_MS.length - 1)
      ];
      timer = window.setTimeout(async () => {
        const result = await loadSnapshot(false, false);
        if (cancelled) return;
        if (result.status === 'accepted' && !result.snapshot.discoveryPending) return;
        attempt += 1;
        schedulePoll();
      }, delay);
    };
    schedulePoll();
    return () => {
      cancelled = true;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, [loadSnapshot, snapshot?.discoveryPending]);

  const sourceGroups = useMemo(
    () => snapshot ? buildExternalSourcePresentationGroups(snapshot) : [],
    [snapshot],
  );
  const opencodeGroups = useMemo(
    () => sourceGroups.filter((group) => group.displayName.toLowerCase().includes('opencode')),
    [sourceGroups],
  );
  const nonOpencodeGroups = useMemo(
    () => sourceGroups.filter((group) => !group.displayName.toLowerCase().includes('opencode')),
    [sourceGroups],
  );
  const catalogDiagnostics = useMemo(
    () => snapshot ? catalogDiagnosticsWithoutSourceDuplicates(snapshot, sourceGroups) : [],
    [snapshot, sourceGroups],
  );

  const commandConflicts = useMemo(
    () => unresolvedFirst(snapshot?.commandConflicts ?? []),
    [snapshot?.commandConflicts],
  );

  const toolConflicts = useMemo(
    () => unresolvedFirst(snapshot?.toolConflicts ?? []),
    [snapshot?.toolConflicts],
  );

  const agentConflicts = useMemo(
    () => unresolvedFirst(snapshot?.subagentConflicts ?? []),
    [snapshot?.subagentConflicts],
  );

  const mcpConflicts = useMemo(
    () => unresolvedFirst(snapshot?.mcpConflicts ?? []),
    [snapshot?.mcpConflicts],
  );

  const hostCapabilities = snapshot?.hostCapabilities ?? {
    canRefresh: false,
    canMutatePolicy: false,
    canManageSources: false,
    canApproveRuntime: false,
    canExecuteExternalAssets: false,
  };
  const policyStatus = snapshot?.integrationPolicy?.status;
  const policyCompatible = policyStatus === 'compatible';
  const policyIncompatible = policyStatus === 'incompatible_schema';
  const policyUnknown = !policyCompatible && !policyIncompatible;
  const hostReadOnly = !hostCapabilities.canMutatePolicy
    && !hostCapabilities.canManageSources
    && !hostCapabilities.canApproveRuntime;
  const remoteWorkspace = workspace?.workspaceKind === WorkspaceKind.Remote;
  const readOnlyHintKey = remoteWorkspace
    ? 'policy.remoteReadOnlyHint'
    : 'policy.hostReadOnlyHint';

  const runMutation = useCallback(async (
    mutationKey: string,
    request: () => Promise<ExternalSourceCatalogSnapshot>,
    _focusResult = false,
    partition: 'all' | 'subagents' = 'all',
    successMessage?: string,
    requiredCapability: 'canMutatePolicy' | 'canManageSources' | 'canApproveRuntime' = 'canManageSources',
    allowIncompatiblePolicy = false,
  ): Promise<boolean> => {
    const current = snapshotRef.current;
    const currentCapabilities = current?.hostCapabilities ?? {
      canMutatePolicy: false,
      canManageSources: false,
      canApproveRuntime: false,
    };
    const currentStatus = current?.integrationPolicy.status;
    if (!current || currentCapabilities[requiredCapability] !== true
      || (currentStatus !== 'compatible'
        && !(allowIncompatiblePolicy && currentStatus === 'incompatible_schema'))) {
      setOperationStatus(t(readOnlyHintKey));
      return false;
    }
    if (activeMutation.current?.scope === requestScope) {
      setOperationStatus(t('actions.waitForUpdate'));
      return false;
    }
    const scope = requestScope;
    const sequence = ++requestSequence.current;
    activeMutation.current = { scope, sequence };
    pendingMutations.current.set(sequence, scope);
    latestMutationByScope.current.set(scope, sequence);
    setBusyKey(mutationKey);
    setOperationStatus(null);
    try {
      setError(null);
      const next = await request();
      const accepted = acceptMutationSnapshot(next, scope, sequence, partition);
      if (accepted) {
        setOperationStatus(successMessage ?? t('actions.updated'));
      }
      return accepted;
    } catch (updateError) {
      if (requestScopeRef.current === scope
        && latestMutationByScope.current.get(scope) === sequence) {
        acceptedSequence.current = sequence;
        setError({ kind: 'mutation', ...externalOperationErrorFacts(updateError) });
      }
      return false;
    } finally {
      if (activeMutation.current?.scope === scope
        && activeMutation.current.sequence === sequence) {
        activeMutation.current = null;
      }
      pendingMutations.current.delete(sequence);
      if (requestScopeRef.current === scope) {
        setBusyKey((current) => (current === mutationKey ? null : current));
      }
    }
  }, [acceptMutationSnapshot, readOnlyHintKey, requestScope, t]);

  const setEnabled = useCallback(async (
    sourceKey: string,
    enabled: boolean,
  ) => {
    const currentSnapshot = snapshotRef.current;
    if (!currentSnapshot) return;
    await runMutation(
      sourceKey,
      () => externalSourcesAPI.setSourceEnabled(
        workspacePath,
        sourceKey,
        enabled,
        currentSnapshot.preferenceRevision ?? 0,
      ),
    );
  }, [runMutation, workspacePath]);

  const chooseConflict = useCallback(async (conflictKey: string, candidateId: string) => {
    if (!snapshot) return;
    await runMutation(
      conflictKey,
      () => externalSourcesAPI.setConflictChoice(
        workspacePath,
        conflictKey,
        candidateId,
        snapshot.preferenceRevision ?? 0,
      ),
      true,
      'all',
      undefined,
      'canApproveRuntime',
    );
  }, [runMutation, snapshot, workspacePath]);

  const decideToolTarget = useCallback(async (
    approvalKey: string,
    decisionKey: string,
    approved: boolean,
  ) => {
    if (!snapshot) return false;
    return runMutation(
      decisionKey,
      () => externalSourcesAPI.setToolTargetDecision(
        workspacePath,
        approvalKey,
        decisionKey,
        approved,
        snapshot.preferenceRevision ?? 0,
      ),
      true,
      'all',
      undefined,
      'canApproveRuntime',
    );
  }, [runMutation, snapshot, workspacePath]);

  const chooseToolConflict = useCallback(async (conflictKey: string, candidateId: string) => {
    if (!snapshot) return;
    await runMutation(
      conflictKey,
      () => externalSourcesAPI.setToolConflictChoice(
        workspacePath,
        conflictKey,
        candidateId,
        snapshot.preferenceRevision ?? 0,
      ),
      true,
      'all',
      undefined,
      'canApproveRuntime',
    );
  }, [runMutation, snapshot, workspacePath]);

  const decideAgent = useCallback(async (candidateId: string, decisionKey: string, approved: boolean) => {
    if (!snapshot) return false;
    const label = snapshot.subagents?.find((agent) => agent.candidateId === candidateId)
      ?.displayName ?? candidateId;
    const accepted = await runMutation(
      decisionKey,
      () => externalSourcesAPI.setSubagentActivation(
        workspacePath,
        candidateId,
        approved,
        snapshot.subagentGeneration ?? 0,
        snapshot.preferenceRevision ?? 0,
        decisionKey,
      ),
      true,
      'subagents',
      t('actions.agentUpdated', { name: label }),
      'canApproveRuntime',
    );
    if (accepted) await loadSnapshot(true, false);
    return accepted;
  }, [loadSnapshot, runMutation, snapshot, t, workspacePath]);

  const chooseAgentConflict = useCallback(async (
    conflictKey: string,
    candidateId: string,
    approveExternal: boolean,
  ) => {
    if (!snapshot) return;
    const logicalId = snapshot.subagentConflicts
      ?.find((conflict) => conflict.conflictKey === conflictKey)?.logicalId ?? conflictKey;
    const accepted = await runMutation(
      conflictKey,
      () => externalSourcesAPI.chooseSubagentConflict(
        workspacePath,
        conflictKey,
        candidateId,
        approveExternal,
        snapshot.subagentGeneration ?? 0,
        snapshot.preferenceRevision ?? 0,
      ),
      true,
      'subagents',
      t('actions.agentUpdated', { name: logicalId }),
      'canApproveRuntime',
    );
    if (accepted) await loadSnapshot(true, false);
  }, [loadSnapshot, runMutation, snapshot, t, workspacePath]);

  const decideMcpServer = useCallback(async (
    candidateId: string,
    decisionKey: string,
    approved: boolean,
  ) => {
    if (!snapshot) return false;
    const accepted = await runMutation(
      decisionKey,
      () => externalSourcesAPI.setMcpServerDecision(
        workspacePath,
        candidateId,
        decisionKey,
        approved,
        snapshot.mcpGeneration ?? 0,
        snapshot.preferenceRevision ?? 0,
      ),
      true,
      'all',
      t('actions.mcpUpdated'),
      'canApproveRuntime',
    );
    if (accepted) await loadSnapshot(true, false);
    return accepted;
  }, [loadSnapshot, runMutation, snapshot, t, workspacePath]);

  const chooseMcpConflict = useCallback(async (
    conflictKey: string,
    candidateId: string,
    approveExternal: boolean,
  ) => {
    if (!snapshot) return false;
    const accepted = await runMutation(
      conflictKey,
      () => externalSourcesAPI.chooseMcpConflict(
        workspacePath,
        conflictKey,
        candidateId,
        approveExternal,
        snapshot.mcpGeneration ?? 0,
        snapshot.preferenceRevision ?? 0,
      ),
      true,
      'all',
      t('actions.mcpUpdated'),
      'canApproveRuntime',
    );
    if (accepted) await loadSnapshot(true, false);
    return accepted;
  }, [loadSnapshot, runMutation, snapshot, t, workspacePath]);

  const isRemote = workspace?.workspaceKind === 'remote' || Boolean(workspace?.connectionId);
  const policy = snapshot?.integrationPolicy;
  const selectedPolicyEnabled = policyScope === 'workspace'
    ? policy?.workspaceOverride?.enabled ?? policy?.userDefaults.enabled ?? true
    : policy?.userDefaults.enabled ?? true;
  const selectedPolicyEffective = policyScope === 'workspace'
    ? policy?.effective
    : policy?.globalEffective;
  const workspacePolicyInherited = policyScope === 'workspace'
    && policy?.workspaceOverride?.enabled === undefined
    && Object.values(policy?.workspaceOverride?.ecosystems ?? {}).every((ecosystem) => (
      ecosystem.mode === undefined
      && Object.keys(ecosystem.capabilityOverrides ?? {}).length === 0
    ));
  const ecosystemPolicies = (policy?.registeredEcosystems ?? []).map((descriptor) => {
    const ecosystemId = descriptor.ecosystemId;
    const userPolicy = policy?.userDefaults.ecosystems?.[ecosystemId];
    const workspacePolicy = policy?.workspaceOverride?.ecosystems?.[ecosystemId];
    const mode: ExternalIntegrationMode = policyScope === 'workspace'
      ? workspacePolicy?.mode ?? userPolicy?.mode ?? 'recommended'
      : userPolicy?.mode ?? 'recommended';
    const capabilityOverrides = policyScope === 'workspace'
      ? {
          ...(userPolicy?.capabilityOverrides ?? {}),
          ...(workspacePolicy?.capabilityOverrides ?? {}),
        }
      : userPolicy?.capabilityOverrides ?? {};
    const sources = (snapshot?.sources ?? []).filter(
      (source) => source.record.ecosystemId === ecosystemId,
    );
    const hasIssue = sources.some((source) => (
      ['degraded', 'unavailable'].includes(source.record.health)
      || (source.record.diagnostics?.length ?? 0) > 0
    ));
    const state: 'checking' | 'attention' | 'ready' | 'noConfig' = snapshot?.discoveryPending
      ? 'checking'
      : !policyCompatible || hasIssue
        ? 'attention'
        : sources.length > 0
          ? 'ready'
          : 'noConfig';
    return {
      descriptor,
      ecosystemId,
      mode,
      capabilityOverrides,
      effective: selectedPolicyEffective?.ecosystems[ecosystemId],
      sourceLocations: Array.from(new Set(sources.map((source) => source.record.location))),
      state,
    };
  });
  const selectedCapabilityAccess = (
    ecosystem: (typeof ecosystemPolicies)[number],
    capabilityId: string,
  ): ExternalIntegrationAccess => {
    if (!selectedPolicyEnabled) return 'disabled';
    if (ecosystem.mode === 'recommended') {
      return ecosystem.descriptor.capabilities
        .find((capability) => capability.capabilityId === capabilityId)
        ?.recommendedAccess ?? 'disabled';
    }
    if (ecosystem.mode === 'discover_only') return 'discover_only';
    if (ecosystem.mode === 'disabled' || !KNOWN_INTEGRATION_MODES.has(ecosystem.mode)) {
      return 'disabled';
    }
    const requested = ecosystem.capabilityOverrides[capabilityId] ?? 'discover_only';
    const ceiling = ecosystem.descriptor.capabilities
      .find((capability) => capability.capabilityId === capabilityId)
      ?.safetyCeiling ?? 'disabled';
    const accessRank: Record<string, number> = {
      disabled: 0,
      discover_only: 1,
      ask_before_use: 2,
      auto: 3,
    };
    if (accessRank[requested] === undefined || accessRank[ceiling] === undefined) {
      return 'disabled';
    }
    return accessRank[requested] <= accessRank[ceiling] ? requested : ceiling;
  };
  const diagnosticAttentionCount = catalogDiagnostics
    .filter((diagnostic) => diagnostic.severity !== 'info').length
    + sourceGroups.reduce((count, group) => count + group.diagnostics.length, 0);
  const externalAttentionCount = (snapshot?.toolApprovalRequests?.length ?? 0)
    + (snapshot?.pendingSubagentApprovals?.length ?? 0)
    + (snapshot?.mcpApprovalRequests?.length ?? 0)
    + commandConflicts.filter((conflict) => !conflict.selectedCandidateId).length
    + toolConflicts.filter((conflict) => !conflict.selectedCandidateId).length
    + agentConflicts.filter((conflict) => !conflict.selectedCandidateId).length
    + mcpConflicts.filter((conflict) => !conflict.selectedCandidateId).length
    + diagnosticAttentionCount
    + Number(Boolean(snapshot) && !policyCompatible);

  const updatePolicy = useCallback(async (
    change: ExternalIntegrationPolicyMutation['change'],
  ) => {
    if (!snapshot) return false;
    return runMutation(
      `integration-policy:${policyScope}`,
      () => externalSourcesAPI.updateIntegrationPolicy(workspacePath, {
        expectedPreferenceRevision: snapshot.preferenceRevision ?? 0,
        scope: policyScope,
        change,
      }),
      true,
      'all',
      t('policy.updated'),
      'canMutatePolicy',
      change.operation === 'reset_incompatible_policy',
    );
  }, [policyScope, runMutation, snapshot, t, workspacePath]);

  const updateCapabilityAccess = useCallback((
    ecosystemId: string,
    capabilityId: string,
    access: ExternalIntegrationAccess,
  ) => updatePolicy({
    operation: 'set_capability_access',
    ecosystemId,
    capabilityId,
    access,
  }), [updatePolicy]);

  const resetIncompatiblePolicy = useCallback((confirmation: {
    requestScope: string;
    workspacePath?: string;
    preferenceRevision: number;
  }) => {
    if (requestScope !== confirmation.requestScope) return Promise.resolve(false);
    return runMutation(
      'integration-policy:recovery',
      () => externalSourcesAPI.updateIntegrationPolicy(confirmation.workspacePath, {
        expectedPreferenceRevision: confirmation.preferenceRevision,
        scope: 'user',
        change: { operation: 'reset_incompatible_policy' },
      }),
      false,
      'all',
      t('policy.recoveryResetComplete'),
      'canMutatePolicy',
      true,
    );
  }, [requestScope, runMutation, t]);

  const scrollToFirstAttentionItem = useCallback(() => {
    const target = document.querySelector<HTMLElement>(
      '[data-external-attention="true"]',
    );
    if (!target) return;
    target.scrollIntoView({ block: 'center', behavior: 'smooth' });
    if (target instanceof HTMLDetailsElement) {
      target.open = true;
      target.querySelector<HTMLElement>('summary')?.focus();
      return;
    }
    const focusTarget = target.querySelector<HTMLElement>('button, [href], [tabindex]');
    if (focusTarget) {
      focusTarget.focus();
      return;
    }
    target.tabIndex = -1;
    target.focus();
  }, []);

  const renderPathLink = useCallback((location: string) => {
    const display = abbreviatedLocation(location);
    if (isRemote) {
      return (
        <Tooltip content={t('common.openInExplorerRemote')} placement="top">
          <span className="bitfun-external-sources-config__path-link bitfun-external-sources-config__path-link--disabled">
            {display}
          </span>
        </Tooltip>
      );
    }
    return (
      <a
        className="bitfun-external-sources-config__path-link"
        title={location}
        translate="no"
        onClick={(event) => {
          event.preventDefault();
          void workspaceAPI.revealInExplorer(location).catch(() => {
            // silently ignore — do not block UI
          });
        }}
      >
        {display}
      </a>
    );
  }, [isRemote, t]);

  if (loading && !snapshot) {
    return <ConfigPageLoading text={t('loading')} />;
  }

  const hostUnavailable = !snapshot && error?.code === 'host_unavailable';

  return (
    <ConfigPageLayout className="bitfun-external-sources-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
        extra={(
          <Tooltip
            content={refreshing ? t('actions.refreshing') : t('actions.refresh')}
            placement="top"
          >
            <Button
              variant="ghost"
              size="small"
              aria-label={refreshing ? t('actions.refreshing') : t('actions.refresh')}
              disabled={refreshing || (!hostCapabilities.canRefresh && !error)}
              onClick={() => void loadSnapshot(true, true)}
            >
              <RefreshCw size={15} aria-hidden="true" />
            </Button>
          </Tooltip>
        )}
      />
      <ConfigPageContent id="external-integration-attention-region">
        {hostUnavailable ? (
          <ConfigPageSection
            title={t('unavailable.hostTitle')}
            description={t(peerDeviceId
              ? 'unavailable.remoteConnectionDescription'
              : remoteWorkspace
                ? 'unavailable.remoteDescription'
                : 'unavailable.hostDescription')}
          >
            {null}
          </ConfigPageSection>
        ) : (
          <>
            {error ? (
              <div
                className="bitfun-external-sources-config__notice"
                role={error.kind === 'mutation' ? 'alert' : 'status'}
              >
                <div>{t(externalErrorMessageKey(error, Boolean(snapshot)))}</div>
                {error.correlationId ? (
                  <div>{t('operationErrors.referenceId', { id: error.correlationId })}</div>
                ) : null}
                <details>
                  <summary>{t('common.technicalDetails')}</summary>
                  <div>{error.detail}</div>
                </details>
              </div>
            ) : null}
            {snapshot && hostReadOnly ? (
              <div className="bitfun-external-sources-config__host-mode" role="status">
                <ShieldCheck size={16} aria-hidden="true" />
                <span>{t(readOnlyHintKey)}</span>
              </div>
            ) : null}
            {snapshot && policy ? (
              <ConfigPageSection
                className="bitfun-external-sources-config__policy-card"
                title={t('policy.title')}
                description={externalAttentionCount > 0 ? (
                  <span className="bitfun-external-sources-config__policy-summary">
                    <button
                      type="button"
                      aria-controls="external-integration-attention-region"
                      onClick={scrollToFirstAttentionItem}
                    >
                      {t('policy.attentionSummary', {
                        count: externalAttentionCount,
                      })}
                    </button>
                  </span>
                ) : undefined}
                extra={(
                  <Switch
                    size="small"
                    checked={selectedPolicyEnabled}
                    disabled={!policyCompatible || !hostCapabilities.canMutatePolicy}
                    loading={busyKey === `integration-policy:${policyScope}`}
                    aria-label={t('policy.enabledLabel')}
                    onChange={(event) => void updatePolicy({
                      operation: 'set_enabled',
                      enabled: event.currentTarget.checked,
                    })}
                  />
                )}
              >
                {policyIncompatible ? (
                  <div
                    className="bitfun-external-sources-config__policy-recovery"
                    role="alert"
                    data-external-attention="true"
                  >
                    <AlertTriangle size={16} aria-hidden="true" />
                    <span>{t('policy.recoveryRequired')}</span>
                    <Button
                      variant="secondary"
                      size="small"
                      disabled={busyKey !== null || !hostCapabilities.canMutatePolicy}
                      onClick={() => setResetPolicyConfirmation({
                        requestScope,
                        workspacePath,
                        preferenceRevision: snapshot.preferenceRevision ?? 0,
                      })}
                    >
                      {t('policy.backupAndReset')}
                    </Button>
                  </div>
                ) : null}
                {policyUnknown ? (
                  <div
                    className="bitfun-external-sources-config__policy-recovery"
                    role="alert"
                    data-external-attention="true"
                  >
                    <AlertTriangle size={16} aria-hidden="true" />
                    <span>{t('policy.unknownStatus')}</span>
                  </div>
                ) : null}

                <div className="bitfun-external-sources-config__scope-bar">
                  <button
                    type="button"
                    className={policyScope === 'user' ? 'is-active' : undefined}
                    aria-pressed={policyScope === 'user'}
                    onClick={() => setPolicyScope('user')}
                  >
                    <Globe2 size={14} aria-hidden="true" />
                    {t('policy.scope.user')}
                  </button>
                  <Tooltip
                    content={workspacePath
                      ? t('policy.scope.workspaceHint')
                      : t('policy.scope.workspaceUnavailable')}
                    placement="top"
                  >
                    <button
                      type="button"
                      className={policyScope === 'workspace' ? 'is-active' : undefined}
                      aria-pressed={policyScope === 'workspace'}
                      aria-disabled={!workspacePath}
                      aria-describedby={!workspacePath
                        ? 'external-policy-workspace-unavailable'
                        : undefined}
                      onClick={() => {
                        if (workspacePath) setPolicyScope('workspace');
                      }}
                    >
                      <FolderKanban size={14} aria-hidden="true" />
                      {t('policy.scope.workspace')}
                    </button>
                  </Tooltip>
                  {!workspacePath ? (
                    <span id="external-policy-workspace-unavailable" className="sr-only">
                      {t('policy.scope.workspaceUnavailable')}
                    </span>
                  ) : null}
                  {workspacePolicyInherited ? (
                    <span className="bitfun-external-sources-config__inherited-badge">
                      {t('policy.inherited')}
                    </span>
                  ) : policyScope === 'workspace' ? (
                    <span className="bitfun-external-sources-config__override-badge">
                      {t('policy.projectOverride')}
                    </span>
                  ) : null}
                  {policyScope === 'workspace' && policy.workspaceOverride ? (
                    <Button
                      variant="secondary"
                      size="small"
                      disabled={busyKey !== null || !policyCompatible
                        || !hostCapabilities.canMutatePolicy}
                      onClick={() => void updatePolicy({ operation: 'reset_workspace' })}
                    >
                      {t('policy.resetWorkspace')}
                    </Button>
                  ) : null}
                </div>

                {ecosystemPolicies.map((ecosystem) => {
                  const isOpencode = ecosystem.descriptor.ecosystemId === 'opencode'
                    || ecosystem.descriptor.displayName === 'OpenCode';
                  if (isOpencode) {
                    return (
                      <React.Fragment key={ecosystem.ecosystemId}>
                        <div className="bitfun-external-sources-config__opencode-card">
                          <div className="bitfun-external-sources-config__opencode-summary">
                            <div>
                              <strong>{t('opencode.title')}</strong>
                              <span> · {t('opencode.summary', {
                                agents: opencodeGroups.reduce((sum, g) => sum + g.counts.agents, 0),
                                commands: opencodeGroups.reduce((sum, g) => sum + g.counts.commands, 0),
                                mcps: opencodeGroups.reduce((sum, g) => sum + g.counts.mcps, 0),
                              })}</span>
                            </div>
                            <div className="bitfun-external-sources-config__policy-actions">
                              <Select
                                size="small"
                                value={ecosystem.mode}
                                triggerAriaLabel={t('policy.modeLabel', {
                                  ecosystem: ecosystem.descriptor.displayName,
                                })}
                                disabled={!policyCompatible || !hostCapabilities.canMutatePolicy
                                  || !selectedPolicyEnabled || busyKey !== null}
                                options={[
                                  { value: 'recommended', label: t('policy.mode.recommended') },
                                  { value: 'discover_only', label: t('policy.mode.discoverOnly') },
                                  { value: 'disabled', label: t('policy.mode.disabled') },
                                  ...(ecosystem.mode === 'custom'
                                    ? [{ value: 'custom', label: t('policy.mode.custom'), disabled: true }]
                                    : []),
                                  ...(!KNOWN_INTEGRATION_MODES.has(ecosystem.mode)
                                    ? [{
                                        value: ecosystem.mode,
                                        label: t('policy.unsupportedSafelyOff'),
                                        disabled: true,
                                      }]
                                    : []),
                                ]}
                                onChange={(value) => void updatePolicy({
                                  operation: 'set_ecosystem_mode',
                                  ecosystemId: ecosystem.ecosystemId,
                                  mode: String(Array.isArray(value) ? value[0] : value) as ExternalIntegrationMode,
                                })}
                              />
                              <Tooltip content={t('policy.capabilitiesHint')} placement="top">
                                <button
                                  type="button"
                                  className="bitfun-external-sources-config__icon-action"
                                  aria-label={t('policy.capabilitiesFor', {
                                    ecosystem: ecosystem.descriptor.displayName,
                                  })}
                                  aria-expanded={expandedEcosystems.has(ecosystem.ecosystemId)}
                                  aria-controls={`external-capabilities-${ecosystem.ecosystemId}`}
                                  onClick={() => setExpandedEcosystems((current) => {
                                    const next = new Set(current);
                                    if (next.has(ecosystem.ecosystemId)) next.delete(ecosystem.ecosystemId);
                                    else next.add(ecosystem.ecosystemId);
                                    return next;
                                  })}
                                >
                                  <Settings2 size={16} aria-hidden="true" />
                                </button>
                              </Tooltip>
                            </div>
                          </div>
                          {expandedEcosystems.has(ecosystem.ecosystemId) ? (
                            <div
                              id={`external-capabilities-${ecosystem.ecosystemId}`}
                              className="bitfun-external-sources-config__capability-grid"
                            >
                              {ecosystem.descriptor.capabilities.map((capabilityDescriptor) => {
                                const capabilityId = capabilityDescriptor.capabilityId;
                                const limited = ecosystem.effective?.policyLimitedCapabilities
                                  ?.includes(capabilityId);
                                const configuredAccess = ecosystem.capabilityOverrides[capabilityId];
                                const accessKnown = configuredAccess === undefined
                                  || KNOWN_INTEGRATION_ACCESS.has(configuredAccess);
                                const countKey = capabilityId === 'subagent' ? 'agents'
                                  : capabilityId === 'command' ? 'commands'
                                  : capabilityId;
                                const count = opencodeGroups.reduce(
                                  (sum, g) => sum + (g.counts[countKey as keyof typeof g.counts] ?? 0),
                                  0,
                                );
                                const riskKey = `opencode.capability.${capabilityId}.risk`;
                                const riskText = t(riskKey);
                                return (
                                  <div
                                    className="bitfun-external-sources-config__capability-row"
                                    key={capabilityId}
                                  >
                                    <div>
                                      <span>{t(`policy.capability.${capabilityId}`)}</span>
                                      {limited ? (
                                        <span className="bitfun-external-sources-config__limited-badge">
                                          {t('policy.safetyLimited')}
                                        </span>
                                      ) : null}
                                      <span className="bitfun-external-sources-config__candidate-detail">
                                        {t(`opencode.capability.${capabilityId}.description`, {
                                          count,
                                          scope: t('scope.user_global'),
                                        })}
                                      </span>
                                      <span className="bitfun-external-sources-config__candidate-detail">
                                        {t(`opencode.capability.${capabilityId}.effect`)}
                                      </span>
                                      {riskText && riskText !== riskKey ? (
                                        <span className="bitfun-external-sources-config__tool-warning">
                                          {riskText}
                                        </span>
                                      ) : null}
                                    </div>
                                    <Select
                                      size="small"
                                      value={accessKnown
                                        ? selectedCapabilityAccess(ecosystem, capabilityId)
                                        : configuredAccess}
                                      triggerAriaLabel={t('policy.capabilityAccessLabel', {
                                        ecosystem: ecosystem.descriptor.displayName,
                                        capability: t(`policy.capability.${capabilityId}`),
                                      })}
                                      disabled={!policyCompatible || !hostCapabilities.canMutatePolicy
                                        || !selectedPolicyEnabled || busyKey !== null}
                                      options={[
                                        { value: 'disabled', label: t('policy.access.disabled') },
                                        { value: 'discover_only', label: t('policy.access.discoverOnly') },
                                        { value: 'ask_before_use', label: t('policy.access.askBeforeUse') },
                                        ...(capabilityDescriptor.safetyCeiling === 'auto'
                                          ? [{ value: 'auto', label: t('policy.access.auto') }]
                                          : []),
                                        ...(!accessKnown && configuredAccess
                                          ? [{
                                              value: configuredAccess,
                                              label: t('policy.unsupportedSafelyOff'),
                                              disabled: true,
                                            }]
                                          : []),
                                      ]}
                                      onChange={(value) => {
                                        const access = String(
                                          Array.isArray(value) ? value[0] : value,
                                        ) as ExternalIntegrationAccess;
                                        void updateCapabilityAccess(
                                          ecosystem.ecosystemId,
                                          capabilityId,
                                          access,
                                        );
                                      }}
                                    />
                                  </div>
                                );
                              })}
                              {opencodeGroups.length > 0 ? (
                                <div className="bitfun-external-sources-config__opencode-locations">
                                  <span>{t('opencode.configLocations')}</span>
                                  {Array.from(new Set(opencodeGroups.map((g) => g.location))).map((location) => (
                                    <span key={location}>{renderPathLink(location)}</span>
                                  ))}
                                </div>
                              ) : null}
                            </div>
                          ) : null}
                        </div>
                      </React.Fragment>
                    );
                  }
                  return (
                  <React.Fragment key={ecosystem.ecosystemId}>
                <div className="bitfun-external-sources-config__ecosystem-card">
                  <div className="bitfun-external-sources-config__ecosystem-heading">
                    <div>
                      <div className="bitfun-external-sources-config__ecosystem-name">
                        {ecosystem.descriptor.displayName}
                        <span className={`bitfun-external-sources-config__ecosystem-state is-${ecosystem.state}`}>
                          {ecosystem.state === 'checking' ? <CircleDashed size={13} aria-hidden="true" /> : null}
                          {ecosystem.state === 'attention' ? <AlertTriangle size={13} aria-hidden="true" /> : null}
                          {ecosystem.state === 'ready' ? <CheckCircle2 size={13} aria-hidden="true" /> : null}
                          {ecosystem.state === 'noConfig' ? <MinusCircle size={13} aria-hidden="true" /> : null}
                          {t(`policy.state.${ecosystem.state}`)}
                        </span>
                      </div>
                    </div>
                  </div>
                  <div className="bitfun-external-sources-config__policy-actions">
                    <Select
                      size="small"
                      value={ecosystem.mode}
                      triggerAriaLabel={t('policy.modeLabel', {
                        ecosystem: ecosystem.descriptor.displayName,
                      })}
                      disabled={!policyCompatible || !hostCapabilities.canMutatePolicy
                        || !selectedPolicyEnabled || busyKey !== null}
                      options={[
                        { value: 'recommended', label: t('policy.mode.recommended') },
                        { value: 'discover_only', label: t('policy.mode.discoverOnly') },
                        { value: 'disabled', label: t('policy.mode.disabled') },
                        ...(ecosystem.mode === 'custom'
                          ? [{ value: 'custom', label: t('policy.mode.custom'), disabled: true }]
                          : []),
                        ...(!KNOWN_INTEGRATION_MODES.has(ecosystem.mode)
                          ? [{
                              value: ecosystem.mode,
                              label: t('policy.unsupportedSafelyOff'),
                              disabled: true,
                            }]
                          : []),
                      ]}
                      onChange={(value) => void updatePolicy({
                        operation: 'set_ecosystem_mode',
                        ecosystemId: ecosystem.ecosystemId,
                        mode: String(Array.isArray(value) ? value[0] : value) as ExternalIntegrationMode,
                      })}
                    />
                    <Tooltip content={t('policy.capabilitiesHint')} placement="top">
                      <button
                        type="button"
                        className="bitfun-external-sources-config__icon-action"
                        aria-label={t('policy.capabilitiesFor', {
                          ecosystem: ecosystem.descriptor.displayName,
                        })}
                        aria-expanded={expandedEcosystems.has(ecosystem.ecosystemId)}
                        aria-controls={`external-capabilities-${ecosystem.ecosystemId}`}
                        onClick={() => setExpandedEcosystems((current) => {
                          const next = new Set(current);
                          if (next.has(ecosystem.ecosystemId)) next.delete(ecosystem.ecosystemId);
                          else next.add(ecosystem.ecosystemId);
                          return next;
                        })}
                      >
                        <Settings2 size={16} aria-hidden="true" />
                      </button>
                    </Tooltip>
                  </div>
                </div>

                {expandedEcosystems.has(ecosystem.ecosystemId) ? (
                  <div
                    id={`external-capabilities-${ecosystem.ecosystemId}`}
                    className="bitfun-external-sources-config__capability-grid"
                  >
                    {ecosystem.descriptor.capabilities.map((capabilityDescriptor) => {
                      const capabilityId = capabilityDescriptor.capabilityId;
                      const limited = ecosystem.effective?.policyLimitedCapabilities
                        ?.includes(capabilityId);
                      const configuredAccess = ecosystem.capabilityOverrides[capabilityId];
                      const accessKnown = configuredAccess === undefined
                        || KNOWN_INTEGRATION_ACCESS.has(configuredAccess);
                      return (
                        <div
                          className="bitfun-external-sources-config__capability-row"
                          key={capabilityId}
                        >
                          <div>
                            <span>{t(`policy.capability.${capabilityId}`)}</span>
                            {limited ? (
                              <span className="bitfun-external-sources-config__limited-badge">
                                {t('policy.safetyLimited')}
                              </span>
                            ) : null}
                          </div>
                          <Select
                            size="small"
                            value={accessKnown
                              ? selectedCapabilityAccess(ecosystem, capabilityId)
                              : configuredAccess}
                            triggerAriaLabel={t('policy.capabilityAccessLabel', {
                              ecosystem: ecosystem.descriptor.displayName,
                              capability: t(`policy.capability.${capabilityId}`),
                            })}
                            disabled={!policyCompatible || !hostCapabilities.canMutatePolicy
                              || !selectedPolicyEnabled || busyKey !== null}
                            options={[
                              { value: 'disabled', label: t('policy.access.disabled') },
                              { value: 'discover_only', label: t('policy.access.discoverOnly') },
                              { value: 'ask_before_use', label: t('policy.access.askBeforeUse') },
                              ...(capabilityDescriptor.safetyCeiling === 'auto'
                                ? [{ value: 'auto', label: t('policy.access.auto') }]
                                : []),
                              ...(!accessKnown && configuredAccess
                                ? [{
                                    value: configuredAccess,
                                    label: t('policy.unsupportedSafelyOff'),
                                    disabled: true,
                                  }]
                                : []),
                            ]}
                            onChange={(value) => {
                              const access = String(
                                Array.isArray(value) ? value[0] : value,
                              ) as ExternalIntegrationAccess;
                              void updateCapabilityAccess(
                                ecosystem.ecosystemId,
                                capabilityId,
                                access,
                              );
                            }}
                          />
                        </div>
                      );
                    })}
                  </div>
                ) : null}
                  </React.Fragment>
                  );
                })}
              </ConfigPageSection>
            ) : null}
            {operationStatus ? (
              <div
                className="bitfun-external-sources-config__notice"
                role="status"
                aria-live="polite"
              >
                {operationStatus}
              </div>
            ) : null}
            {agentChangeNotice ? (
              <div
                className="bitfun-external-sources-config__notice"
                role="status"
                aria-live="polite"
              >
                {agentChangeNotice.message}
              </div>
            ) : null}
            {catalogDiagnostics.length > 0 ? (
              <details
                className="bitfun-external-sources-config__notice"
                data-external-attention={catalogDiagnostics
                  .some((diagnostic) => diagnostic.severity !== 'info') ? 'true' : undefined}
              >
                <summary>
                  {t('diagnostics.summary', { count: catalogDiagnostics.length })}
                </summary>
                <ul className="bitfun-external-sources-config__diagnostics">
                  {catalogDiagnostics.map((diagnostic) => (
                    <li key={externalSourceDiagnosticKey(diagnostic)}>
                      <span>{t(`diagnostics.category.${sourceDiagnosticCategory(diagnostic.code)}`)}</span>
                      <details>
                        <summary>{t('common.technicalDetails')}</summary>
                        <code>{diagnostic.code}</code>
                        <div>{diagnostic.message}</div>
                      </details>
                    </li>
                  ))}
                </ul>
              </details>
            ) : null}
            {snapshot?.discoveryPending ? (
              <div className="bitfun-external-sources-config__notice" role="status">
                {t('checkingNonBlocking')}
              </div>
            ) : null}

            {(snapshot?.mcpApprovalRequests?.length ?? 0) > 0 ? (
              <ConfigPageSection
                title={t('mcpApprovals.title')}
              >
                {snapshot?.mcpApprovalRequests?.map((request) => {
                  const source = snapshot.sources.find((candidate) => (
                    candidate.record.key.providerId === request.definition.id.source.providerId
                    && candidate.record.key.sourceId === request.definition.id.source.sourceId
                  ));
                  return (
                    <div
                      className="bitfun-external-sources-config__tool-card"
                      key={request.decisionKey}
                    >
                    <div className="bitfun-external-sources-config__conflict-title">
                      {request.definition.name}
                    </div>
                    <div className="bitfun-external-sources-config__tool-detail">
                      <span>{t('mcp.source', {
                        source: source?.record.displayName ?? t('mcp.externalSource'),
                      })}</span>
                      {source ? (
                        <span>{t('mcp.sourceLocation', {
                          location: source.record.location,
                        })}</span>
                      ) : null}
                      {source ? (
                        <span>{t('mcp.scope', {
                          scope: source.record.scope === 'workspace_local'
                            ? t('shared:features.workspace')
                            : t(`scope.${source.record.scope}`),
                        })}</span>
                      ) : null}
                      <span>{t(`mcp.transport.${request.definition.transport}`)}</span>
                      {request.definition.commandPreview ? (
                        <span>{t('mcp.command', { command: request.definition.commandPreview })}</span>
                      ) : null}
                      {request.definition.remoteUrlPreview ? (
                        <span>{t('mcp.url', { url: request.definition.remoteUrlPreview })}</span>
                      ) : null}
                      {request.definition.workingDirectory ? (
                        <span>{t('mcp.workingDirectory', {
                          location: request.definition.workingDirectory,
                        })}</span>
                      ) : null}
                      <span>{t('mcp.argumentCount', {
                        count: request.definition.argumentCount,
                      })}</span>
                      <span>{t('mcp.environmentCount', {
                        count: request.definition.environmentKeys.length,
                      })}</span>
                      {request.definition.environmentKeys.length > 0 ? (
                        <span>{t('mcp.environmentNames', {
                          names: request.definition.environmentKeys.join(', '),
                        })}</span>
                      ) : null}
                      {(request.definition.environmentReferenceNames?.length ?? 0) > 0 ? (
                        <span>{t('mcp.environmentReads', {
                          names: (request.definition.environmentReferenceNames ?? []).join(', '),
                        })}</span>
                      ) : null}
                      <span>{t('mcp.headerCount', {
                        count: request.definition.headerNames.length,
                      })}</span>
                      {request.definition.headerNames.length > 0 ? (
                        <span>{t('mcp.headerNames', {
                          names: request.definition.headerNames.join(', '),
                        })}</span>
                      ) : null}
                    </div>
                    <div className="bitfun-external-sources-config__tool-warning">
                      {t('mcpApprovals.warning')}
                    </div>
                    <div className="bitfun-external-sources-config__tool-actions">
                      <Button
                        variant="secondary"
                        size="small"
                        disabled={!policyCompatible || busyKey !== null
                          || !hostCapabilities.canApproveRuntime}
                        onClick={() => void decideMcpServer(
                          request.candidateId,
                          request.decisionKey,
                          false,
                        )}
                      >
                        {t('mcpApprovals.keepDisabled')}
                      </Button>
                      <Button
                        variant="primary"
                        size="small"
                        disabled={!policyCompatible || busyKey !== null
                          || !hostCapabilities.canApproveRuntime}
                        onClick={() => void decideMcpServer(
                          request.candidateId,
                          request.decisionKey,
                          true,
                        )}
                      >
                        {t('mcpApprovals.enable')}
                      </Button>
                    </div>
                    </div>
                  );
                })}
              </ConfigPageSection>
            ) : null}

            {(snapshot?.mcpServers?.length ?? 0) > 0 ? (
              <ConfigPageSection title={t('mcp.title')}>
                {snapshot?.mcpServers?.map((server) => {
                  const state = server.activationState.state;
                  const reviewing = reviewingMcpKey === server.candidateId;
                  const canEnable = state === 'declined' || state === 'configuration_changed';
                  const canDisable = ['starting', 'active', 'runtime_unavailable'].includes(state);
                  const source = snapshot.sources.find((candidate) => (
                    candidate.record.key.providerId === server.definition.id.source.providerId
                    && candidate.record.key.sourceId === server.definition.id.source.sourceId
                  ));
                  return (
                    <React.Fragment key={server.candidateId}>
                      <ConfigPageRow
                        label={server.definition.name}
                        description={`${t(`mcp.transport.${server.definition.transport}`)} · ${t('mcp.externalSource')}`}
                        align="center"
                      >
                        <div className="bitfun-external-sources-config__source-control">
                          <span
                            className={`bitfun-external-sources-config__state is-${state}`}
                            data-external-attention={state === 'approval_required' ? 'true' : undefined}
                          >
                            {t(`mcpState.${state}`)}
                          </span>
                          <Button
                            variant="secondary"
                            size="small"
                            aria-expanded={reviewing}
                            onClick={() => setReviewingMcpKey(reviewing ? null : server.candidateId)}
                          >
                            {reviewing ? t('common.hideDetails') : t('common.details')}
                          </Button>
                          {canDisable ? (
                            <Button
                              variant="secondary"
                              size="small"
                              disabled={!policyCompatible || busyKey !== null
                                || !hostCapabilities.canApproveRuntime}
                              onClick={() => void decideMcpServer(
                                server.candidateId,
                                server.decisionKey,
                                false,
                              )}
                            >
                              {t('mcp.disable')}
                            </Button>
                          ) : null}
                        </div>
                      </ConfigPageRow>
                      {reviewing ? (
                        <div className="bitfun-external-sources-config__tool-card">
                          <div className="bitfun-external-sources-config__tool-detail">
                            <span>{t('mcp.source', {
                              source: source?.record.displayName ?? t('mcp.externalSource'),
                            })}</span>
                            {source ? (
                              <>
                                <span>{t('mcp.sourceLocation')}: {renderPathLink(source.record.location)}</span>
                                <span>{t('mcp.scope', {
                                  scope: t(`scope.${source.record.scope}`),
                                })}</span>
                              </>
                            ) : null}
                            {server.definition.commandPreview ? (
                              <span>{t('mcp.command', { command: server.definition.commandPreview })}</span>
                            ) : null}
                            {server.definition.remoteUrlPreview ? (
                              <span>{t('mcp.url', { url: server.definition.remoteUrlPreview })}</span>
                            ) : null}
                            {server.definition.workingDirectory ? (
                              <span>{t('mcp.workingDirectory', {
                                location: server.definition.workingDirectory,
                              })}</span>
                            ) : null}
                            <span>{t('mcp.argumentCount', {
                              count: server.definition.argumentCount,
                            })}</span>
                            {(server.definition.environmentReferenceNames?.length ?? 0) > 0 ? (
                              <span>{t('mcp.environmentReads', {
                                names: (server.definition.environmentReferenceNames ?? []).join(', '),
                              })}</span>
                            ) : null}
                            {'reason' in server.activationState ? (
                              <span>{t(server.activationState.state === 'runtime_unavailable'
                                ? 'mcp.runtimeUnavailableGuidance'
                                : 'mcp.unsupportedGuidance')}</span>
                            ) : null}
                            <span>{t('mcp.changePolicy')}</span>
                          </div>
                          {canEnable ? (
                            <div className="bitfun-external-sources-config__tool-actions">
                              <Button
                                variant="primary"
                                size="small"
                                disabled={!policyCompatible || busyKey !== null
                                  || !hostCapabilities.canApproveRuntime}
                                onClick={() => void decideMcpServer(
                                  server.candidateId,
                                  server.decisionKey,
                                  true,
                                )}
                              >
                                {t('mcp.enable')}
                              </Button>
                            </div>
                          ) : null}
                        </div>
                      ) : null}
                    </React.Fragment>
                  );
                })}
              </ConfigPageSection>
            ) : null}

            {mcpConflicts.length > 0 ? (
              <ConfigPageSection
                title={t('mcpConflicts.title')}
              >
                {mcpConflicts.map((conflict) => (
                  <div
                    className="bitfun-external-sources-config__conflict"
                    key={conflict.conflictKey}
                    data-external-attention={!conflict.selectedCandidateId ? 'true' : undefined}
                  >
                    <div className="bitfun-external-sources-config__conflict-title">
                      {t('mcpConflicts.serverName', { name: conflict.serverName })}
                    </div>
                    <div className="bitfun-external-sources-config__conflict-options">
                      {conflict.candidates.map((candidate) => {
                        const selected = conflict.selectedCandidateId === candidate.candidateId;
                        const externalServer = candidate.external
                          ? snapshot?.mcpServers?.find((server) => (
                            server.candidateId === candidate.candidateId
                          ))
                          : undefined;
                        const externalSource = externalServer
                          ? snapshot?.sources?.find((source) => (
                            source.record.key.providerId
                              === externalServer.definition.id.source.providerId
                            && source.record.key.sourceId
                              === externalServer.definition.id.source.sourceId
                          ))
                          : undefined;
                        const conflictReviewKey = `${conflict.conflictKey}:${candidate.candidateId}`;
                        const reviewingExternal = reviewingMcpConflictKey === conflictReviewKey;
                        const detailId = `mcp-conflict-detail-${candidate.candidateId.replace(/[^a-zA-Z0-9_-]/g, '-')}`;
                        return (
                          <div className="bitfun-external-sources-config__candidate" key={candidate.candidateId}>
                            <Button
                              variant={selected ? 'primary' : 'secondary'}
                              size="small"
                              disabled={!policyCompatible || busyKey !== null || !candidate.available
                                || !hostCapabilities.canApproveRuntime}
                              aria-pressed={selected}
                              aria-expanded={candidate.external ? reviewingExternal : undefined}
                              aria-controls={candidate.external ? detailId : undefined}
                              onClick={() => {
                                if (candidate.external) {
                                  setReviewingMcpConflictKey(
                                    reviewingExternal ? null : conflictReviewKey,
                                  );
                                } else {
                                  void chooseMcpConflict(
                                    conflict.conflictKey,
                                    candidate.candidateId,
                                    false,
                                  );
                                }
                              }}
                            >
                              {candidate.external
                                ? reviewingExternal
                                  ? t('common.hideDetails')
                                  : t('mcpConflicts.review', { name: candidate.displayName })
                                : candidate.displayName}
                            </Button>
                            <span className="bitfun-external-sources-config__candidate-state">
                              {!candidate.available
                                ? t(candidate.external
                                  ? 'mcpConflicts.unavailable'
                                  : 'mcpConflicts.nativeDisabled')
                                : selected
                                  ? t('common.selected')
                                  : t('common.availableChoice')}
                            </span>
                            {!candidate.available && candidate.unavailableReason ? (
                              <span className="bitfun-external-sources-config__candidate-state">
                                {candidate.unavailableReason}
                              </span>
                            ) : null}
                            {externalServer && (reviewingExternal || selected) ? (
                              <div
                                className="bitfun-external-sources-config__tool-detail"
                                id={detailId}
                              >
                                <span>{t('mcp.source', {
                                  source: externalSource?.record.displayName
                                    ?? t('mcp.externalSource'),
                                })}</span>
                                {externalSource ? (
                                  <>
                                    <span>{t('mcp.sourceLocation', {
                                      location: externalSource.record.location,
                                    })}</span>
                                    <span>{t('mcp.scope', {
                                      scope: t(`scope.${externalSource.record.scope}`),
                                    })}</span>
                                  </>
                                ) : null}
                                <span>{t(`mcp.transport.${externalServer.definition.transport}`)}</span>
                                {externalServer.definition.commandPreview ? (
                                  <span>{t('mcp.command', {
                                    command: externalServer.definition.commandPreview,
                                  })}</span>
                                ) : null}
                                {externalServer.definition.remoteUrlPreview ? (
                                  <span>{t('mcp.url', {
                                    url: externalServer.definition.remoteUrlPreview,
                                  })}</span>
                                ) : null}
                                <span>{t('mcp.argumentCount', {
                                  count: externalServer.definition.argumentCount,
                                })}</span>
                                {externalServer.definition.workingDirectory ? (
                                  <span>{t('mcp.workingDirectory', {
                                    location: externalServer.definition.workingDirectory,
                                  })}</span>
                                ) : null}
                                {(externalServer.definition.environmentKeys?.length ?? 0) > 0 ? (
                                  <span>{t('mcp.environmentNames', {
                                    names: externalServer.definition.environmentKeys.join(', '),
                                  })}</span>
                                ) : null}
                                {(externalServer.definition.environmentReferenceNames?.length ?? 0) > 0 ? (
                                  <span>{t('mcp.environmentReads', {
                                    names: (externalServer.definition.environmentReferenceNames ?? []).join(', '),
                                  })}</span>
                                ) : null}
                                {(externalServer.definition.headerNames?.length ?? 0) > 0 ? (
                                  <span>{t('mcp.headerNames', {
                                    names: externalServer.definition.headerNames.join(', '),
                                  })}</span>
                                ) : null}
                                <span className="bitfun-external-sources-config__tool-warning">
                                  {t('mcpApprovals.warning')}
                                </span>
                                {reviewingExternal && !selected && candidate.available ? (
                                  <div className="bitfun-external-sources-config__tool-actions">
                                    <Button
                                      variant="primary"
                                      size="small"
                                      disabled={!policyCompatible || busyKey !== null
                                        || !hostCapabilities.canApproveRuntime}
                                      aria-describedby={detailId}
                                      onClick={() => void chooseMcpConflict(
                                        conflict.conflictKey,
                                        candidate.candidateId,
                                        true,
                                      ).then((accepted) => {
                                        if (accepted) setReviewingMcpConflictKey(null);
                                      })}
                                    >
                                      {t('mcpConflicts.approveAndUse', {
                                        name: candidate.displayName,
                                      })}
                                    </Button>
                                  </div>
                                ) : null}
                              </div>
                            ) : null}
                          </div>
                        );
                      })}
                    </div>
                    <div className="bitfun-external-sources-config__conflict-hint">
                      {conflict.selectedCandidateId
                        ? t('mcpConflicts.currentSelection')
                        : t('mcpConflicts.pending')}
                    </div>
                  </div>
                ))}
              </ConfigPageSection>
            ) : null}

            {(snapshot?.mcpServers?.length ?? 0) === 0
              && (snapshot?.mcpApprovalRequests?.length ?? 0) === 0
              && mcpConflicts.length === 0
              && !snapshot?.discoveryPending ? (
              <ConfigPageSection title={t('mcp.title')}>
                <div className="bitfun-external-sources-config__mcp-empty">
                  <span>{t('mcp.empty')}</span>
                  <span>{t('mcp.emptyGuidance')}</span>
                  <span>· {t('mcp.emptyLocation.userGlobal')}</span>
                  <span>· {t('mcp.emptyLocation.project')}</span>
                  <span>· {t('mcp.emptyLocation.envConfig')}</span>
                  <code>{t('mcp.emptyExample')}</code>
                </div>
              </ConfigPageSection>
            ) : null}

            {(snapshot?.subagents?.length ?? 0) > 0 ? (
              <ConfigPageSection title={t('agents.title')}>
                {snapshot?.subagents?.map((agent) => {
                  const reviewing = reviewingAgentKey === agent.candidateId;
                  const state = agent.activationState.state;
                  const canEnable = state === 'approval_required' || state === 'declined';
                  const canDisable = state === 'active';
                  return (
                    <React.Fragment key={agent.candidateId}>
                      <ConfigPageRow
                        label={agent.displayName}
                        description={`${agent.providerLabel} · ${agent.logicalId} · ${externalAgentModelLabel(agent.effectiveModelLabel, t)} · ${t('agents.singleRun')}`}
                        align="center"
                      >
                        <div className="bitfun-external-sources-config__source-control">
                          <span
                            className={`bitfun-external-sources-config__state is-${state}`}
                            data-external-attention={state === 'approval_required' ? 'true' : undefined}
                          >
                            {t(`agentState.${state}`)}
                          </span>
                          <Button
                            variant="secondary"
                            size="small"
                            aria-expanded={reviewing}
                            onClick={() => setReviewingAgentKey(reviewing ? null : agent.candidateId)}
                          >
                            {reviewing ? t('common.hideDetails') : t('common.details')}
                          </Button>
                          {canDisable ? (
                            <Button
                              variant="secondary"
                              size="small"
                              disabled={!policyCompatible || busyKey !== null
                                || !hostCapabilities.canApproveRuntime}
                              onClick={() => void decideAgent(agent.candidateId, agent.decisionKey, false)}
                            >
                              {t('agents.disable')}
                            </Button>
                          ) : null}
                        </div>
                      </ConfigPageRow>
                      {reviewing ? (
                        <div className="bitfun-external-sources-config__tool-card">
                          <div className="bitfun-external-sources-config__conflict-title">
                            {t('agents.reviewTitle', { name: agent.displayName })}
                          </div>
                          <div className="bitfun-external-sources-config__tool-detail">
                            <span>{agent.description || t('agents.noDescription')}</span>
                            <span>{t('agents.model', { model: externalAgentModelLabel(agent.effectiveModelLabel, t) })}</span>
                            <span>{t('agents.tools', { tools: agent.effectiveToolLabels.join(', ') || t('agents.noTools') })}</span>
                            <span>{t('agents.executionDomain')}</span>
                            <span>{t('agents.compatibility', { state: t(`agentCompatibility.${agent.compatibilityState}`) })}</span>
                            {agent.sourceLocationLabels.length > 0 ? (
                              <details className="bitfun-external-sources-config__source-detail-toggle">
                                <summary>{t('agents.sourceLocations', { count: agent.sourceLocationLabels.length })}</summary>
                                <div className="bitfun-external-sources-config__tool-detail">
                                  {agent.sourceLocationLabels.map((location) => (
                                    <span key={location}>{renderPathLink(location)}</span>
                                  ))}
                                </div>
                              </details>
                            ) : null}
                            {agent.diagnostics.map((diagnostic) => {
                                const category = agentDiagnosticCategory(
                                  diagnostic.code,
                                  diagnostic.blocksActivation,
                                );
                              return (
                                <div key={diagnostic.code}>
                                  <span>{t(`agentDiagnostics.${category}.reason`)}</span>
                                  <span>{t(`agentDiagnostics.${category}.nextStep`)}</span>
                                </div>
                              );
                            })}
                          </div>
                          {canEnable ? (
                            <div className="bitfun-external-sources-config__tool-warning">
                              {t('agents.approvalWarning')}
                            </div>
                          ) : null}
                          <div className="bitfun-external-sources-config__tool-actions">
                            <Button
                              variant="secondary"
                              size="small"
                              onClick={() => setReviewingAgentKey(null)}
                            >
                              {t('common.close')}
                            </Button>
                            {canEnable ? (
                              <Button
                                variant="primary"
                                size="small"
                                disabled={!policyCompatible || busyKey !== null
                                  || !hostCapabilities.canApproveRuntime}
                                onClick={() => void decideAgent(
                                  agent.candidateId,
                                  agent.decisionKey,
                                  true,
                                ).then((applied) => {
                                  if (applied) setReviewingAgentKey(null);
                                })}
                              >
                                {t('agents.enable')}
                              </Button>
                            ) : null}
                          </div>
                        </div>
                      ) : null}
                    </React.Fragment>
                  );
                })}
              </ConfigPageSection>
            ) : null}

            {agentConflicts.length > 0 ? (
              <ConfigPageSection
                title={t('agentConflicts.title')}
              >
                {agentConflicts.map((conflict) => {
                  const selectedExternalAgent = snapshot?.subagents?.find((agent) => (
                    agent.candidateId === conflict.selectedCandidateId
                  ));
                  const selectedChoiceUnavailable = Boolean(
                    selectedExternalAgent
                    && selectedExternalAgent.activationState.state !== 'active',
                  );
                  return (
                    <div
                      className="bitfun-external-sources-config__conflict"
                      key={conflict.conflictKey}
                      data-external-attention={!conflict.selectedCandidateId ? 'true' : undefined}
                    >
                    <div className="bitfun-external-sources-config__conflict-title">
                      {t('agentConflicts.agentName', { name: conflict.logicalId })}
                    </div>
                    <div className="bitfun-external-sources-config__conflict-options">
                      {conflict.candidates.map((candidate) => {
                        const selected = conflict.selectedCandidateId === candidate.candidateId;
                        const externalAgent = candidate.external
                          ? snapshot?.subagents?.find((agent) => (
                            agent.candidateId === candidate.candidateId
                          ))
                          : undefined;
                        return (
                          <div className="bitfun-external-sources-config__candidate" key={candidate.candidateId}>
                            <Button
                              variant={selected ? 'primary' : 'secondary'}
                              size="small"
                              disabled={!policyCompatible || busyKey !== null
                                || !hostCapabilities.canApproveRuntime}
                              aria-pressed={selected}
                              onClick={() => void chooseAgentConflict(
                                conflict.conflictKey,
                                candidate.candidateId,
                                candidate.external,
                              )}
                            >
                              {candidate.displayName}
                              <span className="bitfun-external-sources-config__ecosystem">
                                {candidate.sourceLabel}
                              </span>
                            </Button>
                            <span className="bitfun-external-sources-config__candidate-state">
                              {t(selected
                                ? selectedChoiceUnavailable
                                  ? 'common.selectedUnavailable'
                                  : 'common.selected'
                                : conflict.selectedCandidateId
                                  ? 'common.notSelected'
                                  : 'common.availableChoice')}
                            </span>
                            {externalAgent ? (
                              <div className="bitfun-external-sources-config__candidate-detail">
                                <span>{t('agents.model', { model: externalAgentModelLabel(externalAgent.effectiveModelLabel, t) })}</span>
                                <span>{t('agents.tools', { tools: externalAgent.effectiveToolLabels.join(', ') || t('agents.noTools') })}</span>
                                <span>{t('agents.executionDomain')}</span>
                                <span>{t('agents.compatibility', { state: t(`agentCompatibility.${externalAgent.compatibilityState}`) })}</span>
                                {externalAgent.sourceLocationLabels.map((location) => (
                                  <span key={location}>{abbreviatedLocation(location)}</span>
                                ))}
                                {externalAgent.diagnostics.map((diagnostic) => {
                                  const category = agentDiagnosticCategory(
                                    diagnostic.code,
                                    diagnostic.blocksActivation,
                                  );
                                  return (
                                    <span key={diagnostic.code}>
                                      {t(`agentDiagnostics.${category}.reason`)}{' '}
                                      {t(`agentDiagnostics.${category}.impact`, {
                                        impact: diagnostic.blocksActivation
                                          ? t('agentDiagnostics.activationBlocked')
                                          : t('agentDiagnostics.degradedOnly'),
                                      })}{' '}
                                      {t(`agentDiagnostics.${category}.nextStep`)}
                                    </span>
                                  );
                                })}
                                <strong>{t('agentConflicts.selectionApproves')}</strong>
                              </div>
                            ) : null}
                          </div>
                        );
                      })}
                      <Button
                        variant={conflict.selectedCandidateId === DISABLED_SUBAGENT_CONFLICT_CHOICE
                          ? 'primary'
                          : 'secondary'}
                        size="small"
                        disabled={!policyCompatible || busyKey !== null
                          || !hostCapabilities.canApproveRuntime}
                        aria-pressed={
                          conflict.selectedCandidateId === DISABLED_SUBAGENT_CONFLICT_CHOICE
                        }
                        onClick={() => void chooseAgentConflict(
                          conflict.conflictKey,
                          DISABLED_SUBAGENT_CONFLICT_CHOICE,
                          false,
                        )}
                      >
                        {t('agentConflicts.disableAll')}
                      </Button>
                    </div>
                    <div className="bitfun-external-sources-config__conflict-hint">
                      {conflict.selectedCandidateId === DISABLED_SUBAGENT_CONFLICT_CHOICE
                        ? t('agentConflicts.keptUnavailable')
                        : conflict.selectedCandidateId
                          ? t(selectedChoiceUnavailable
                            ? 'agentConflicts.currentSelectionUnavailable'
                            : 'agentConflicts.currentSelection')
                          : t('agentConflicts.pending')}
                    </div>
                    </div>
                  );
                })}
              </ConfigPageSection>
            ) : null}

            {(snapshot?.toolApprovalRequests?.length ?? 0) > 0 ? (
              <ConfigPageSection
                title={t('toolApprovals.title')}
              >
                {snapshot?.toolApprovalRequests?.map((request) => {
                  const targetTools = (snapshot.tools ?? []).filter((tool) => (
                    tool.definition.id.target.source.providerId === request.targetId.source.providerId
                    && tool.definition.id.target.source.sourceId === request.targetId.source.sourceId
                    && tool.definition.id.target.localId === request.targetId.localId
                  ));
                  const source = snapshot.sources.find((candidate) => (
                    candidate.record.key.providerId === request.targetId.source.providerId
                    && candidate.record.key.sourceId === request.targetId.source.sourceId
                  ));
                  const modulePaths = Array.from(new Set(
                    targetTools.map((tool) => tool.definition.modulePath),
                  ));
                  return (
                    <div
                      className="bitfun-external-sources-config__tool-card"
                      key={request.decisionKey}
                    >
                      <div className="bitfun-external-sources-config__conflict-title">
                        {request.sourceDisplayName}: {request.toolNames.join(', ')}
                      </div>
                      <div className="bitfun-external-sources-config__tool-detail">
                        <span title={source?.record.location ?? request.sourceLocation}>
                          {t('toolApprovals.sourceRoot', {
                            location: source?.record.location ?? request.sourceLocation,
                          })}
                        </span>
                        {(modulePaths.length > 0 ? modulePaths : [request.sourceLocation]).map((path) => (
                          <span title={path} key={path}>
                            {t('toolApprovals.modulePath', { location: path })}
                          </span>
                        ))}
                        <span>
                          {t('toolApprovals.scope', {
                            scope: (source?.record.scope ?? request.sourceScope) === 'workspace_local'
                              ? t('shared:features.workspace')
                              : t(`scope.${source?.record.scope ?? request.sourceScope}`),
                          })}
                        </span>
                        <span>
                          {t('toolApprovals.executionDomain', {
                            domain: executionLocationLabel(t, source?.record.executionDomainId),
                          })}
                        </span>
                        <span>
                          {t('toolApprovals.runtime', {
                            runtime: t(`runtime.${request.runtimeKind}`),
                          })}
                        </span>
                        <span title={request.workingDirectory}>
                          {t('toolApprovals.workingDirectory', {
                            location: request.workingDirectory,
                          })}
                        </span>
                        <span>
                          {t('toolApprovals.capabilities', {
                            capabilities: request.capabilities
                              .map((capability) => t(`capability.${capability}`))
                              .join(', '),
                          })}
                        </span>
                      </div>
                      <div className="bitfun-external-sources-config__tool-warning">
                        {t('toolApprovals.warning')}
                      </div>
                      <div className="bitfun-external-sources-config__tool-actions">
                        <Button
                          variant="secondary"
                          size="small"
                        disabled={!policyCompatible || busyKey === request.decisionKey
                          || !hostCapabilities.canApproveRuntime}
                          onClick={() => void decideToolTarget(
                            request.approvalKey,
                            request.decisionKey,
                            false,
                          )}
                        >
                          {t('toolApprovals.keepDisabled')}
                        </Button>
                        <Button
                          variant="primary"
                          size="small"
                          disabled={!policyCompatible || busyKey === request.decisionKey
                            || !hostCapabilities.canApproveRuntime}
                          onClick={() => void decideToolTarget(
                            request.approvalKey,
                            request.decisionKey,
                            true,
                          )}
                        >
                          {t('toolApprovals.enable')}
                        </Button>
                      </div>
                    </div>
                  );
                })}
              </ConfigPageSection>
            ) : null}

            {nonOpencodeGroups.length > 0 ? (
              <ConfigPageSection title={t('sources.title')}>
                {nonOpencodeGroups.map((group) => {
                  return (
                    <React.Fragment key={group.key}>
                      <ConfigPageRow
                        className="bitfun-external-sources-config__source-group"
                        label={group.displayName}
                        description={(
                          <div className="bitfun-external-sources-config__source-description">
                            <span className="bitfun-external-sources-config__source-origin">
                              <span
                                className="bitfun-external-sources-config__source-location"
                                title={group.location}
                                translate="no"
                              >
                                {group.location}
                              </span>
                              <span aria-hidden="true">·</span>
                              <span className="bitfun-external-sources-config__source-scopes">
                                {group.scopes.map((scope, index) => (
                                  <React.Fragment key={scope}>
                                    {index > 0 ? <span aria-hidden="true"> + </span> : null}
                                    <span>
                                      {scope === 'workspace_local'
                                        ? t('shared:features.workspace')
                                        : t(`scope.${scope}`)}
                                    </span>
                                  </React.Fragment>
                                ))}
                              </span>
                            </span>
                            {SOURCE_COUNT_LABELS.some(
                              ([capability]) => group.counts[capability] > 0,
                            ) ? (
                              <span className="bitfun-external-sources-config__source-counts">
                                {SOURCE_COUNT_LABELS.map(([capability, label]) => {
                                  const count = group.counts[capability];
                                  return count > 0 ? (
                                    <span
                                      key={capability}
                                      className="bitfun-external-sources-config__source-count"
                                    >
                                      {t(label, { count })}
                                    </span>
                                  ) : null;
                                })}
                              </span>
                            ) : null}
                          </div>
                        )}
                        align="center"
                      >
                        <div
                          className="bitfun-external-sources-config__source-members"
                          role="group"
                          aria-label={t('sources.toggleLabel', { name: group.displayName })}
                        >
                          {group.members.map((member) => {
                            const capabilityLabel = member.capability === 'source'
                              ? group.displayName
                              : t(`policy.capability.${member.capability}`);
                            const scopeLabel = member.scope === 'workspace_local'
                              ? t('shared:features.workspace')
                              : t(`scope.${member.scope}`);
                            return (
                              <Switch
                                key={member.stableKey}
                                className="bitfun-external-sources-config__source-member"
                                size="small"
                                label={capabilityLabel}
                                description={group.scopes.length > 1 ? scopeLabel : undefined}
                                checked={member.enabled}
                                disabled={!policyCompatible
                                  || !member.mutable
                                  || !hostCapabilities.canManageSources}
                                loading={busyKey === member.stableKey}
                                aria-label={t('sources.toggleLabel', {
                                  name: [
                                    group.displayName,
                                    capabilityLabel,
                                    scopeLabel,
                                    t(`lifecycle.${member.lifecycle}`),
                                  ].join(' · '),
                                })}
                                onChange={(event) => void setEnabled(
                                  member.stableKey,
                                  event.currentTarget.checked,
                                )}
                              >
                                {member.lifecycle !== 'available' ? (
                                  <span className={`bitfun-external-sources-config__state is-${member.lifecycle}`}>
                                    {t(`lifecycle.${member.lifecycle}`)}
                                  </span>
                                ) : null}
                              </Switch>
                            );
                          })}
                        </div>
                      </ConfigPageRow>
                      {group.diagnostics.length > 0 ? (
                        <details
                          className="bitfun-external-sources-config__notice"
                          data-external-attention="true"
                        >
                          <summary>
                            {t('diagnostics.sourceSummary', {
                              name: group.displayName,
                              count: group.diagnostics.length,
                            })}
                          </summary>
                          <ul className="bitfun-external-sources-config__diagnostics">
                            {group.diagnostics.map((diagnostic) => (
                              <li key={externalSourceDiagnosticKey(diagnostic)}>
                                <span>{t(`diagnostics.category.${sourceDiagnosticCategory(diagnostic.code)}`)}</span>
                                <details>
                                  <summary>{t('common.technicalDetails')}</summary>
                                  <code>{diagnostic.code}</code>
                                  <div>{diagnostic.message}</div>
                                </details>
                              </li>
                            ))}
                          </ul>
                        </details>
                      ) : null}
                    </React.Fragment>
                  );
                })}
              </ConfigPageSection>
            ) : null}

            {(snapshot?.tools?.length ?? 0) > 0 ? (
              <ConfigPageSection title={t('tools.title')}>
                {snapshot?.tools?.map((tool) => {
                  const toolKey = `${tool.definition.id.target.source.providerId}:${tool.definition.id.target.source.sourceId}:${tool.definition.id.target.localId}:${tool.definition.id.exportId}`;
                  const source = snapshot.sources.find((candidate) => matchesToolSource(candidate, tool));
                  const targetTools = (snapshot.tools ?? []).filter((candidate) => (
                    candidate.definition.id.target.source.providerId
                      === tool.definition.id.target.source.providerId
                    && candidate.definition.id.target.source.sourceId
                      === tool.definition.id.target.source.sourceId
                    && candidate.definition.id.target.localId
                      === tool.definition.id.target.localId
                  ));
                  const firstTargetExport = targetTools[0] === tool;
                  const enableable = ['approval_required', 'disabled'].includes(
                    tool.activation.state,
                  );
                  const disableable = firstTargetExport && targetTools.some((candidate) => (
                    ['active', 'conflict', 'load_failed'].includes(candidate.activation.state)
                  ));
                  const reviewing = reviewingToolKey === toolKey;
                  const reason = t(`toolReason.${tool.activation.state}`);
                  return (
                    <React.Fragment key={toolKey}>
                      <ConfigPageRow
                        label={tool.definition.name}
                        description={tool.definition.descriptionPreview
                          || abbreviatedLocation(tool.definition.modulePath)}
                        align="center"
                      >
                        <div className="bitfun-external-sources-config__source-control">
                          <span
                            className={`bitfun-external-sources-config__state is-${tool.activation.state}`}
                            data-external-attention={tool.activation.state === 'approval_required'
                              ? 'true'
                              : undefined}
                          >
                            {t(`toolState.${tool.activation.state}`)}
                          </span>
                          <Button
                            variant="secondary"
                            size="small"
                            aria-expanded={reviewing}
                            onClick={() => setReviewingToolKey(reviewing ? null : toolKey)}
                          >
                            {reviewing ? t('common.hideDetails') : t('common.details')}
                          </Button>
                          {disableable ? (
                            <Button
                              variant="secondary"
                              size="small"
                              disabled={!policyCompatible || busyKey === tool.decisionKey
                                || !hostCapabilities.canApproveRuntime}
                              onClick={() => void decideToolTarget(
                                tool.approvalKey,
                                tool.decisionKey,
                                false,
                              )}
                            >
                              {t('tools.disable')}
                            </Button>
                          ) : null}
                        </div>
                      </ConfigPageRow>
                      {reviewing ? (
                        <div className="bitfun-external-sources-config__tool-card">
                          <div className="bitfun-external-sources-config__conflict-title">
                            {t('tools.reviewTitle', {
                              name: tool.definition.name,
                              source: source?.record.displayName ?? tool.definition.id.target.source.providerId,
                            })}
                          </div>
                          <div className="bitfun-external-sources-config__tool-detail">
                            <span title={source?.record.location}>
                              {t('toolApprovals.sourceRoot', {
                                location: source?.record.location ?? t('common.unknown'),
                              })}
                            </span>
                            <span title={tool.definition.modulePath}>
                              {t('toolApprovals.modulePath', {
                                location: tool.definition.modulePath,
                              })}
                            </span>
                            <span>
                              {t('toolApprovals.scope', {
                                scope: source?.record.scope === 'workspace_local'
                                  ? t('shared:features.workspace')
                                  : source?.record.scope
                                    ? t(`scope.${source.record.scope}`)
                                    : t('common.unknown'),
                              })}
                            </span>
                            <span>
                              {t('toolApprovals.executionDomain', {
                                domain: executionLocationLabel(t, source?.record.executionDomainId),
                              })}
                            </span>
                            <span>
                              {t('toolApprovals.runtime', {
                                runtime: t(`runtime.${tool.definition.runtimeKind}`),
                              })}
                            </span>
                            <span title={tool.definition.workingDirectory}>
                              {t('toolApprovals.workingDirectory', {
                                location: tool.definition.workingDirectory,
                              })}
                            </span>
                            <span>
                              {t('toolApprovals.capabilities', {
                                capabilities: tool.definition.capabilities
                                  .map((capability) => t(`capability.${capability}`))
                                  .join(', '),
                                })}
                            </span>
                            <span>{t('tools.reason', { reason })}</span>
                            <span>{t('tools.targetScope')}</span>
                            <span>
                              {t('tools.nextStep', {
                                nextStep: t(`toolNextStep.${tool.activation.state}`),
                              })}
                            </span>
                          </div>
                          {enableable ? (
                            <div className="bitfun-external-sources-config__tool-warning">
                              {t('toolApprovals.warning')}
                            </div>
                          ) : null}
                          <div className="bitfun-external-sources-config__tool-actions">
                            <Button
                              variant="secondary"
                              size="small"
                              disabled={!policyCompatible || busyKey === tool.decisionKey
                                || !hostCapabilities.canApproveRuntime}
                              onClick={() => setReviewingToolKey(null)}
                            >
                              {t('common.close')}
                            </Button>
                            {enableable ? (
                              <Button
                                variant="primary"
                                size="small"
                                disabled={!policyCompatible || busyKey === tool.decisionKey
                                  || !hostCapabilities.canApproveRuntime}
                                onClick={() => void decideToolTarget(
                                  tool.approvalKey,
                                  tool.decisionKey,
                                  true,
                                ).then((applied) => {
                                  if (applied) setReviewingToolKey(null);
                                })}
                              >
                                {t('toolApprovals.enable')}
                              </Button>
                            ) : null}
                          </div>
                        </div>
                      ) : null}
                    </React.Fragment>
                  );
                })}
              </ConfigPageSection>
            ) : null}

            {commandConflicts.length > 0 ? (
              <ConfigPageSection
                title={t('conflicts.title')}
              >
                {commandConflicts.map((conflict) => {
                  const selectedChoiceUnavailable = conflict.candidates.some((candidate) => (
                    candidate.candidateId === conflict.selectedCandidateId
                    && candidate.availability.state !== 'available'
                  ));
                  return (
                    <div
                      className="bitfun-external-sources-config__conflict"
                      key={conflict.conflictKey}
                      data-external-attention={!conflict.selectedCandidateId ? 'true' : undefined}
                    >
                    <div className="bitfun-external-sources-config__conflict-title">
                      {t('conflicts.commandName', { name: conflict.commandName })}
                    </div>
                    <div className="bitfun-external-sources-config__conflict-options">
                      {conflict.candidates.map((candidate) => {
                        const selected = conflict.selectedCandidateId === candidate.candidateId;
                        const available = candidate.availability.state === 'available';
                        return (
                          <div
                            className="bitfun-external-sources-config__candidate"
                            key={candidate.candidateId}
                          >
                            <Button
                              variant={selected ? 'primary' : 'secondary'}
                              size="small"
                              disabled={!policyCompatible || busyKey === conflict.conflictKey || !available
                                || !hostCapabilities.canManageSources}
                              aria-pressed={selected}
                              onClick={() => void chooseConflict(
                                conflict.conflictKey,
                                candidate.candidateId,
                              )}
                            >
                              {candidate.sourceDisplayName}
                              <span className="bitfun-external-sources-config__ecosystem">
                                {candidate.ecosystemId}
                              </span>
                            </Button>
                            <span className="bitfun-external-sources-config__candidate-state">
                              {t(selected
                                ? selectedChoiceUnavailable
                                  ? 'common.selectedUnavailable'
                                  : 'common.selected'
                                : !available
                                  ? 'conflicts.restricted'
                                  : conflict.selectedCandidateId
                                    ? 'common.notSelected'
                                    : 'common.availableChoice')}
                            </span>
                            <div className="bitfun-external-sources-config__candidate-detail">
                              {candidate.commandDescription}
                              {' · '}
                              {candidate.sourceScope === 'workspace_local'
                                ? t('shared:features.workspace')
                                : t(`scope.${candidate.sourceScope}`)}
                              {' · '}
                              <span title={candidate.sourceLocation}>
                                {abbreviatedLocation(candidate.sourceLocation)}
                              </span>
                              {!available ? ` · ${t('conflicts.restricted')}` : ''}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                    <div className="bitfun-external-sources-config__conflict-hint">
                      {conflict.selectedCandidateId
                        ? t(selectedChoiceUnavailable
                          ? 'conflicts.currentSelectionUnavailable'
                          : 'conflicts.currentSelection')
                        : t('conflicts.pending')}
                    </div>
                    </div>
                  );
                })}
              </ConfigPageSection>
            ) : null}

            {toolConflicts.length > 0 ? (
              <ConfigPageSection
                title={t('toolConflicts.title')}
              >
                {toolConflicts.map((conflict) => {
                  const selectedCandidate = conflict.candidates.find((candidate) => (
                    candidate.candidateId === conflict.selectedCandidateId
                  ));
                  const selectedExternalTool = selectedCandidate?.kind === 'external'
                    ? snapshot?.tools?.find((tool) => (
                      tool.definition.id.target.source.providerId
                        === selectedCandidate.source?.providerId
                      && tool.definition.id.target.source.sourceId
                        === selectedCandidate.source?.sourceId
                      && tool.definition.modulePath === selectedCandidate.sourceLocation
                      && tool.definition.name === conflict.toolName
                      && tool.definition.contentVersion === selectedCandidate.contentVersion
                    ))
                    : undefined;
                  const selectedChoiceUnavailable = selectedCandidate?.kind === 'external'
                    && selectedExternalTool?.activation.state !== 'active';
                  return (
                    <div
                      className="bitfun-external-sources-config__conflict"
                      key={conflict.conflictKey}
                      data-external-attention={!conflict.selectedCandidateId ? 'true' : undefined}
                    >
                    <div className="bitfun-external-sources-config__conflict-title">
                      {t('toolConflicts.toolName', { name: conflict.toolName })}
                    </div>
                    <div className="bitfun-external-sources-config__conflict-options">
                      {conflict.candidates.map((candidate) => {
                        const selected = conflict.selectedCandidateId === candidate.candidateId;
                        return (
                          <div className="bitfun-external-sources-config__candidate" key={candidate.candidateId}>
                            <Button
                              variant={selected ? 'primary' : 'secondary'}
                              size="small"
                              disabled={!policyCompatible || busyKey === conflict.conflictKey
                                || !hostCapabilities.canApproveRuntime}
                              aria-pressed={selected}
                              onClick={() => void chooseToolConflict(
                                conflict.conflictKey,
                                candidate.candidateId,
                              )}
                            >
                              {candidate.displayName}
                              <span className="bitfun-external-sources-config__ecosystem">
                                {t(`toolCandidateKind.${candidate.kind}`)}
                              </span>
                            </Button>
                            <span className="bitfun-external-sources-config__candidate-state">
                              {t(selected
                                ? selectedChoiceUnavailable
                                  ? 'common.selectedUnavailable'
                                  : 'common.selected'
                                : conflict.selectedCandidateId
                                  ? 'common.notSelected'
                                  : 'common.availableChoice')}
                            </span>
                            <div className="bitfun-external-sources-config__candidate-detail">
                              {candidate.sourceLocation
                                ? abbreviatedLocation(candidate.sourceLocation)
                                : candidate.providerId}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                    <div className="bitfun-external-sources-config__conflict-hint">
                      {conflict.selectedCandidateId
                        ? t(selectedChoiceUnavailable
                          ? 'toolConflicts.currentSelectionUnavailable'
                          : 'toolConflicts.currentSelection')
                        : t('toolConflicts.pending')}
                    </div>
                    </div>
                  );
                })}
              </ConfigPageSection>
            ) : null}
          </>
        )}
      </ConfigPageContent>
      <ConfirmDialog
        isOpen={resetPolicyConfirmation !== null}
        onClose={() => setResetPolicyConfirmation(null)}
        onConfirm={() => {
          const confirmation = resetPolicyConfirmation;
          setResetPolicyConfirmation(null);
          if (confirmation) void resetIncompatiblePolicy(confirmation);
        }}
        title={t('policy.resetConfirmTitle')}
        message={t('policy.resetConfirmMessage')}
        type="warning"
        confirmDanger
        confirmText={t('policy.backupAndReset')}
      />
    </ConfigPageLayout>
  );
};

export default ExternalSourcesConfig;

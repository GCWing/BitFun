import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { RefreshCw } from 'lucide-react';
import { Button, ConfigPageLoading, Switch } from '@/component-library';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { isTauriRuntime } from '@/infrastructure/runtime';
import { isRemoteWorkspace } from '@/shared/types';
import {
  externalSourcesAPI,
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
import './ExternalSourcesConfig.scss';

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
  detail: string;
};

type AgentChangeNotice = {
  key: string;
  candidateIds: string[];
  message: string;
};

const DISABLED_SUBAGENT_CONFLICT_CHOICE = '__bitfun_disabled__';

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
  const desktopRuntime = isTauriRuntime();
  const remoteWorkspace = isRemoteWorkspace(workspace);
  const translateRef = useRef(t);
  translateRef.current = t;
  const [snapshot, setSnapshot] = useState<ExternalSourceCatalogSnapshot | null>(null);
  const [loading, setLoading] = useState(desktopRuntime && !remoteWorkspace);
  const [refreshing, setRefreshing] = useState(false);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [reviewingToolKey, setReviewingToolKey] = useState<string | null>(null);
  const [reviewingAgentKey, setReviewingAgentKey] = useState<string | null>(null);
  const [reviewingMcpKey, setReviewingMcpKey] = useState<string | null>(null);
  const [reviewingMcpConflictKey, setReviewingMcpConflictKey] = useState<string | null>(null);
  const [error, setError] = useState<ExternalSourcesError | null>(null);
  const [operationStatus, setOperationStatus] = useState<string | null>(null);
  const [agentChangeNotice, setAgentChangeNotice] = useState<AgentChangeNotice | null>(null);
  const snapshotRef = useRef<ExternalSourceCatalogSnapshot | null>(null);
  const agentChangeNoticeRef = useRef<AgentChangeNotice | null>(null);
  const operationStatusRef = useRef<HTMLDivElement>(null);
  const focusOperationStatus = useRef(false);
  const requestSequence = useRef(0);
  const acceptedSequence = useRef(0);
  const pendingMutations = useRef(new Map<number, string>());
  const latestMutationByScope = useRef(new Map<string, number>());
  const mutationInFlight = useRef(false);
  const foregroundSequence = useRef<number | null>(null);
  const requestScope = `${desktopRuntime}:${remoteWorkspace}:${workspacePath ?? ''}`;
  const requestScopeRef = useRef(requestScope);
  if (requestScopeRef.current !== requestScope) {
    requestScopeRef.current = requestScope;
    requestSequence.current += 1;
    acceptedSequence.current = requestSequence.current;
  }

  const applySnapshot = useCallback((
    next: ExternalSourceCatalogSnapshot,
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
    setSnapshot(selected);
  }, []);

  const acceptReadSnapshot = useCallback((
    next: ExternalSourceCatalogSnapshot,
    scope: string,
    sequence: number,
  ): boolean => {
    if (requestScopeRef.current !== scope || sequence < acceptedSequence.current) return false;
    if (Array.from(pendingMutations.current.values()).includes(scope)) return false;
    acceptedSequence.current = sequence;
    applySnapshot(next);
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
    applySnapshot(next, partition, 'mutation');
    return true;
  }, [applySnapshot]);

  const loadSnapshot = useCallback(async (forceRefresh: boolean, foreground: boolean) => {
    if (!desktopRuntime || remoteWorkspace) return;
    const scope = requestScope;
    const sequence = ++requestSequence.current;
    if (foreground) {
      foregroundSequence.current = sequence;
      setRefreshing(true);
    }
    try {
      const next = await externalSourcesAPI.getSnapshot(workspacePath, forceRefresh);
      if (!acceptReadSnapshot(next, scope, sequence)) return;
      setError(null);
    } catch (loadError) {
      if (requestScopeRef.current !== scope || sequence < acceptedSequence.current) return;
      acceptedSequence.current = sequence;
      setError({
        kind: 'load',
        detail: loadError instanceof Error ? loadError.message : String(loadError),
      });
    } finally {
      if (requestScopeRef.current === scope) {
        if (sequence >= acceptedSequence.current) setLoading(false);
        if (foregroundSequence.current === sequence) {
          foregroundSequence.current = null;
          setRefreshing(false);
        }
      }
    }
  }, [acceptReadSnapshot, desktopRuntime, remoteWorkspace, requestScope, workspacePath]);

  useEffect(() => {
    setSnapshot(null);
    snapshotRef.current = null;
    agentChangeNoticeRef.current = null;
    setAgentChangeNotice(null);
    setError(null);
    setOperationStatus(null);
    focusOperationStatus.current = false;
    setBusyKey(null);
    setReviewingToolKey(null);
    setReviewingAgentKey(null);
    setReviewingMcpKey(null);
    setReviewingMcpConflictKey(null);
    setLoading(desktopRuntime && !remoteWorkspace);
    void loadSnapshot(false, false);
    if (!desktopRuntime || remoteWorkspace) return undefined;
    const timer = window.setInterval(() => void loadSnapshot(false, false), 5000);
    return () => window.clearInterval(timer);
  }, [desktopRuntime, loadSnapshot, remoteWorkspace, workspacePath]);

  useEffect(() => {
    if (operationStatus && focusOperationStatus.current) {
      focusOperationStatus.current = false;
      operationStatusRef.current?.focus();
    }
  }, [operationStatus]);

  useEffect(() => {
    if (!desktopRuntime || remoteWorkspace || !snapshot?.discoveryPending) return undefined;
    const timer = window.setInterval(() => void loadSnapshot(false, false), 750);
    return () => window.clearInterval(timer);
  }, [desktopRuntime, loadSnapshot, remoteWorkspace, snapshot?.discoveryPending]);

  const commandCounts = useMemo(() => {
    const namesBySource = new Map<string, Set<string>>();
    const add = (providerId: string, sourceId: string, commandName: string) => {
      const key = `${providerId}\u0000${sourceId}`;
      const names = namesBySource.get(key) ?? new Set<string>();
      names.add(commandName.toLowerCase());
      namesBySource.set(key, names);
    };
    for (const command of snapshot?.commands ?? []) {
      const source = command.definition.id.source;
      add(source.providerId, source.sourceId, command.definition.name);
    }
    for (const conflict of snapshot?.commandConflicts ?? []) {
      for (const candidate of conflict.candidates) {
        add(candidate.source.providerId, candidate.source.sourceId, conflict.commandName);
      }
    }
    return new Map(
      Array.from(namesBySource, ([source, names]) => [source, names.size]),
    );
  }, [snapshot]);

  const toolCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const tool of snapshot?.tools ?? []) {
      const source = tool.definition.id.target.source;
      const key = `${source.providerId}\u0000${source.sourceId}`;
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return counts;
  }, [snapshot?.tools]);

  const agentCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const agent of snapshot?.subagents ?? []) {
      for (const source of agent.sourceKeys) {
        const key = `${source.providerId}\u0000${source.sourceId}`;
        counts.set(key, (counts.get(key) ?? 0) + 1);
      }
    }
    return counts;
  }, [snapshot]);

  const mcpCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const server of snapshot?.mcpServers ?? []) {
      const source = server.definition.id.source;
      const key = `${source.providerId}\u0000${source.sourceId}`;
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return counts;
  }, [snapshot?.mcpServers]);

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

  const runMutation = useCallback(async (
    mutationKey: string,
    request: () => Promise<ExternalSourceCatalogSnapshot>,
    focusResult = false,
    partition: 'all' | 'subagents' = 'all',
    successMessage?: string,
  ): Promise<boolean> => {
    if (mutationInFlight.current) {
      setOperationStatus(t('actions.waitForUpdate'));
      return false;
    }
    mutationInFlight.current = true;
    const scope = requestScope;
    const sequence = ++requestSequence.current;
    pendingMutations.current.set(sequence, scope);
    latestMutationByScope.current.set(scope, sequence);
    setBusyKey(mutationKey);
    setOperationStatus(null);
    try {
      setError(null);
      const next = await request();
      const accepted = acceptMutationSnapshot(next, scope, sequence, partition);
      if (accepted) {
        focusOperationStatus.current = focusResult;
        setOperationStatus(successMessage ?? t('actions.updated'));
      }
      return accepted;
    } catch (updateError) {
      if (requestScopeRef.current === scope
        && latestMutationByScope.current.get(scope) === sequence) {
        acceptedSequence.current = sequence;
        setError({
          kind: 'mutation',
          detail: updateError instanceof Error ? updateError.message : String(updateError),
        });
      }
      return false;
    } finally {
      mutationInFlight.current = false;
      pendingMutations.current.delete(sequence);
      if (requestScopeRef.current === scope) {
        setBusyKey((current) => (current === mutationKey ? null : current));
      }
    }
  }, [acceptMutationSnapshot, requestScope, t]);

  const setEnabled = useCallback(async (sourceKey: string, enabled: boolean) => {
    await runMutation(
      sourceKey,
      () => externalSourcesAPI.setSourceEnabled(workspacePath, sourceKey, enabled),
    );
  }, [runMutation, workspacePath]);

  const chooseConflict = useCallback(async (conflictKey: string, candidateId: string) => {
    await runMutation(
      conflictKey,
      () => externalSourcesAPI.setConflictChoice(workspacePath, conflictKey, candidateId),
      true,
    );
  }, [runMutation, workspacePath]);

  const decideToolTarget = useCallback(async (
    approvalKey: string,
    decisionKey: string,
    approved: boolean,
  ) => {
    return runMutation(
      decisionKey,
      () => externalSourcesAPI.setToolTargetDecision(
        workspacePath,
        approvalKey,
        decisionKey,
        approved,
      ),
      true,
    );
  }, [runMutation, workspacePath]);

  const chooseToolConflict = useCallback(async (conflictKey: string, candidateId: string) => {
    await runMutation(
      conflictKey,
      () => externalSourcesAPI.setToolConflictChoice(workspacePath, conflictKey, candidateId),
      true,
    );
  }, [runMutation, workspacePath]);

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
    );
    if (accepted) await loadSnapshot(true, false);
    return accepted;
  }, [loadSnapshot, runMutation, snapshot, t, workspacePath]);

  if (loading && !snapshot) {
    return <ConfigPageLoading text={t('loading')} />;
  }

  const unavailableReason = !desktopRuntime
    ? t('unavailable.desktopOnly')
    : remoteWorkspace
      ? t('unavailable.remoteWorkspace')
      : null;

  return (
    <ConfigPageLayout className="bitfun-external-sources-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
        extra={!remoteWorkspace && desktopRuntime ? (
          <Button
            variant="secondary"
            size="small"
            disabled={refreshing}
            onClick={() => void loadSnapshot(true, true)}
          >
            <RefreshCw size={14} aria-hidden="true" />
            {refreshing ? t('actions.refreshing') : t('actions.refresh')}
          </Button>
        ) : undefined}
      />
      <ConfigPageContent>
        {unavailableReason ? (
          <ConfigPageSection title={t('unavailable.title')} description={unavailableReason}>
            {null}
          </ConfigPageSection>
        ) : (
          <>
            {error ? (
              <div className="bitfun-external-sources-config__notice" role="status">
                <div>{t(error.kind === 'mutation'
                  ? 'errors.mutationUnknown'
                  : snapshot
                    ? 'errors.refreshFailed'
                    : 'errors.loadFailed')}</div>
                <details>
                  <summary>{t('common.technicalDetails')}</summary>
                  <div>{error.detail}</div>
                </details>
              </div>
            ) : null}
            {operationStatus ? (
              <div
                ref={operationStatusRef}
                className="bitfun-external-sources-config__notice"
                role="status"
                aria-live="polite"
                tabIndex={-1}
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
            {(snapshot?.diagnostics?.length ?? 0) > 0 ? (
              <details className="bitfun-external-sources-config__notice">
                <summary>
                  {t('diagnostics.summary', { count: snapshot?.diagnostics?.length ?? 0 })}
                </summary>
                <ul className="bitfun-external-sources-config__diagnostics">
                  {snapshot?.diagnostics?.map((diagnostic, index) => (
                    <li key={`${diagnostic.code}-${index}`}>
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
            {!workspacePath ? (
              <div className="bitfun-external-sources-config__notice" role="status">
                {t('sources.globalOnly')}
              </div>
            ) : null}

            {snapshot?.discoveryPending ? (
              <div className="bitfun-external-sources-config__notice" role="status">
                {t('checkingNonBlocking')}
              </div>
            ) : null}

            {(snapshot?.mcpApprovalRequests?.length ?? 0) > 0 ? (
              <ConfigPageSection
                title={t('mcpApprovals.title')}
                description={t('mcpApprovals.description')}
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
                        disabled={busyKey !== null}
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
                        disabled={busyKey !== null}
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
              <ConfigPageSection title={t('mcp.title')} description={t('mcp.description')}>
                {snapshot?.mcpServers?.map((server) => {
                  const state = server.activationState.state;
                  const reviewing = reviewingMcpKey === server.candidateId;
                  const canEnable = state === 'declined' || state === 'configuration_changed';
                  const canDisable = ['starting', 'active', 'runtime_unavailable'].includes(state);
                  return (
                    <React.Fragment key={server.candidateId}>
                      <ConfigPageRow
                        label={server.definition.name}
                        description={`${t(`mcp.transport.${server.definition.transport}`)} · ${t('mcp.externalSource')}`}
                        align="center"
                      >
                        <div className="bitfun-external-sources-config__source-control">
                          <span className={`bitfun-external-sources-config__state is-${state}`}>
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
                              disabled={busyKey !== null}
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
                              <>
                                <span>{t(server.activationState.state === 'runtime_unavailable'
                                  ? 'mcp.runtimeUnavailableGuidance'
                                  : 'mcp.unsupportedGuidance')}</span>
                                <details>
                                  <summary>{t('common.technicalDetails')}</summary>
                                  <code>{server.activationState.reason}</code>
                                </details>
                              </>
                            ) : null}
                            <span>{t('mcp.changePolicy')}</span>
                          </div>
                          {canEnable ? (
                            <div className="bitfun-external-sources-config__tool-actions">
                              <Button
                                variant="primary"
                                size="small"
                                disabled={busyKey !== null}
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
                description={t('mcpConflicts.description')}
              >
                {mcpConflicts.map((conflict) => (
                  <div className="bitfun-external-sources-config__conflict" key={conflict.conflictKey}>
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
                              disabled={busyKey !== null || !candidate.available}
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
                                      disabled={busyKey !== null}
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

            {(snapshot?.subagents?.length ?? 0) > 0 ? (
              <ConfigPageSection title={t('agents.title')} description={t('agents.description')}>
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
                          <span className={`bitfun-external-sources-config__state is-${state}`}>
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
                              disabled={busyKey !== null}
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
                            <span>{t('agents.sources', { count: agent.sourceCount })}</span>
                            {agent.sourceLocationLabels.map((location) => (
                              <span key={location}>{abbreviatedLocation(location)}</span>
                            ))}
                            {agent.diagnostics.map((diagnostic) => {
                                const category = agentDiagnosticCategory(
                                  diagnostic.code,
                                  diagnostic.blocksActivation,
                                );
                              return (
                                <div key={diagnostic.code}>
                                  <span>{t(`agentDiagnostics.${category}.reason`)}</span>
                                  <span>{t(`agentDiagnostics.${category}.impact`, {
                                    impact: diagnostic.blocksActivation
                                      ? t('agentDiagnostics.activationBlocked')
                                      : t('agentDiagnostics.degradedOnly'),
                                  })}</span>
                                  <span>{t(`agentDiagnostics.${category}.nextStep`)}</span>
                                  <details>
                                    <summary>{t('common.technicalDetails')}</summary>
                                    <code>{diagnostic.code}</code>
                                  </details>
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
                                disabled={busyKey !== null}
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
                description={t('agentConflicts.description')}
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
                    <div className="bitfun-external-sources-config__conflict" key={conflict.conflictKey}>
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
                              disabled={busyKey !== null}
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
                        disabled={busyKey !== null}
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
                description={t('toolApprovals.description')}
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
                          disabled={busyKey === request.decisionKey}
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
                          disabled={busyKey === request.decisionKey}
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

            <ConfigPageSection
              title={t('sources.title')}
              description={t('sources.description')}
            >
              {!snapshot?.discoveryPending && (snapshot?.sources.length ?? 0) === 0 ? (
                <div className="bitfun-external-sources-config__empty">{t('sources.empty')}</div>
              ) : snapshot?.sources.map((source) => {
                const sourcePair = `${source.record.key.providerId}\u0000${source.record.key.sourceId}`;
                const removed = source.lifecycle === 'removed';
                const enabled = !removed && source.lifecycle !== 'suppressed';
                return (
                  <ConfigPageRow
                    key={source.stableKey}
                    label={source.record.displayName}
                    description={(
                      <>
                        <span>{source.record.location}</span>
                        {' · '}
                        {source.record.scope === 'workspace_local'
                          ? t('shared:features.workspace')
                          : t(`scope.${source.record.scope}`)}
                        {' · '}
                        {t('sources.commandCount', { count: commandCounts.get(sourcePair) ?? 0 })}
                        {' · '}
                        {t('sources.toolCount', { count: toolCounts.get(sourcePair) ?? 0 })}
                        {' · '}
                        {t('sources.agentCount', { count: agentCounts.get(sourcePair) ?? 0 })}
                        {' · '}
                        {t('sources.mcpCount', { count: mcpCounts.get(sourcePair) ?? 0 })}
                      </>
                    )}
                    align="center"
                  >
                    <div className="bitfun-external-sources-config__source-control">
                      <span className={`bitfun-external-sources-config__state is-${source.lifecycle}`}>
                        {t(`lifecycle.${source.lifecycle}`)}
                      </span>
                      <Switch
                        size="small"
                        checked={enabled}
                        disabled={removed}
                        loading={busyKey === source.stableKey}
                        aria-label={t('sources.toggleLabel', { name: source.record.displayName })}
                        onChange={(event) => void setEnabled(source.stableKey, event.currentTarget.checked)}
                      />
                    </div>
                  </ConfigPageRow>
                );
              })}
            </ConfigPageSection>

            {(snapshot?.tools?.length ?? 0) > 0 ? (
              <ConfigPageSection title={t('tools.title')} description={t('tools.description')}>
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
                          <span className={`bitfun-external-sources-config__state is-${tool.activation.state}`}>
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
                              disabled={busyKey === tool.decisionKey}
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
                              disabled={busyKey === tool.decisionKey}
                              onClick={() => setReviewingToolKey(null)}
                            >
                              {t('common.close')}
                            </Button>
                            {enableable ? (
                              <Button
                                variant="primary"
                                size="small"
                                disabled={busyKey === tool.decisionKey}
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
                description={t('conflicts.description')}
              >
                {commandConflicts.map((conflict) => {
                  const selectedChoiceUnavailable = conflict.candidates.some((candidate) => (
                    candidate.candidateId === conflict.selectedCandidateId
                    && candidate.availability.state !== 'available'
                  ));
                  return (
                    <div className="bitfun-external-sources-config__conflict" key={conflict.conflictKey}>
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
                              disabled={busyKey === conflict.conflictKey || !available}
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
                description={t('toolConflicts.description')}
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
                    <div className="bitfun-external-sources-config__conflict" key={conflict.conflictKey}>
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
                              disabled={busyKey === conflict.conflictKey}
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
    </ConfigPageLayout>
  );
};

export default ExternalSourcesConfig;

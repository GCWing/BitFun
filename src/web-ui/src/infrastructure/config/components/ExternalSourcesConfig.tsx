import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw } from 'lucide-react';
import { Button, ConfigPageLoading, Switch } from '@/component-library';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { isTauriRuntime } from '@/infrastructure/runtime';
import { isRemoteWorkspace } from '@/shared/types';
import {
  externalSourcesAPI,
  type ExternalSourceCatalogSnapshot,
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

const ExternalSourcesConfig: React.FC = () => {
  const { t } = useTranslation('settings/external-sources');
  const { workspace, workspacePath } = useCurrentWorkspace();
  const desktopRuntime = isTauriRuntime();
  const remoteWorkspace = isRemoteWorkspace(workspace);
  const [snapshot, setSnapshot] = useState<ExternalSourceCatalogSnapshot | null>(null);
  const [loading, setLoading] = useState(desktopRuntime && !remoteWorkspace);
  const [refreshing, setRefreshing] = useState(false);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const requestSequence = useRef(0);
  const acceptedSequence = useRef(0);
  const pendingMutations = useRef(new Map<number, string>());
  const latestMutationByScope = useRef(new Map<string, number>());
  const foregroundSequence = useRef<number | null>(null);
  const requestScope = `${desktopRuntime}:${remoteWorkspace}:${workspacePath ?? ''}`;
  const requestScopeRef = useRef(requestScope);
  if (requestScopeRef.current !== requestScope) {
    requestScopeRef.current = requestScope;
    requestSequence.current += 1;
    acceptedSequence.current = requestSequence.current;
  }

  const applySnapshot = useCallback((next: ExternalSourceCatalogSnapshot) => {
    setSnapshot((current) => (
      current && next.generation < current.generation ? current : next
    ));
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
  ): boolean => {
    if (requestScopeRef.current !== scope) return false;
    if ((latestMutationByScope.current.get(scope) ?? sequence) > sequence) return false;
    acceptedSequence.current = Math.max(acceptedSequence.current, sequence);
    applySnapshot(next);
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
      setError(loadError instanceof Error ? loadError.message : String(loadError));
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
    setError(null);
    setBusyKey(null);
    setLoading(desktopRuntime && !remoteWorkspace);
    void loadSnapshot(false, false);
    if (!desktopRuntime || remoteWorkspace) return undefined;
    const timer = window.setInterval(() => void loadSnapshot(false, false), 5000);
    return () => window.clearInterval(timer);
  }, [desktopRuntime, loadSnapshot, remoteWorkspace, workspacePath]);

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

  const pendingConflicts = useMemo(
    () => (snapshot?.commandConflicts ?? []).filter(
      (conflict) => !conflict.selectedCandidateId,
    ),
    [snapshot?.commandConflicts],
  );

  const setEnabled = useCallback(async (sourceKey: string, enabled: boolean) => {
    const scope = requestScope;
    const sequence = ++requestSequence.current;
    pendingMutations.current.set(sequence, scope);
    latestMutationByScope.current.set(scope, sequence);
    setBusyKey(sourceKey);
    try {
      setError(null);
      const next = await externalSourcesAPI.setSourceEnabled(workspacePath, sourceKey, enabled);
      acceptMutationSnapshot(next, scope, sequence);
    } catch (updateError) {
      if (requestScopeRef.current === scope
        && latestMutationByScope.current.get(scope) === sequence) {
        acceptedSequence.current = sequence;
        setError(updateError instanceof Error ? updateError.message : String(updateError));
      }
    } finally {
      pendingMutations.current.delete(sequence);
      if (requestScopeRef.current === scope) {
        setBusyKey((current) => (current === sourceKey ? null : current));
      }
    }
  }, [acceptMutationSnapshot, requestScope, workspacePath]);

  const chooseConflict = useCallback(async (conflictKey: string, candidateId: string) => {
    const scope = requestScope;
    const sequence = ++requestSequence.current;
    pendingMutations.current.set(sequence, scope);
    latestMutationByScope.current.set(scope, sequence);
    setBusyKey(conflictKey);
    try {
      setError(null);
      const next = await externalSourcesAPI.setConflictChoice(
        workspacePath,
        conflictKey,
        candidateId,
      );
      acceptMutationSnapshot(next, scope, sequence);
    } catch (updateError) {
      if (requestScopeRef.current === scope
        && latestMutationByScope.current.get(scope) === sequence) {
        acceptedSequence.current = sequence;
        setError(updateError instanceof Error ? updateError.message : String(updateError));
      }
    } finally {
      pendingMutations.current.delete(sequence);
      if (requestScopeRef.current === scope) {
        setBusyKey((current) => (current === conflictKey ? null : current));
      }
    }
  }, [acceptMutationSnapshot, requestScope, workspacePath]);

  if (loading || snapshot?.discoveryPending) {
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
            <RefreshCw size={14} />
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
                {t('errors.nonBlocking', { error })}
              </div>
            ) : null}
            {(snapshot?.diagnostics?.length ?? 0) > 0 ? (
              <details className="bitfun-external-sources-config__notice">
                <summary>
                  {t('diagnostics.summary', { count: snapshot?.diagnostics?.length ?? 0 })}
                </summary>
                <ul className="bitfun-external-sources-config__diagnostics">
                  {snapshot?.diagnostics?.map((diagnostic, index) => (
                    <li key={`${diagnostic.code}-${index}`}>{diagnostic.message}</li>
                  ))}
                </ul>
              </details>
            ) : null}
            {!workspacePath ? (
              <div className="bitfun-external-sources-config__notice" role="status">
                {t('sources.globalOnly')}
              </div>
            ) : null}

            <ConfigPageSection
              title={t('sources.title')}
              description={t('sources.description')}
            >
              {(snapshot?.sources.length ?? 0) === 0 ? (
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
                        <span title={source.record.location}>
                          {abbreviatedLocation(source.record.location)}
                        </span>
                        {' · '}
                        {source.record.scope === 'workspace_local'
                          ? t('shared:features.workspace')
                          : t(`scope.${source.record.scope}`)}
                        {' · '}
                        {t('sources.commandCount', { count: commandCounts.get(sourcePair) ?? 0 })}
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

            {pendingConflicts.length > 0 ? (
              <ConfigPageSection
                title={t('conflicts.title')}
                description={t('conflicts.description')}
              >
                {pendingConflicts.map((conflict) => (
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
                    {!conflict.selectedCandidateId ? (
                      <div className="bitfun-external-sources-config__conflict-hint">
                        {t('conflicts.pending')}
                      </div>
                    ) : null}
                  </div>
                ))}
              </ConfigPageSection>
            ) : null}
          </>
        )}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default ExternalSourcesConfig;

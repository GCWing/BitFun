import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Puzzle, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { IconButton } from '@/component-library';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/peerDeviceContextState';
import {
  type ExternalMcpActivation,
  type ExternalMcpCatalogEntry,
  ExternalSourceApiError,
  type ExternalSourceCatalogSnapshot,
  type ExternalSourceScope,
  externalSourcesAPI,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import { createLogger } from '@/shared/utils/logger';
import { ConfigCollectionItem, ConfigPageSection } from './common';
import { externalSourceRequestScopeKey } from './externalSourceRequestScope';

const log = createLogger('ExternalMcpOverview');
const DISCOVERY_POLL_DELAYS_MS = [750, 1_500, 3_000, 5_000] as const;

function sourceKey(providerId: string, sourceId: string): string {
  return `${providerId}\u0000${sourceId}`;
}

function statusTone(activation: ExternalMcpActivation): string {
  switch (activation.state) {
    case 'active':
      return 'is-healthy';
    case 'approval_required':
    case 'starting':
    case 'configuration_changed':
      return 'is-pending';
    case 'conflict':
    case 'unsupported':
    case 'runtime_unavailable':
      return 'is-error';
    default:
      return 'is-muted';
  }
}

function scopeRank(scope: ExternalSourceScope | undefined): number {
  switch (scope) {
    case 'project':
    case 'workspace_local':
    case 'remote_project':
      return 0;
    case 'user_global':
    case 'remote_user':
      return 1;
    default:
      return 2;
  }
}

type ExternalMcpSourceState = 'stale' | 'degraded' | null;

function sourceState(
  source: ExternalSourceCatalogSnapshot['sources'][number] | undefined,
): ExternalMcpSourceState {
  if (!source) return null;
  if (source.lifecycle === 'using_last_valid_version') return 'stale';
  if (
    ['restricted', 'degraded', 'unavailable'].includes(source.lifecycle)
    || ['partial', 'degraded', 'unavailable'].includes(source.record.health)
    || (source.record.diagnostics ?? []).some((diagnostic) => diagnostic.severity !== 'info')
  ) {
    return 'degraded';
  }
  return null;
}

function safeLoadErrorFacts(error: unknown): Record<string, unknown> {
  if (error instanceof ExternalSourceApiError) {
    const correlationId = error.correlationId?.trim();
    return {
      error_type: 'external_source_api',
      code: error.code,
      correlation_id: correlationId && /^[a-z0-9_-]{1,64}$/i.test(correlationId)
        ? correlationId
        : undefined,
      retryable: error.retryable,
    };
  }
  return {
    error_type: error instanceof Error ? 'error' : 'unknown',
    code: 'internal',
    correlation_id: undefined,
    retryable: false,
  };
}

const ExternalMcpDetail: React.FC<{
  label: string;
  value: string;
  code?: boolean;
}> = ({ label, value, code = false }) => (
  <div className="bitfun-mcp-tools__server-detail-item">
    <span className="bitfun-mcp-tools__server-detail-label">{label}:</span>
    {code ? (
      <code className="bitfun-mcp-tools__server-detail-value">{value}</code>
    ) : (
      <span className="bitfun-mcp-tools__server-detail-value">{value}</span>
    )}
  </div>
);

const ExternalMcpOverview: React.FC = () => {
  const { t } = useTranslation('settings/mcp');
  const { t: tShared } = useTranslation('shared');
  const { workspace, workspacePath } = useCurrentWorkspace();
  const peerDevice = usePeerDeviceModeOptional();
  const setSettingsTab = useSettingsStore((state) => state.setActiveTab);
  const requestIdRef = useRef(0);
  const peerDeviceId = peerDevice?.peerMode.active ? peerDevice.peerMode.deviceId : undefined;
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
    snapshot: ExternalSourceCatalogSnapshot | null;
  } | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadFailed, setLoadFailed] = useState(false);
  const snapshot = snapshotState?.scope === requestScope ? snapshotState.snapshot : null;
  const scopedLoading = loading || snapshotState?.scope !== requestScope;

  const loadSnapshot = useCallback(async () => {
    const requestId = ++requestIdRef.current;
    setLoading(true);
    setLoadFailed(false);
    try {
      const nextSnapshot = await externalSourcesAPI.getSnapshot(workspacePath || undefined);
      if (requestId === requestIdRef.current) {
        setSnapshotState({ scope: requestScope, snapshot: nextSnapshot });
      }
    } catch (error) {
      if (requestId === requestIdRef.current) {
        setSnapshotState((current) => (
          current?.scope === requestScope && current.snapshot
            ? current
            : { scope: requestScope, snapshot: null }
        ));
        setLoadFailed(true);
        log.warn('Failed to load external MCP summary', safeLoadErrorFacts(error));
      }
    } finally {
      if (requestId === requestIdRef.current) {
        setLoading(false);
      }
    }
  }, [requestScope, workspacePath]);

  useEffect(() => {
    void loadSnapshot();
    return () => {
      requestIdRef.current += 1;
    };
  }, [loadSnapshot]);

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
        await loadSnapshot();
        if (cancelled) return;
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

  const sourceByKey = useMemo(() => new Map(
    (snapshot?.sources ?? []).map((source) => [
      sourceKey(source.record.key.providerId, source.record.key.sourceId),
      source,
    ]),
  ), [snapshot?.sources]);

  const ecosystemLabels = useMemo(() => new Map(
    (snapshot?.integrationPolicy.registeredEcosystems ?? []).map((ecosystem) => [
      ecosystem.ecosystemId,
      ecosystem.displayName,
    ]),
  ), [snapshot?.integrationPolicy.registeredEcosystems]);

  const entries = useMemo(() => [...(snapshot?.mcpServers ?? [])].sort((left, right) => {
    const leftSource = sourceByKey.get(sourceKey(
      left.definition.id.source.providerId,
      left.definition.id.source.sourceId,
    ));
    const rightSource = sourceByKey.get(sourceKey(
      right.definition.id.source.providerId,
      right.definition.id.source.sourceId,
    ));
    return scopeRank(leftSource?.record.scope) - scopeRank(rightSource?.record.scope)
      || left.definition.name.localeCompare(right.definition.name);
  }), [snapshot?.mcpServers, sourceByKey]);

  const hostReadOnly = snapshot !== null
    && !snapshot.hostCapabilities.canMutatePolicy
    && !snapshot.hostCapabilities.canManageSources
    && !snapshot.hostCapabilities.canApproveRuntime;
  const hasMcpDiagnostics = (snapshot?.diagnostics ?? []).some((diagnostic) => (
    diagnostic.severity !== 'info'
    && (!diagnostic.assetKind || diagnostic.assetKind === 'source' || diagnostic.assetKind === 'mcp')
  ));

  const scopeLabel = (scope: ExternalSourceScope | undefined): string => {
    switch (scope) {
      case 'user_global': return t('external.scope.userGlobal');
      case 'project': return t('external.scope.project');
      case 'workspace_local': return tShared('features.workspace');
      case 'remote_user': return t('external.scope.remoteUser');
      case 'remote_project': return t('external.scope.remoteProject');
      default: return t('external.unknown');
    }
  };

  const activationLabel = (activation: ExternalMcpActivation): string => {
    switch (activation.state) {
      case 'approval_required': return t('external.status.approvalRequired');
      case 'starting': return t('external.status.starting');
      case 'active': return t('external.status.active');
      case 'declined': return t('external.status.declined');
      case 'conflict': return t('external.status.conflict');
      case 'covered': return t('external.status.covered');
      case 'source_disabled': return t('external.status.sourceDisabled');
      case 'configuration_changed': return t('external.status.configurationChanged');
      case 'unsupported': return t('external.status.unsupported');
      case 'runtime_unavailable': return t('external.status.runtimeUnavailable');
      case 'removed': return t('external.status.removed');
      default: return t('external.unknown');
    }
  };

  const renderEntry = (entry: ExternalMcpCatalogEntry) => {
    const source = sourceByKey.get(sourceKey(
      entry.definition.id.source.providerId,
      entry.definition.id.source.sourceId,
    ));
    const sourceRecord = source?.record;
    const ecosystemLabel = sourceRecord
      ? ecosystemLabels.get(sourceRecord.ecosystemId) ?? sourceRecord.ecosystemId
      : entry.definition.id.source.providerId;
    const sourceStatus = sourceState(source);
    const badges = (
      <>
        <span className="bitfun-collection-item__badge bitfun-mcp-tools__external-source-badge">
          {ecosystemLabel}
        </span>
        <span className="bitfun-collection-item__badge">
          {scopeLabel(sourceRecord?.scope)}
        </span>
        {sourceStatus ? (
          <span className={`bitfun-mcp-tools__status-badge ${sourceStatus === 'stale' ? 'is-pending' : 'is-error'}`}>
            {t(`external.status.${sourceStatus}`)}
          </span>
        ) : null}
      </>
    );
    const details = (
      <div className="bitfun-mcp-tools__server-details">
        <ExternalMcpDetail label={t('external.details.source')} value={sourceRecord?.displayName ?? ecosystemLabel} />
        <ExternalMcpDetail label={t('external.details.scope')} value={scopeLabel(sourceRecord?.scope)} />
        <ExternalMcpDetail
          label={t('external.details.location')}
          value={sourceRecord?.location ?? t('external.unknown')}
          code
        />
        <ExternalMcpDetail
          label={t('external.details.transport')}
          value={entry.definition.transport === 'local_stdio'
            ? t('external.transport.localStdio')
            : t('external.transport.streamableHttp')}
        />
      </div>
    );
    return (
      <ConfigCollectionItem
        key={entry.candidateId}
        data-testid="external-mcp-item"
        label={entry.definition.name}
        badge={badges}
        badgePlacement="below"
        control={(
          <span className={`bitfun-mcp-tools__status-badge ${statusTone(entry.activationState)}`}>
            {activationLabel(entry.activationState)}
          </span>
        )}
        details={details}
      />
    );
  };

  return (
    <ConfigPageSection
      className="bitfun-mcp-tools__external-section"
      title={t('external.title')}
      titleSuffix={snapshot ? (
        <span className="bitfun-mcp-tools__external-summary">
          {snapshot.discoveryPending ? (
            <span className="bitfun-mcp-tools__status-badge is-pending">
              {t('external.status.checking')}
            </span>
          ) : null}
          {hostReadOnly ? (
            <span className="bitfun-mcp-tools__status-badge is-muted">
              {t('external.status.readOnly')}
            </span>
          ) : null}
          {loadFailed && snapshot ? (
            <span className="bitfun-mcp-tools__status-badge is-pending">
              {t('external.status.stale')}
            </span>
          ) : null}
          {hasMcpDiagnostics ? (
            <span className="bitfun-mcp-tools__status-badge is-error">
              {t('external.status.degraded')}
            </span>
          ) : null}
        </span>
      ) : undefined}
      extra={(
        <>
          {loadFailed ? (
            <IconButton
              variant="ghost"
              size="small"
              onClick={() => void loadSnapshot()}
              tooltip={t('external.retry')}
              aria-label={t('external.retry')}
            >
              <RefreshCw size={16} aria-hidden="true" />
            </IconButton>
          ) : null}
          <IconButton
            variant="ghost"
            size="small"
            onClick={() => setSettingsTab('external-sources')}
            tooltip={t('external.manage')}
            aria-label={t('external.manage')}
          >
            <Puzzle size={16} aria-hidden="true" />
          </IconButton>
        </>
      )}
    >
      {scopedLoading && !snapshot ? (
        <div className="bitfun-collection-empty"><p>{t('external.loading')}</p></div>
      ) : loadFailed && !snapshot ? (
        <div className="bitfun-collection-empty" role="status"><p>{t('external.unavailable')}</p></div>
      ) : snapshot?.discoveryPending && entries.length === 0 ? (
        <div className="bitfun-collection-empty" role="status"><p>{t('external.loading')}</p></div>
      ) : entries.length === 0 ? (
        <div className="bitfun-collection-empty"><p>{t('external.empty')}</p></div>
      ) : entries.map(renderEntry)}
    </ConfigPageSection>
  );
};

export default ExternalMcpOverview;

import {
  Button,
  Icon,
  LoadingState,
  NavigationPanel,
  NavigationPanelBody,
  NavigationPanelContent,
  NavigationPanelHeader,
  NavigationPanelItem,
  NavigationPanelSection,
  OverflowText,
  ScrollArea,
  SearchField,
  Select,
  StatusPill,
  Switch,
  Textarea,
  type IconSource,
  type StatusPillTone,
} from '@openbitfun/ui';
import React, { Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Bot, CircleUserRound, Network, Package, PawPrint, Server, Webhook, Wrench } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import {
  externalSourcesAPI,
  type ExternalMcpImportPlanV1,
  type ExternalSourceCatalogSnapshot,
} from '@/infrastructure/api/service-api/ExternalSourcesAPI';
import {
  ACPClientAPI,
  type AcpClientInfo,
} from '@/infrastructure/api/service-api/ACPClientAPI';
import { useNotification } from '@/shared/notification-system';
import { usePeerDeviceModeOptional } from '@/infrastructure/peer-device/peerDeviceContextState';
import { WorkspaceKind } from '@/shared/types';
import {
  buildEcosystemImportItems,
  buildEcosystemProductRuntimes,
  totalDiscoveredAssets,
  type EcosystemImportItem,
  type EcosystemImportItemKind,
  type EcosystemProductId,
  type EcosystemProductRuntime,
} from './ecosystemCompatibilityModel';
import { useEcosystemCompatibilityStore } from './ecosystemCompatibilityStore';
import './EcosystemCompatibilityScene.scss';

const AcpAgentsConfig = lazy(
  () => import('@/infrastructure/config/components/AcpAgentsConfig'),
);
const ExternalSourcesConfig = lazy(
  () => import('@/infrastructure/config/components/ExternalSourcesConfig'),
);

const PRODUCT_ICON_SOURCES: Record<EcosystemProductId, string> = {
  'claude-code': '/assets/ecosystem-compatibility/claude-code.svg',
  codex: '/assets/ecosystem-compatibility/codex.svg',
  pi: '/assets/ecosystem-compatibility/pi.svg',
  dsh: '/assets/ecosystem-compatibility/deepseek-harness.svg',
  opencode: '/assets/ecosystem-compatibility/opencode.svg',
};

function EcosystemProductIcon({ productId, size }: {
  productId: EcosystemProductId;
  size: number;
}) {
  const source = PRODUCT_ICON_SOURCES[productId];
  return (
    <img
      className="ecosystem-compatibility__brand-image"
      src={source}
      alt=""
      width={size}
      height={size}
      draggable={false}
    />
  );
}

const IMPORT_ITEM_ICONS: Record<EcosystemImportItemKind, IconSource> = {
  account: { glyph: CircleUserRound },
  settings: { name: 'settings' },
  command: { name: 'command-mac' },
  tool: { glyph: Wrench },
  subagent: { glyph: Bot },
  skill: { glyph: Package },
  mcp: { glyph: Server },
  hook: { glyph: Webhook },
  memory: { name: 'thinking' },
  plugin: { name: 'extension' },
  pet: { glyph: PawPrint },
};

const GROUP_ORDER = ['connected', 'available', 'other'] as const;
const PRODUCT_STATUS_TONES: Record<EcosystemProductRuntime['status'], StatusPillTone> = {
  connected: 'success',
  detected: 'info',
  configured: 'success',
  available: 'neutral',
  development: 'neutral',
};

type LoadIssue = 'externalSources' | 'acpClients';
type ImportItemState =
  | 'ready'
  | 'readyRename'
  | 'checking'
  | 'imported'
  | 'reusable'
  | 'adapted'
  | 'notDetected'
  | 'notAdapted'
  | 'unsupportedContext'
  | 'unavailable';

const IMPORT_STATE_TONES: Record<ImportItemState, StatusPillTone> = {
  ready: 'success',
  readyRename: 'success',
  checking: 'info',
  imported: 'success',
  reusable: 'success',
  adapted: 'success',
  notDetected: 'neutral',
  notAdapted: 'neutral',
  unsupportedContext: 'warning',
  unavailable: 'warning',
};

interface AcpSubagentDraft {
  enabled: boolean;
  description: string;
  bestFor: string;
}

const ACP_SUBAGENT_PROFILE_MAX_LENGTH = 320;

function OwnerSurfaceLoading({ label }: { label: string }) {
  return (
    <LoadingState className="ecosystem-compatibility__owner-loading" role="status" size="sm">
      {label}
    </LoadingState>
  );
}

const EcosystemCompatibilityScene: React.FC = () => {
  const { t, formatNumber } = useI18n([
    'scenes/ecosystem-compatibility',
    'settings/external-apps',
    'settings/acp-agents',
  ]);
  const notification = useNotification();
  const { workspace, workspacePath } = useCurrentWorkspace();
  const peerDevice = usePeerDeviceModeOptional();
  const requestSequence = useRef(0);
  const importPlanSequence = useRef(0);
  const importActionSequence = useRef(0);
  const contentRef = useRef<HTMLDivElement>(null);
  const [snapshot, setSnapshot] = useState<ExternalSourceCatalogSnapshot | null>(null);
  const [acpClients, setAcpClients] = useState<AcpClientInfo[]>([]);
  const [loadIssues, setLoadIssues] = useState<LoadIssue[]>([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const selectedProductId = useEcosystemCompatibilityStore((state) => state.selectedProductId);
  const ownerSurface = useEcosystemCompatibilityStore((state) => state.ownerSurface);
  const selectProduct = useEcosystemCompatibilityStore((state) => state.selectProduct);
  const setOwnerSurface = useEcosystemCompatibilityStore((state) => state.setOwnerSurface);
  const [mcpImportPlan, setMcpImportPlan] = useState<ExternalMcpImportPlanV1 | null>(null);
  const [mcpImportPlanState, setMcpImportPlanState] = useState<'idle' | 'loading' | 'ready' | 'failed'>('idle');
  const [importingCandidateId, setImportingCandidateId] = useState<string | null>(null);
  const [editingSubagentClientId, setEditingSubagentClientId] = useState<string | null>(null);
  const [savingSubagentClientId, setSavingSubagentClientId] = useState<string | null>(null);
  const [subagentDraft, setSubagentDraft] = useState<AcpSubagentDraft>({
    enabled: true,
    description: '',
    bestFor: '',
  });

  const loadCompatibility = useCallback(async (forceRefresh: boolean) => {
    const sequence = ++requestSequence.current;
    if (!forceRefresh) setLoading(true);

    const [sourceResult, clientsResult] = await Promise.allSettled([
      externalSourcesAPI.getSnapshot(workspacePath, forceRefresh),
      ACPClientAPI.getClients(),
    ]);
    if (sequence !== requestSequence.current) return;

    const nextIssues: LoadIssue[] = [];
    if (sourceResult.status === 'fulfilled') {
      setSnapshot(sourceResult.value);
    } else {
      nextIssues.push('externalSources');
    }
    if (clientsResult.status === 'fulfilled') {
      setAcpClients(clientsResult.value);
    } else {
      nextIssues.push('acpClients');
    }
    setLoadIssues(nextIssues);
    setLoading(false);
  }, [workspacePath]);

  useEffect(() => {
    setSnapshot(null);
    setAcpClients([]);
    setLoadIssues([]);
    void loadCompatibility(false);
    return () => {
      requestSequence.current += 1;
    };
  }, [loadCompatibility]);

  useEffect(() => {
    const refreshClients = () => {
      void loadCompatibility(false);
    };
    window.addEventListener('openbitfun:acp-clients-changed', refreshClients);
    window.addEventListener('openbitfun:acp-requirements-changed', refreshClients);
    return () => {
      window.removeEventListener('openbitfun:acp-clients-changed', refreshClients);
      window.removeEventListener('openbitfun:acp-requirements-changed', refreshClients);
    };
  }, [loadCompatibility]);

  const productRuntimes = useMemo(
    () => buildEcosystemProductRuntimes(snapshot, acpClients),
    [acpClients, snapshot],
  );
  const selectedRuntime = productRuntimes.find(
    (runtime) => runtime.spec.id === selectedProductId,
  ) ?? productRuntimes[0];
  const importItems = useMemo(
    () => selectedRuntime
      ? buildEcosystemImportItems(snapshot, selectedRuntime)
      : [],
    [selectedRuntime, snapshot],
  );
  const hasMcpImportItems = importItems.some(
    (item) => item.kind === 'mcp' && item.discovered,
  );
  const peerDeviceId = peerDevice?.peerMode.active ? peerDevice.peerMode.deviceId : undefined;
  const externalHostReadOnly = snapshot !== null
    && !snapshot.hostCapabilities.canMutatePolicy
    && !snapshot.hostCapabilities.canManageSources
    && !snapshot.hostCapabilities.canApproveRuntime;
  const mcpImportSupported = !peerDeviceId
    && workspace?.workspaceKind !== WorkspaceKind.Remote
    && !externalHostReadOnly;
  const normalizedSearch = searchQuery.trim().toLowerCase();
  const filteredRuntimes = productRuntimes.filter((runtime) => {
    if (!normalizedSearch) return true;
    const searchable = [
      runtime.spec.name,
      runtime.spec.id,
      runtime.spec.ecosystemId,
      runtime.spec.acpClientId,
      ...runtime.spec.searchTerms,
      ...runtime.capabilityIds,
    ].filter(Boolean).join(' ').toLowerCase();
    return searchable.includes(normalizedSearch);
  });

  useEffect(() => {
    setEditingSubagentClientId(null);
    setSavingSubagentClientId(null);
    if (contentRef.current) contentRef.current.scrollTop = 0;
  }, [selectedProductId]);

  useEffect(() => {
    const sequence = ++importPlanSequence.current;
    setMcpImportPlan(null);

    if (!hasMcpImportItems || !mcpImportSupported) {
      setMcpImportPlanState('idle');
      return undefined;
    }

    setMcpImportPlanState('loading');
    void externalSourcesAPI.planMcpImport(workspacePath || undefined)
      .then((plan) => {
        if (sequence !== importPlanSequence.current) return;
        setMcpImportPlan(plan);
        setMcpImportPlanState('ready');
      })
      .catch(() => {
        if (sequence !== importPlanSequence.current) return;
        setMcpImportPlan(null);
        setMcpImportPlanState('failed');
      });

    return () => {
      importPlanSequence.current += 1;
    };
  }, [hasMcpImportItems, mcpImportSupported, selectedProductId, snapshot?.generation, workspacePath]);

  useEffect(() => {
    importActionSequence.current += 1;
    setImportingCandidateId(null);
  }, [selectedProductId, workspacePath]);

  const showDevelopmentNotice = useCallback((name: string) => {
    notification.info(t('comingSoon.notice', { name }), {
      title: t('comingSoon.title'),
      duration: 3200,
    });
  }, [notification, t]);

  const handleSelectProduct = useCallback((runtime: EcosystemProductRuntime) => {
    selectProduct(runtime.spec.id);
    if (runtime.spec.development) {
      showDevelopmentNotice(runtime.spec.name);
    }
  }, [selectProduct, showDevelopmentNotice]);

  const handleStartAcpClient = useCallback((client: AcpClientInfo) => {
    if (!workspacePath) {
      notification.info(t('run.workspaceRequired'), { duration: 3200 });
      return;
    }
    if (!client.enabled || client.status === 'failed') {
      notification.info(t('run.configurationRequired', { name: client.name || client.id }), {
        duration: 3200,
      });
      setOwnerSurface('acp');
      return;
    }
    window.dispatchEvent(new CustomEvent('openbitfun:create-acp-session', {
      detail: { clientId: client.id },
    }));
    notification.info(t('run.starting', { name: client.name || client.id }), { duration: 2400 });
  }, [notification, setOwnerSurface, t, workspacePath]);

  const handleConfigureSubagent = useCallback((client: AcpClientInfo) => {
    if (!client.subagent) return;
    setSubagentDraft({
      enabled: client.subagent.enabled,
      description: client.subagent.description ?? '',
      bestFor: client.subagent.bestFor ?? '',
    });
    setEditingSubagentClientId(client.id);
  }, []);

  const handleSaveSubagent = useCallback(async (client: AcpClientInfo) => {
    if (!client.subagent || savingSubagentClientId) return;
    setSavingSubagentClientId(client.id);
    try {
      await ACPClientAPI.updateClientSubagentConfig({
        clientId: client.id,
        enabled: subagentDraft.enabled,
        description: subagentDraft.description,
        bestFor: subagentDraft.bestFor,
      });
      notification.success(t('run.subagent.notifications.saved', {
        name: client.name || client.id,
      }));
      setEditingSubagentClientId(null);
      await loadCompatibility(false);
    } catch {
      notification.error(t('run.subagent.notifications.failed', {
        name: client.name || client.id,
      }));
    } finally {
      setSavingSubagentClientId(null);
    }
  }, [loadCompatibility, notification, savingSubagentClientId, subagentDraft, t]);

  const handleImportItem = useCallback(async (item: EcosystemImportItem) => {
    if (!item.candidateId || !mcpImportSupported || !mcpImportPlan) return;
    const planItem = mcpImportPlan.items.find(
      (candidate) => candidate.candidateId === item.candidateId,
    );
    if (!planItem || !['eligible', 'automatic_rename'].includes(planItem.disposition)) return;

    const sequence = ++importActionSequence.current;
    setImportingCandidateId(item.candidateId);
    try {
      const result = await externalSourcesAPI.applyMcpImport(
        workspacePath || undefined,
        mcpImportPlan,
        [{ candidateId: item.candidateId }],
      );
      if (sequence !== importActionSequence.current) return;

      if (result.outcome.status === 'stale') {
        setMcpImportPlan(result.outcome.refreshedPlan);
        notification.info(t('import.notifications.stale'), { duration: 3200 });
        return;
      }

      setMcpImportPlan((current) => current ? {
        ...current,
        items: current.items.map((candidate) => (
          candidate.candidateId === item.candidateId
            ? { ...candidate, disposition: 'already_imported' }
            : candidate
        )),
      } : current);
      notification.success(t('import.notifications.success', { name: item.name }));
      await loadCompatibility(true);
    } catch {
      if (sequence === importActionSequence.current) {
        notification.error(t('import.notifications.failed', { name: item.name }));
      }
    } finally {
      if (sequence === importActionSequence.current) {
        setImportingCandidateId(null);
      }
    }
  }, [loadCompatibility, mcpImportPlan, mcpImportSupported, notification, t, workspacePath]);

  if (!selectedRuntime) return null;

  const discoveredAssetCount = totalDiscoveredAssets(selectedRuntime.capabilityCounts);
  const currentHost = selectedRuntime.executionDomainId
    ?? (workspace?.sshHost
      ? t('host.remote', { name: workspace.sshHost })
      : t('host.local'));
  const adapterLabel = selectedRuntime.adapterRevision
    ? t('header.adapterRevision', { revision: selectedRuntime.adapterRevision })
    : selectedRuntime.spec.acpClientId
      ? t('header.acpRuntime')
      : t('header.notAvailable');
  const headerCheckSummary = t('header.checksSummary', {
    sourceCount: formatNumber(selectedRuntime.sources.length),
    assetCount: formatNumber(discoveredAssetCount),
    runtimeCount: formatNumber(selectedRuntime.acpClients.length),
  });

  const importItemState = (item: EcosystemImportItem): ImportItemState => {
    if (item.support === 'notAdapted') return 'notAdapted';
    if (!item.discovered && item.detection === 'owner') return 'adapted';
    if (!item.discovered) return 'notDetected';
    if (!item.nativeImportSupported) return 'reusable';
    if (!mcpImportSupported) return 'unsupportedContext';
    if (mcpImportPlanState === 'loading' || mcpImportPlanState === 'idle') return 'checking';
    if (mcpImportPlanState === 'failed' || !item.candidateId) return 'unavailable';
    const planItem = mcpImportPlan?.items.find(
      (candidate) => candidate.candidateId === item.candidateId,
    );
    if (planItem?.disposition === 'eligible') return 'ready';
    if (planItem?.disposition === 'automatic_rename') return 'readyRename';
    if (planItem?.disposition === 'already_imported') return 'imported';
    return 'unavailable';
  };

  const renderProductSummary = (runtime: EcosystemProductRuntime): string => {
    const assetCount = totalDiscoveredAssets(runtime.capabilityCounts);
    if (runtime.spec.development) return t('productSummary.development');
    if (assetCount > 0) {
      return t('productSummary.assets', { count: formatNumber(assetCount) });
    }
    if (runtime.acpClients.length > 0) {
      return t('productSummary.acpClients', { count: formatNumber(runtime.acpClients.length) });
    }
    return t('productSummary.available');
  };

  const renderImport = () => (
    <div className="ecosystem-compatibility__view-stack">
      <section className="ecosystem-compatibility__section">
        <div className="ecosystem-compatibility__section-heading">
          <div>
            <h2>{t('import.title')}</h2>
            <p>{t('import.description', { name: selectedRuntime.spec.name })}</p>
          </div>
        </div>
        <div className="ecosystem-compatibility__import-table" role="table" aria-label={t('import.title')}>
          <div className="ecosystem-compatibility__import-row ecosystem-compatibility__import-row--header" role="row">
            <span role="columnheader">{t('import.columns.item')}</span>
            <span role="columnheader">{t('import.columns.source')}</span>
            <span role="columnheader">{t('import.columns.state')}</span>
            <span role="columnheader">{t('import.columns.action')}</span>
          </div>
          {importItems.map((item) => {
            const state = importItemState(item);
            const itemIcon = IMPORT_ITEM_ICONS[item.kind];
            const ready = state === 'ready' || state === 'readyRename';
            const dimmed = [
              'notDetected',
              'notAdapted',
              'unsupportedContext',
              'unavailable',
            ].includes(state);
            const importing = item.candidateId === importingCandidateId;
            const capabilityName = t(`capabilities.${item.kind}`);
            const itemName = item.discovered ? item.name : capabilityName;
            const itemDescription = item.discovered
              ? item.description
              : state === 'notAdapted'
                ? t('import.notAdaptedDescription', {
                  name: selectedRuntime.spec.name,
                  type: capabilityName,
                })
                : state === 'adapted'
                  ? t('import.ownerAdaptedDescription', { type: capabilityName })
                  : t('import.undetectedDescription');
            return (
              <div
                className={`ecosystem-compatibility__import-row${dimmed ? ' is-disabled' : ''}`}
                role="row"
                key={item.id}
                data-import-kind={item.kind}
                data-import-support={item.support}
                data-import-state={state}
                data-import-discovered={item.discovered ? 'true' : 'false'}
              >
                <span className="ecosystem-compatibility__import-item" role="cell">
                  <span className="ecosystem-compatibility__import-item-icon" aria-hidden="true">
                    <Icon {...itemIcon} size="sm" />
                  </span>
                  <span className="ecosystem-compatibility__import-item-copy">
                    <strong><OverflowText>{itemName}</OverflowText></strong>
                    <small>
                      {item.discovered ? <span className="ecosystem-compatibility__import-kind">{capabilityName}</span> : null}
                      {itemDescription}
                    </small>
                  </span>
                </span>
                <span className="ecosystem-compatibility__import-source" role="cell">
                  <span className="ecosystem-compatibility__import-mobile-label" aria-hidden="true">{t('import.columns.source')}</span>
                  <strong><OverflowText>{item.sourceName}</OverflowText></strong>
                  {item.sourceLocation ? <small title={item.sourceLocation}>{item.sourceLocation}</small> : null}
                </span>
                <span role="cell" className="ecosystem-compatibility__import-state">
                  <StatusPill tone={IMPORT_STATE_TONES[state]} title={t(`import.states.${state}`)}>
                    {t(`import.states.${state}`)}
                  </StatusPill>
                </span>
                <span className="ecosystem-compatibility__import-action" role="cell">
                  {!ready && !importing ? (
                    <span className="ecosystem-compatibility__import-action-placeholder" aria-hidden="true">
                      -
                    </span>
                  ) : (
                    <Button
                      className="ecosystem-compatibility__import-button"
                      size="sm"
                      variant="outline"
                      disabled={!ready || importing}
                      loading={importing}
                      aria-label={`${t('import.importAction')} ${itemName}`}
                      onClick={() => void handleImportItem(item)}
                    >
                      {t(importing
                        ? 'import.importingAction'
                        : 'import.importAction')}
                    </Button>
                  )}
                </span>
              </div>
            );
          })}
        </div>
      </section>
    </div>
  );

  const renderRun = () => (
    <div className="ecosystem-compatibility__view-stack">
      <section className="ecosystem-compatibility__section">
        <div className="ecosystem-compatibility__section-heading ecosystem-compatibility__section-heading--actions">
          <div>
            <h2>{t('run.title')}</h2>
            <p>{t('run.description', { name: selectedRuntime.spec.name })}</p>
          </div>
          <Button
            className="ecosystem-compatibility__section-action"
            size="sm"
            variant="outline"
            leadingIcon={<Icon name="settings" />}
            aria-expanded={ownerSurface === 'acp'}
            aria-controls="ecosystem-acp-manager"
            onClick={() => setOwnerSurface(ownerSurface === 'acp' ? null : 'acp')}
          >
            {t(ownerSurface === 'acp' ? 'run.hideManager' : 'run.openManager')}
          </Button>
        </div>
        {selectedRuntime.acpClients.length > 0 ? (
          <div className="ecosystem-compatibility__runtime-list">
            {selectedRuntime.acpClients.map((client) => {
              const profile = client.subagent;
              const profileSupported = profile !== undefined;
              const hasProfile = Boolean(
                profile?.description?.trim() || profile?.bestFor?.trim(),
              );
              const profileState = !profileSupported
                ? 'unsupportedHost'
                : !client.enabled
                  ? 'clientDisabled'
                  : !profile.enabled
                    ? 'disabled'
                    : hasProfile
                      ? 'configured'
                      : 'defaultProfile';
              const editingProfile = editingSubagentClientId === client.id;
              const savingProfile = savingSubagentClientId === client.id;
              const displayName = client.name || client.id;

              return (
                <article className="ecosystem-compatibility__runtime-agent" key={client.id}>
                  <div className="ecosystem-compatibility__runtime-row">
                    <span className="ecosystem-compatibility__runtime-icon" aria-hidden="true">
                      <Icon name="terminal" size="md" />
                    </span>
                    <div className="ecosystem-compatibility__runtime-copy">
                      <strong><OverflowText>{displayName}</OverflowText></strong>
                      <StatusPill tone={client.status === 'failed' ? 'danger' : client.status === 'running' ? 'success' : 'neutral'}>
                        {t(`run.clientStatus.${client.status}`)}
                      </StatusPill>
                    </div>
                    <code><OverflowText>{client.toolName}</OverflowText></code>
                  </div>

                  <div className="ecosystem-compatibility__runtime-mode-grid">
                    <div className="ecosystem-compatibility__runtime-mode">
                      <span className="ecosystem-compatibility__runtime-mode-icon" aria-hidden="true">
                        <Icon name="side-chat" size="md" />
                      </span>
                      <div className="ecosystem-compatibility__runtime-mode-copy">
                        <strong>{t('run.session.title')}</strong>
                        <p>{t('run.session.description', { name: displayName })}</p>
                      </div>
                      <Button
                        className="ecosystem-compatibility__runtime-mode-action"
                        size="sm"
                        variant="primary"
                        leadingIcon={<Icon name="side-chat" />}
                        onClick={() => handleStartAcpClient(client)}
                      >
                        {t('run.startSession')}
                      </Button>
                    </div>

                    <div className="ecosystem-compatibility__runtime-mode">
                      <span className="ecosystem-compatibility__runtime-mode-icon" aria-hidden="true">
                        <Icon glyph={Bot} size="md" />
                      </span>
                      <div className="ecosystem-compatibility__runtime-mode-copy">
                        <div className="ecosystem-compatibility__runtime-mode-title">
                          <strong>{t('run.subagent.title')}</strong>
                          <StatusPill tone={profileState === 'configured' || profileState === 'defaultProfile' ? 'success' : profileState === 'unsupportedHost' ? 'warning' : 'neutral'}>
                            {t(`run.subagent.states.${profileState}`)}
                          </StatusPill>
                        </div>
                        <p>{profile?.description || t('run.subagent.responsibilityFallback')}</p>
                        <small>{profile?.bestFor
                          ? t('run.subagent.bestForSummary', { value: profile.bestFor })
                          : t('run.subagent.bestForFallback')}</small>
                      </div>
                      <Button
                        className="ecosystem-compatibility__runtime-mode-action"
                        size="sm"
                        variant="outline"
                        disabled={!profileSupported}
                        title={!profileSupported
                          ? t('run.subagent.states.unsupportedHost')
                          : undefined}
                        onClick={() => handleConfigureSubagent(client)}
                      >
                        {t(hasProfile
                          ? 'run.subagent.editAction'
                          : 'run.subagent.configureAction')}
                      </Button>
                    </div>
                  </div>

                  {editingProfile ? (
                    <div className="ecosystem-compatibility__subagent-editor">
                      <div className="ecosystem-compatibility__subagent-editor-heading">
                        <div>
                          <strong>{t('run.subagent.editorTitle', { name: displayName })}</strong>
                          <p>{t('run.subagent.editorDescription')}</p>
                        </div>
                        <label className="ecosystem-compatibility__subagent-toggle">
                          <span>{t('run.subagent.enabledLabel')}</span>
                          <Switch
                            checked={subagentDraft.enabled}
                            disabled={savingProfile}
                            aria-label={t('run.subagent.enabledLabel')}
                            onChange={(event) => setSubagentDraft((current) => ({
                              ...current,
                              enabled: event.target.checked,
                            }))}
                          />
                        </label>
                      </div>

                      <div className="ecosystem-compatibility__subagent-fields">
                        <Textarea
                          label={t('run.subagent.responsibilityLabel')}
                          rows={3}
                          maxLength={ACP_SUBAGENT_PROFILE_MAX_LENGTH}
                          value={subagentDraft.description}
                          disabled={savingProfile}
                          placeholder={t('run.subagent.responsibilityPlaceholder')}
                          onChange={(event) => setSubagentDraft((current) => ({
                            ...current,
                            description: event.target.value,
                          }))}
                        />
                        <Textarea
                          label={t('run.subagent.bestForLabel')}
                          rows={3}
                          maxLength={ACP_SUBAGENT_PROFILE_MAX_LENGTH}
                          value={subagentDraft.bestFor}
                          disabled={savingProfile}
                          placeholder={t('run.subagent.bestForPlaceholder')}
                          onChange={(event) => setSubagentDraft((current) => ({
                            ...current,
                            bestFor: event.target.value,
                          }))}
                        />
                      </div>

                      <div className="ecosystem-compatibility__subagent-editor-footer">
                        <span>
                          <Icon name="info" size="sm" aria-hidden="true" />
                          {t('run.subagent.profileNote')}
                        </span>
                        <div>
                          <Button
                            size="sm"
                            variant="fill"
                            disabled={savingProfile}
                            onClick={() => setEditingSubagentClientId(null)}
                          >
                            {t('run.subagent.cancelAction')}
                          </Button>
                          <Button
                            size="sm"
                            variant="primary"
                            loading={savingProfile}
                            onClick={() => void handleSaveSubagent(client)}
                          >
                            {t(savingProfile
                              ? 'run.subagent.savingAction'
                              : 'run.subagent.saveAction')}
                          </Button>
                        </div>
                      </div>
                    </div>
                  ) : null}
                </article>
              );
            })}
          </div>
        ) : (
          <div className="ecosystem-compatibility__empty-card">
            <Icon glyph={Network} size="md" />
            <div>
              <strong>{t('run.notConfiguredTitle', { name: selectedRuntime.spec.name })}</strong>
              <p>{t('run.notConfiguredDescription', { name: selectedRuntime.spec.name })}</p>
            </div>
          </div>
        )}
      </section>
      {ownerSurface === 'acp' ? (
        <section id="ecosystem-acp-manager" className="ecosystem-compatibility__owner-surface" aria-label={t('run.managerLabel')}>
          <div className="ecosystem-compatibility__owner-note">
            <Icon name="info" size="sm" aria-hidden="true" />
            <span>{t('run.managerScope')}</span>
          </div>
          <Suspense fallback={<OwnerSurfaceLoading label={t('run.loadingManager')} />}>
            <AcpAgentsConfig />
          </Suspense>
        </section>
      ) : null}
    </div>
  );

  return (
    <div
      className="ecosystem-compatibility"
      data-testid="ecosystem-compatibility-scene"
      data-openbitfun-scene="ecosystem-compatibility"
      data-openbitfun-part="root"
    >
      <NavigationPanel
        className="ecosystem-compatibility__sidebar"
        aria-label={t('sidebar.label')}
        data-openbitfun-scene="ecosystem-compatibility"
        data-openbitfun-part="sidebar"
      >
        <NavigationPanelHeader className="ecosystem-compatibility__sidebar-header">
          <div className="ecosystem-compatibility__sidebar-title">{t('sidebar.label')}</div>
          <div className="ecosystem-compatibility__search">
            <SearchField
              leadingIcon={<Icon name="search" aria-hidden />}
              size="sm"
              value={searchQuery}
              onValueChange={setSearchQuery}
              placeholder={t('search.placeholder')}
              aria-label={t('search.label')}
            />
          </div>
          <div className="ecosystem-compatibility__compact-picker">
            <Select
              size="sm"
              value={selectedRuntime.spec.id}
              options={productRuntimes.map((runtime) => ({
                value: runtime.spec.id,
                label: runtime.spec.name,
                group: t(`groups.${runtime.group}`),
              }))}
              aria-label={t('sidebar.label')}
              onValueChange={(productId) => {
                const runtime = productRuntimes.find((candidate) => candidate.spec.id === productId);
                if (runtime) handleSelectProduct(runtime);
              }}
            />
          </div>
        </NavigationPanelHeader>

        <NavigationPanelBody>
          <div
            className="ecosystem-compatibility__product-groups"
            data-openbitfun-scene="ecosystem-compatibility"
            data-openbitfun-part="productList"
          >
            <NavigationPanelContent>
              {GROUP_ORDER.map((group) => {
                const runtimes = filteredRuntimes.filter((runtime) => runtime.group === group);
                if (runtimes.length === 0) return null;
                return (
                  <NavigationPanelSection key={group} title={t(`groups.${group}`)}>
                    {runtimes.map((runtime) => {
                      const selected = runtime.spec.id === selectedRuntime.spec.id;
                      return (
                        <NavigationPanelItem
                          key={runtime.spec.id}
                          selected={selected}
                          onClick={() => handleSelectProduct(runtime)}
                          data-product-id={runtime.spec.id}
                          title={`${runtime.spec.name} · ${renderProductSummary(runtime)}`}
                          leading={<EcosystemProductIcon productId={runtime.spec.id} size={22} />}
                          metadata={
                            <StatusPill tone={PRODUCT_STATUS_TONES[runtime.status]}>
                              {t(`status.${runtime.status}`)}
                            </StatusPill>
                          }
                        >
                          {runtime.spec.name}
                        </NavigationPanelItem>
                      );
                    })}
                  </NavigationPanelSection>
                );
              })}
              {filteredRuntimes.length === 0 ? (
                <div className="ecosystem-compatibility__sidebar-empty">
                  {t('search.empty')}
                </div>
              ) : null}
            </NavigationPanelContent>
          </div>
        </NavigationPanelBody>
      </NavigationPanel>

      <main
        className="ecosystem-compatibility__main"
        data-openbitfun-scene="ecosystem-compatibility"
        data-openbitfun-part="main"
      >
        <ScrollArea
          ref={contentRef}
          className="ecosystem-compatibility__content"
          data-openbitfun-scene="ecosystem-compatibility"
          data-openbitfun-part="content"
        >
          <header
            className="ecosystem-compatibility__product-header"
            data-openbitfun-scene="ecosystem-compatibility"
            data-openbitfun-part="header"
          >
            <div className="ecosystem-compatibility__header-top">
              <div className="ecosystem-compatibility__product-identity">
                <span className="ecosystem-compatibility__product-logo" aria-hidden="true">
                  <EcosystemProductIcon productId={selectedRuntime.spec.id} size={38} />
                </span>
                <div>
                  <div className="ecosystem-compatibility__product-title-row">
                    <h1><OverflowText>{selectedRuntime.spec.name}</OverflowText></h1>
                    <StatusPill tone={PRODUCT_STATUS_TONES[selectedRuntime.status]} aria-label={`${t('header.currentState')} · ${t(`status.${selectedRuntime.status}`)}`}>
                      {t(`status.${selectedRuntime.status}`)}
                    </StatusPill>
                  </div>
                  <span className="ecosystem-compatibility__adapter-label">{adapterLabel}</span>
                </div>
              </div>
              <Button
                className="ecosystem-compatibility__header-action"
                size="sm"
                variant="outline"
                leadingIcon={<Icon name={ownerSurface === 'external-sources' ? 'chevron-left' : 'settings'} />}
                aria-expanded={ownerSurface === 'external-sources'}
                aria-controls="ecosystem-source-manager"
                onClick={() => setOwnerSurface(
                  ownerSurface === 'external-sources' ? null : 'external-sources',
                )}
              >
                {ownerSurface === 'external-sources'
                  ? t('governance.closeAction')
                  : t('governance.openAction')}
              </Button>
            </div>
            <dl className="ecosystem-compatibility__product-meta">
              <div>
                <dt>{t('header.sourceLocation')}</dt>
                <dd title={selectedRuntime.sourceLocation}>
                  {selectedRuntime.sourceLocation ?? t('header.notDetected')}
                </dd>
              </div>
              <div>
                <dt>{t('header.executionHost')}</dt>
                <dd>{currentHost}</dd>
              </div>
            </dl>
            <div className="ecosystem-compatibility__header-checks">
              <Icon name="info" size="sm" aria-hidden="true" />
              <span>{t('header.checksLabel')}</span>
              <strong>{headerCheckSummary}</strong>
            </div>
          </header>

          <div className="ecosystem-compatibility__body">
            {loading ? (
              <LoadingState className="ecosystem-compatibility__loading" role="status" size="sm">
                {t('loading')}
              </LoadingState>
            ) : null}
            {loadIssues.length > 0 ? (
              <div className="ecosystem-compatibility__load-notice" role="status">
                <Icon name="info" size="sm" aria-hidden="true" />
                <span>{t('partialLoad', {
                  sources: loadIssues.includes('externalSources') ? t('loadAreas.externalSources') : '',
                  acp: loadIssues.includes('acpClients') ? t('loadAreas.acpClients') : '',
                })}</span>
                <Button className="ecosystem-compatibility__load-retry" variant="outline" size="sm" onClick={() => void loadCompatibility(true)}>
                  {t('retry')}
                </Button>
              </div>
            ) : null}
            {ownerSurface === 'external-sources' ? (
              <section
                id="ecosystem-source-manager"
                className="ecosystem-compatibility__owner-surface"
                aria-label={t('governance.managerLabel')}
              >
                <div className="ecosystem-compatibility__owner-note">
                  <Icon name="info" size="sm" aria-hidden="true" />
                  <span>{t('governance.managerScope')}</span>
                </div>
                <Suspense fallback={<OwnerSurfaceLoading label={t('governance.loadingManager')} />}>
                  <ExternalSourcesConfig
                    presentation="governance"
                    onSnapshotChange={setSnapshot}
                  />
                </Suspense>
              </section>
            ) : (
              <div className="ecosystem-compatibility__unified-stack">
                {selectedRuntime.spec.development ? (
                  <div className="ecosystem-compatibility__development-card" role="status">
                    <Icon glyph={Bot} size="lg" />
                    <div>
                      <strong>{t('comingSoon.title')}</strong>
                      <p>{t('comingSoon.notice', { name: selectedRuntime.spec.name })}</p>
                    </div>
                  </div>
                ) : null}
                {renderRun()}
                {renderImport()}
              </div>
            )}
          </div>
        </ScrollArea>
      </main>
    </div>
  );
};

export default EcosystemCompatibilityScene;

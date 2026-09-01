import { Button, Icon, ScrollArea, SearchField, Switch, Textarea, type IconName, type IconSize } from '@bitfun/ui';
import React, { Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { LucideIcon } from 'lucide-react';
import { Bot, CircleUserRound, MessageSquarePlus, Network, Package, PawPrint, Server, Webhook, Wrench } from 'lucide-react';
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

function catalogLucide(name: IconName): LucideIcon {
  return function CatalogLucide({ size }: { size?: number | string }) {
    const n = typeof size === 'number' ? size : 15;
    const mapped: IconSize = n <= 11 ? '2xs' : n <= 13 ? 'xs' : n <= 15 ? 'sm' : n <= 17 ? 'md' : 'lg';
    return <Icon name={name} size={mapped} />;
  } as LucideIcon;
}

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

const IMPORT_ITEM_ICONS: Record<EcosystemImportItemKind, LucideIcon> = {
  account: CircleUserRound,
  settings: catalogLucide('settings'),
  command: catalogLucide('command-mac'),
  tool: Wrench,
  subagent: Bot,
  skill: Package,
  mcp: Server,
  hook: Webhook,
  memory: catalogLucide('thinking'),
  plugin: catalogLucide('extension'),
  pet: PawPrint,
};

const GROUP_ORDER = ['connected', 'available', 'other'] as const;

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

interface AcpSubagentDraft {
  enabled: boolean;
  description: string;
  bestFor: string;
}

const ACP_SUBAGENT_PROFILE_MAX_LENGTH = 320;

function OwnerSurfaceLoading({ label }: { label: string }) {
  return (
    <div className="ecosystem-compatibility__owner-loading" role="status">
      <Icon name="refresh" size="lg" aria-hidden="true" style={{ width: 15, height: 15 }} />
      <span>{label}</span>
    </div>
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
    window.addEventListener('bitfun:acp-clients-changed', refreshClients);
    window.addEventListener('bitfun:acp-requirements-changed', refreshClients);
    return () => {
      window.removeEventListener('bitfun:acp-clients-changed', refreshClients);
      window.removeEventListener('bitfun:acp-requirements-changed', refreshClients);
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
    window.dispatchEvent(new CustomEvent('bitfun:create-acp-session', {
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
        <div className="ecosystem-compatibility__import-table" role="table">
          <div className="ecosystem-compatibility__import-row ecosystem-compatibility__import-row--header" role="row">
            <span role="columnheader">{t('import.columns.item')}</span>
            <span role="columnheader">{t('import.columns.type')}</span>
            <span role="columnheader">{t('import.columns.source')}</span>
            <span role="columnheader">{t('import.columns.state')}</span>
            <span role="columnheader">{t('import.columns.action')}</span>
          </div>
          {importItems.map((item) => {
            const state = importItemState(item);
            const ItemIcon = IMPORT_ITEM_ICONS[item.kind];
            const ready = state === 'ready' || state === 'readyRename';
            const dimmed = [
              'notDetected',
              'notAdapted',
              'unsupportedContext',
              'unavailable',
            ].includes(state);
            const importing = item.candidateId === importingCandidateId;
            const capabilityName = t(`capabilities.${item.kind}`);
            const itemName = state === 'notDetected'
              ? t('import.undetectedItem', { type: capabilityName })
              : item.discovered
                ? item.name
                : capabilityName;
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
                    <ItemIcon size={15} />
                  </span>
                  <span className="ecosystem-compatibility__import-item-copy">
                    <strong title={itemName}>{itemName}</strong>
                    {itemDescription ? <small title={itemDescription}>{itemDescription}</small> : null}
                  </span>
                </span>
                <span role="cell">{capabilityName}</span>
                <span className="ecosystem-compatibility__import-source" role="cell">
                  <strong title={item.sourceName}>{item.sourceName}</strong>
                  {item.sourceLocation ? <small title={item.sourceLocation}>{item.sourceLocation}</small> : null}
                </span>
                <span role="cell" className={`ecosystem-compatibility__import-state is-${state}`}>
                  {t(`import.states.${state}`)}
                </span>
                <span className="ecosystem-compatibility__import-action" role="cell">
                  {state === 'notAdapted' ? (
                    <span className="ecosystem-compatibility__import-action-placeholder" aria-hidden="true">
                      -
                    </span>
                  ) : (
                    <Button
                      className="ecosystem-compatibility__import-button"
                      size="sm"
                      variant="outline"
                      disabled={!ready || importing}
                      title={t(`import.states.${state}`)}
                      onClick={() => void handleImportItem(item)}
                    >
                      {t(importing
                        ? 'import.importingAction'
                        : ready
                          ? 'import.importAction'
                          : `import.states.${state}`)}
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
                      <strong>{displayName}</strong>
                      <span>{t(`run.clientStatus.${client.status}`)}</span>
                    </div>
                    <code>{client.toolName}</code>
                  </div>

                  <div className="ecosystem-compatibility__runtime-mode-grid">
                    <div className="ecosystem-compatibility__runtime-mode">
                      <span className="ecosystem-compatibility__runtime-mode-icon" aria-hidden="true">
                        <MessageSquarePlus size={16} />
                      </span>
                      <div className="ecosystem-compatibility__runtime-mode-copy">
                        <strong>{t('run.session.title')}</strong>
                        <p>{t('run.session.description', { name: displayName })}</p>
                      </div>
                      <Button
                        className="ecosystem-compatibility__runtime-mode-action"
                        size="sm"
                        variant="fill"
                        onClick={() => handleStartAcpClient(client)}
                      >
                        {t('run.startSession')}
                      </Button>
                    </div>

                    <div className="ecosystem-compatibility__runtime-mode">
                      <span className="ecosystem-compatibility__runtime-mode-icon" aria-hidden="true">
                        <Bot size={16} />
                      </span>
                      <div className="ecosystem-compatibility__runtime-mode-copy">
                        <div className="ecosystem-compatibility__runtime-mode-title">
                          <strong>{t('run.subagent.title')}</strong>
                          <span className={`is-${profileState}`}>
                            {t(`run.subagent.states.${profileState}`)}
                          </span>
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
                        <label>
                          <span>{t('run.subagent.responsibilityLabel')}</span>
                          <Textarea
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
                        </label>
                        <label>
                          <span>{t('run.subagent.bestForLabel')}</span>
                          <Textarea
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
                        </label>
                      </div>

                      <div className="ecosystem-compatibility__subagent-editor-footer">
                        <span>
                          <Icon name="info" size="sm" aria-hidden="true" />
                          {t('run.subagent.profileNote')}
                        </span>
                        <div>
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={savingProfile}
                            onClick={() => setEditingSubagentClientId(null)}
                          >
                            {t('run.subagent.cancelAction')}
                          </Button>
                          <Button
                            size="sm"
                            variant="fill"
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
            <Network size={18} aria-hidden="true" />
            <div>
              <strong>{t('run.notConfiguredTitle', { name: selectedRuntime.spec.name })}</strong>
              <p>{t('run.notConfiguredDescription', { name: selectedRuntime.spec.name })}</p>
            </div>
          </div>
        )}
      </section>
      {ownerSurface === 'acp' ? (
        <section className="ecosystem-compatibility__owner-surface" aria-label={t('run.managerLabel')}>
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
      data-bf-scene="ecosystem-compatibility"
      data-bf-part="root"
    >
      <aside
        className="ecosystem-compatibility__sidebar"
        aria-label={t('sidebar.label')}
        data-bf-scene="ecosystem-compatibility"
        data-bf-part="sidebar"
      >
        <div className="ecosystem-compatibility__sidebar-header">
          <SearchField
            leadingIcon={<Icon name="search" size="lg" aria-hidden />}
            size="sm"
            value={searchQuery}
            onValueChange={setSearchQuery}
            placeholder={t('search.placeholder')}
            aria-label={t('search.label')}
          />
          <button
            type="button"
            className="ecosystem-compatibility__host-card"
            aria-label={t('host.switchLabel')}
            onClick={() => showDevelopmentNotice(t('host.switchAction'))}
          >
            <span>{t('host.label')}</span>
            <strong title={currentHost}>{currentHost}</strong>
            <Icon name="chevron-down" size="sm" aria-hidden="true" />
          </button>
        </div>

        <ScrollArea
          className="ecosystem-compatibility__product-groups"
          data-bf-scene="ecosystem-compatibility"
          data-bf-part="productList"
        >
          {GROUP_ORDER.map((group) => {
            const runtimes = filteredRuntimes.filter((runtime) => runtime.group === group);
            if (runtimes.length === 0) return null;
            return (
              <section className="ecosystem-compatibility__product-group" key={group}>
                <h2>{t(`groups.${group}`)}</h2>
                <div className="ecosystem-compatibility__product-list">
                  {runtimes.map((runtime) => {
                    const selected = runtime.spec.id === selectedRuntime.spec.id;
                    return (
                      <button
                        key={runtime.spec.id}
                        type="button"
                        className={`ecosystem-compatibility__product${selected ? ' is-selected' : ''}`}
                        onClick={() => handleSelectProduct(runtime)}
                        aria-pressed={selected}
                        data-product-id={runtime.spec.id}
                        >
                          <span className="ecosystem-compatibility__product-icon" aria-hidden="true">
                            <EcosystemProductIcon productId={runtime.spec.id} size={22} />
                          </span>
                        <span className="ecosystem-compatibility__product-copy">
                          <strong>{runtime.spec.name}</strong>
                          <small>{renderProductSummary(runtime)}</small>
                        </span>
                        <span className={`ecosystem-compatibility__product-status is-${runtime.status}`}>
                          <span className="ecosystem-compatibility__status-dot" aria-hidden="true" />
                          {t(`status.${runtime.status}`)}
                        </span>
                        <Icon name="chevron-right" size="sm" aria-hidden="true" />
                      </button>
                    );
                  })}
                </div>
              </section>
            );
          })}
          {filteredRuntimes.length === 0 ? (
            <div className="ecosystem-compatibility__sidebar-empty">
              {t('search.empty')}
            </div>
          ) : null}
        </ScrollArea>

        <div className="ecosystem-compatibility__sidebar-footer">
          <Icon name="info" size="sm" aria-hidden="true" />
          <span>{t('sidebar.hint')}</span>
        </div>
      </aside>

      <main
        className="ecosystem-compatibility__main"
        data-bf-scene="ecosystem-compatibility"
        data-bf-part="main"
      >
        <header
          className="ecosystem-compatibility__product-header"
          data-bf-scene="ecosystem-compatibility"
          data-bf-part="header"
        >
          <div className="ecosystem-compatibility__product-identity">
            <span className="ecosystem-compatibility__product-logo" aria-hidden="true">
              <EcosystemProductIcon productId={selectedRuntime.spec.id} size={38} />
            </span>
            <div>
              <div className="ecosystem-compatibility__product-title-row">
                <h1>{selectedRuntime.spec.name}</h1>
                <span>{adapterLabel}</span>
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
                  <dd title={currentHost}>{currentHost}</dd>
                </div>
                <div>
                  <dt>{t('header.currentState')}</dt>
                  <dd className={`is-${selectedRuntime.status}`}>
                    <span className="ecosystem-compatibility__status-dot" aria-hidden="true" />
                    {t(`status.${selectedRuntime.status}`)}
                  </dd>
                </div>
              </dl>
            </div>
          </div>
          <div className="ecosystem-compatibility__header-checks">
            <span>{t('header.checksLabel')}</span>
            <strong title={headerCheckSummary}>{headerCheckSummary}</strong>
            <Button
              className="ecosystem-compatibility__header-action"
              size="sm"
              variant="outline"
              onClick={() => setOwnerSurface(
                ownerSurface === 'external-sources' ? null : 'external-sources',
              )}
            >
              {ownerSurface === 'external-sources'
                ? t('governance.closeAction')
                : t('governance.openAction')}
            </Button>
          </div>
        </header>

        <ScrollArea
          className="ecosystem-compatibility__content"
          data-bf-scene="ecosystem-compatibility"
          data-bf-part="content"
        >
          {loading ? (
            <div className="ecosystem-compatibility__loading" role="status">
              <Icon name="refresh" size="md" aria-hidden="true" />
              {t('loading')}
            </div>
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
                  <Bot size={20} aria-hidden="true" />
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
        </ScrollArea>
      </main>
    </div>
  );
};

export default EcosystemCompatibilityScene;

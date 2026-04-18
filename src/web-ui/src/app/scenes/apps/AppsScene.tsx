/**
 * AppsScene — unified application hub.
 *
 * Layout (centered, max-width 860px):
 *   hero (title + subtitle)
 *   search bar
 *   carousel  ← global featured banner, always visible on home
 *   [Agent App] [Live App] [Bridge App]  ← tab pills below carousel
 *   list  ← simple row list for the selected tab
 *
 * Clicking a row:
 *   Agent App  → in-scene detail page
 *   Live App   → detail modal → open live-app:${id} overlay
 *   Bridge App → coming-soon placeholder
 */
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Bot,
  Box,
  Cable,
  ChevronLeft,
  ChevronRight,
  Cpu,
  FolderPlus,
  LayoutGrid,
  Play,
  Plus,
  Search as SearchIcon,
  Sparkles,
  Square,
  Tag,
  Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Button, ConfirmDialog, Search } from '@/component-library';
import { GalleryDetailModal } from '@/app/components';
import { open } from '@tauri-apps/plugin-dialog';
import { liveAppAPI } from '@/infrastructure/api/service-api/LiveAppAPI';
import type { LiveAppMeta } from '@/infrastructure/api/service-api/LiveAppAPI';
import { useOverlayManager } from '@/app/hooks/useOverlayManager';
import { useOverlayStore } from '@/app/stores/overlayStore';
import type { OverlaySceneId } from '@/app/overlay/types';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import { useGallerySceneAutoRefresh } from '@/app/hooks/useGallerySceneAutoRefresh';
import { getCardGradient, getCardColorRgb } from '@/shared/utils/cardGradients';
import { useAppsStore, type AppsTab } from './appsStore';
import { useAppsData } from './hooks/useAppsData';
import type { AppCardModel } from './hooks/useAppsData';
import { useLiveAppStore } from './live-app/liveAppStore';
import { useLiveAppCatalogSync } from './live-app/hooks/useLiveAppCatalogSync';
import { renderLiveAppIcon, getLiveAppIconGradient } from './live-app/liveAppIcons';
import { ModeAppDetailView, AgentDetailView } from './sections/AgentAppDetailViews';
import './AppsScene.scss';

const log = createLogger('AppsScene');
const TAB_KEYS: AppsTab[] = ['agent-app', 'live-app', 'bridge-app'];

// ─────────────────────────────────────────────────────────────────────────────
// Root
// ─────────────────────────────────────────────────────────────────────────────

const AppsScene: React.FC = () => {
  const { page, selectedAppId, selectedAgentId, openHome, openAppDetail, openAgentDetail } = useAppsStore();
  useLiveAppCatalogSync();

  const {
    availableTools, getAgentById, getAppById,
    getModeConfig, getModeSkills, handleResetTools, handleSetSkills, handleSetTools,
    loadAppsData,
  } = useAppsData(useAppsStore((s) => s.searchQuery));

  useGallerySceneAutoRefresh({ sceneId: 'apps', refetch: () => void loadAppsData() });

  const selectedApp   = useMemo(() => getAppById(selectedAppId),    [getAppById, selectedAppId]);
  const selectedAgent = useMemo(() => getAgentById(selectedAgentId), [getAgentById, selectedAgentId]);

  if (page === 'agent-detail' && selectedAgent) {
    return (
      <AgentDetailView
        agent={selectedAgent}
        app={selectedApp}
        availableTools={availableTools}
        getModeConfig={getModeConfig}
        getModeSkills={getModeSkills}
        onBack={() => selectedApp?.kind === 'mode-app' ? openAppDetail(selectedApp.id) : openHome()}
        handleSetTools={handleSetTools}
        handleResetTools={handleResetTools}
        handleSetSkills={handleSetSkills}
      />
    );
  }
  if (page === 'app-detail' && selectedApp?.kind === 'mode-app') {
    return (
      <ModeAppDetailView
        app={selectedApp}
        onBack={openHome}
        onOpenAgent={(agentId) => openAgentDetail(agentId, selectedApp.id)}
      />
    );
  }

  return <AppsHomeView />;
};

// ─────────────────────────────────────────────────────────────────────────────
// Home view
// ─────────────────────────────────────────────────────────────────────────────

const AppsHomeView: React.FC = () => {
  const { t } = useTranslation('scenes/apps');
  const { activeTab, setActiveTab, searchQuery, setSearchQuery, openAppDetail, openAgentDetail } = useAppsStore();

  const { appCards, loading: agentLoading } = useAppsData(searchQuery);

  // Live App state
  const liveApps         = useLiveAppStore((s) => s.apps);
  const liveLoading      = useLiveAppStore((s) => s.loading);
  const runningWorkerIds = useLiveAppStore((s) => s.runningWorkerIds);
  const setLiveApps      = useLiveAppStore((s) => s.setApps);
  const setLiveLoading   = useLiveAppStore((s) => s.setLoading);
  const setRunningIds    = useLiveAppStore((s) => s.setRunningWorkerIds);
  const markStopped      = useLiveAppStore((s) => s.markWorkerStopped);

  const { workspacePath }    = useCurrentWorkspace();
  const { openOverlay, activeOverlay } = useOverlayManager();

  const [liveSearch, setLiveSearch]           = useState('');
  const [selectedLiveApp, setSelectedLiveApp] = useState<LiveAppMeta | null>(null);
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);

  const runningIdSet = useMemo(() => new Set(runningWorkerIds), [runningWorkerIds]);
  const openTabIds   = useMemo(() => new Set(activeOverlay ? [activeOverlay] : []), [activeOverlay]);

  const filteredLiveApps = useMemo(() => {
    const q = liveSearch.toLowerCase();
    return liveApps.filter((app) =>
      !q ||
      app.name.toLowerCase().includes(q) ||
      app.description.toLowerCase().includes(q) ||
      app.tags.some((tag) => tag.toLowerCase().includes(q)),
    );
  }, [liveApps, liveSearch]);

  // Filtered agent apps
  const filteredAgentApps = useMemo(() => {
    const q = searchQuery.toLowerCase();
    if (!q) return appCards;
    return appCards.filter((app) =>
      app.id.toLowerCase().includes(q) ||
      app.includedAgents.some((a) => a.name.toLowerCase().includes(q)),
    );
  }, [appCards, searchQuery]);

  const handleOpenLiveApp = (appId: string) => {
    setSelectedLiveApp(null);
    openOverlay(`live-app:${appId}` as OverlaySceneId);
  };

  const handleStopLiveApp = async (appId: string) => {
    const overlayId = `live-app:${appId}` as OverlaySceneId;
    try { await liveAppAPI.workerStop(appId); } catch (e) { log.warn('Stop failed', e); }
    finally {
      markStopped(appId);
      if (openTabIds.has(overlayId)) useOverlayStore?.getState().closeOverlay();
    }
  };

  const handleDeleteConfirm = async () => {
    if (!pendingDeleteId) return;
    const appId = pendingDeleteId;
    setPendingDeleteId(null);
    try {
      await liveAppAPI.deleteLiveApp(appId);
      if (selectedLiveApp?.id === appId) setSelectedLiveApp(null);
      setLiveApps(liveApps.filter((a) => a.id !== appId));
      markStopped(appId);
      const overlayId = `live-app:${appId}` as OverlaySceneId;
      if (openTabIds.has(overlayId)) useOverlayStore?.getState().closeOverlay();
    } catch (e) { log.error('Delete failed', e); }
  };

  const handleAddFromFolder = async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: t('liveApp.selectFolderTitle') });
      const path = Array.isArray(selected) ? selected[0] : selected;
      if (!path) return;
      setLiveLoading(true);
      const app = await liveAppAPI.importFromPath(path, workspacePath || undefined);
      setLiveApps([app, ...liveApps]);
      handleOpenLiveApp(app.id);
    } catch (e) { log.error('Import failed', e); }
    finally { setLiveLoading(false); }
  };

  const refetchLive = useCallback(async () => {
    setLiveLoading(true);
    try {
      const [apps, running] = await Promise.all([liveAppAPI.listLiveApps(), liveAppAPI.workerListRunning()]);
      setLiveApps(apps);
      setRunningIds(running);
    } finally { setLiveLoading(false); }
  }, [setLiveApps, setLiveLoading, setRunningIds]);

  useGallerySceneAutoRefresh({ sceneId: 'apps', refetch: refetchLive });

  const effectiveSearch = activeTab === 'live-app' ? liveSearch : searchQuery;
  const onChangeSearch  = activeTab === 'live-app'
    ? (v: string) => setLiveSearch(v)
    : (v: string) => setSearchQuery(v);

  const handleOpenAgentApp = (app: AppCardModel) => {
    if (app.kind === 'mode-app') { openAppDetail(app.id); return; }
    openAgentDetail(app.includedAgents[0].id, app.id);
  };

  return (
    <div className="apps-scene">
      <div className="apps-scene__scroll">

        {/* ── Hero ─────────────────────────────────────────────── */}
        <header className="apps-scene__hero">
          <h1 className="apps-scene__hero-title">{t('hero.title')}</h1>
          <p className="apps-scene__hero-subtitle">{t('hero.subtitle')}</p>
          <div className="apps-scene__hero-toolbar">
            <Search
              className="apps-scene__hero-search"
              value={effectiveSearch}
              onChange={onChangeSearch}
              onClear={() => onChangeSearch('')}
              placeholder={t(`tabs.searchPlaceholder.${activeTab}`)}
              size="large"
              clearable
              prefixIcon={<SearchIcon size={13} />}
            />
          </div>
        </header>

        {/* ── Carousel — global, always on home ─────────────────── */}
        {appCards.length > 0 && !agentLoading && (
          <AppCarousel apps={appCards} onOpenApp={handleOpenAgentApp} />
        )}

        {/* ── Tab pills + list section ───────────────────────────── */}
        <section className="apps-scene__list-section">

          {/* Header: pills (left) + action button (right) */}
          <div className="apps-scene__list-header">
            <nav className="apps-scene__pills" role="tablist" aria-label={t('tabs.label')}>
              {TAB_KEYS.map((tab) => (
                <button
                  key={tab}
                  type="button"
                  role="tab"
                  aria-selected={activeTab === tab}
                  className={`apps-scene__pill${activeTab === tab ? ' is-active' : ''}`}
                  onClick={() => setActiveTab(tab)}
                >
                  {t(`tabs.${tab}`)}
                </button>
              ))}
            </nav>

            {/* Per-tab action button, right-aligned */}
            {activeTab === 'agent-app' && (
              <button type="button" className="apps-scene__list-action" disabled title={t('page.newAgentApp')}>
                <Plus size={14} />
                <span>{t('page.newAgentApp')}</span>
              </button>
            )}
            {activeTab === 'live-app' && (
              <button
                type="button"
                className="apps-scene__list-action"
                onClick={handleAddFromFolder}
                disabled={liveLoading}
                title={t('liveApp.importFromFolder')}
              >
                <FolderPlus size={14} />
                <span>{t('liveApp.importFromFolder')}</span>
              </button>
            )}
            {activeTab === 'bridge-app' && (
              <button type="button" className="apps-scene__list-action" disabled title={t('bridgeApp.comingSoon')}>
                <Plus size={14} />
                <span>{t('page.newBridgeApp')}</span>
              </button>
            )}
          </div>

          {/* Agent App list */}
          {activeTab === 'agent-app' && (
            agentLoading ? (
              <div className="apps-scene__list">
                {[0, 1, 2].map((i) => <div key={i} className="apps-list-row apps-list-row--skeleton" />)}
              </div>
            ) : filteredAgentApps.length === 0 ? (
              <div className="apps-scene__empty">
                <Bot size={28} strokeWidth={1.5} />
                <p>{t('page.empty')}</p>
              </div>
            ) : (
              <div className="apps-scene__list">
                {filteredAgentApps.map((app) => (
                  <AgentAppRow key={app.id} app={app} onOpen={handleOpenAgentApp} />
                ))}
              </div>
            )
          )}

          {/* Live App list */}
          {activeTab === 'live-app' && (
            liveLoading && liveApps.length === 0 ? (
              <div className="apps-scene__list">
                {[0, 1, 2].map((i) => <div key={i} className="apps-list-row apps-list-row--skeleton" />)}
              </div>
            ) : filteredLiveApps.length === 0 ? (
              <div className="apps-scene__empty">
                {liveApps.length === 0
                  ? <><Sparkles size={28} strokeWidth={1.5} /><p>{t('liveApp.empty.generate')}</p></>
                  : <><LayoutGrid size={28} strokeWidth={1.5} /><p>{t('liveApp.empty.noMatch')}</p></>}
              </div>
            ) : (
              <div className="apps-scene__list">
                {filteredLiveApps.map((app) => (
                  <LiveAppRow
                    key={app.id}
                    app={app}
                    isRunning={runningIdSet.has(app.id)}
                    onOpenDetails={setSelectedLiveApp}
                    onOpen={handleOpenLiveApp}
                    onStop={handleStopLiveApp}
                    onDelete={setPendingDeleteId}
                  />
                ))}
              </div>
            )
          )}

          {/* Bridge App placeholder */}
          {activeTab === 'bridge-app' && (
            <div className="apps-scene__bridge-empty">
              <Cable size={40} strokeWidth={1.2} />
              <h3>{t('bridgeApp.title')}</h3>
              <p>{t('bridgeApp.comingSoon')}</p>
            </div>
          )}
        </section>
      </div>

      {/* ── Live App detail modal ──────────────────────────────────── */}
      <GalleryDetailModal
        isOpen={Boolean(selectedLiveApp)}
        onClose={() => setSelectedLiveApp(null)}
        icon={selectedLiveApp ? renderLiveAppIcon(selectedLiveApp.icon || 'box', 24) : <Box size={24} />}
        iconGradient={selectedLiveApp ? getLiveAppIconGradient(selectedLiveApp.icon || 'box') : undefined}
        title={selectedLiveApp?.name ?? ''}
        badges={selectedLiveApp?.category ? <Badge variant="info">{selectedLiveApp.category}</Badge> : null}
        description={selectedLiveApp?.description}
        meta={selectedLiveApp ? <span>v{selectedLiveApp.version}</span> : null}
        actions={selectedLiveApp ? (
          <>
            {runningIdSet.has(selectedLiveApp.id) ? (
              <Button variant="secondary" size="small" onClick={() => void handleStopLiveApp(selectedLiveApp.id)}>
                <Square size={14} />{t('liveApp.detail.stop')}
              </Button>
            ) : null}
            <Button variant="danger" size="small" onClick={() => setPendingDeleteId(selectedLiveApp.id)}>
              <Trash2 size={14} />{t('liveApp.detail.delete')}
            </Button>
            <Button variant="primary" size="small" onClick={() => handleOpenLiveApp(selectedLiveApp.id)}>
              <Play size={14} />{t('liveApp.detail.open')}
            </Button>
          </>
        ) : null}
      >
        {selectedLiveApp?.tags.length ? (
          <div className="apps-scene__detail-tags">
            {selectedLiveApp.tags.map((tag) => (
              <span key={tag} className="apps-scene__detail-tag"><Tag size={11} />{tag}</span>
            ))}
          </div>
        ) : null}
      </GalleryDetailModal>

      <ConfirmDialog
        isOpen={pendingDeleteId !== null}
        onClose={() => setPendingDeleteId(null)}
        onConfirm={handleDeleteConfirm}
        title={t('liveApp.confirmDelete.title', { name: liveApps.find((a) => a.id === pendingDeleteId)?.name ?? '' })}
        message={t('liveApp.confirmDelete.message')}
        type="warning"
        confirmDanger
        confirmText={t('liveApp.confirmDelete.confirm')}
        cancelText={t('liveApp.confirmDelete.cancel')}
      />
    </div>
  );
};

// ─────────────────────────────────────────────────────────────────────────────
// App Carousel  (global featured banner, always on home)
// ─────────────────────────────────────────────────────────────────────────────

const AppCarousel: React.FC<{
  apps: AppCardModel[];
  onOpenApp: (app: AppCardModel) => void;
}> = ({ apps, onOpenApp }) => {
  const { t } = useTranslation('scenes/apps');
  const [active, setActive] = useState(0);
  const [hovered, setHovered] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const count = apps.length;

  const go = useCallback((idx: number) => setActive(((idx % count) + count) % count), [count]);

  useEffect(() => {
    if (hovered || count <= 1) return;
    timerRef.current = setTimeout(() => go(active + 1), 3200);
    return () => { if (timerRef.current) clearTimeout(timerRef.current); };
  }, [active, hovered, go, count]);

  const app = apps[active];
  const gradient   = getCardGradient(app.id);
  const rgb        = getCardColorRgb(app.id);
  const accentColor = `rgb(${rgb})`;
  const Icon = app.kind === 'mode-app' ? Cpu : Bot;

  return (
    <div
      className="app-carousel"
      style={{ '--slide-rgb': rgb, background: gradient } as React.CSSProperties}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <button type="button" className="app-carousel__card" onClick={() => onOpenApp(app)}>
        <div className="app-carousel__left">
          <span className="app-carousel__icon-wrap" style={{ background: `rgba(${rgb},0.18)`, color: accentColor }}>
            <Icon size={28} strokeWidth={1.4} />
          </span>
          <div className="app-carousel__text">
            <span className="app-carousel__name">{t(app.nameKey)}</span>
            <span className="app-carousel__desc">{t(app.descriptionKey)}</span>
          </div>
        </div>
        <span className="app-carousel__badge"
          style={{ background: `rgba(${rgb},0.14)`, color: accentColor, borderColor: `rgba(${rgb},0.28)` }}>
          {t(app.badgeKey)}
        </span>
      </button>

      {count > 1 && (
        <div className="app-carousel__controls">
          <button type="button" className="app-carousel__arrow"
            onClick={(e) => { e.stopPropagation(); go(active - 1); }} aria-label="上一个">
            <ChevronLeft size={14} />
          </button>
          <div className="app-carousel__dots">
            {apps.map((_, i) => (
              <button key={i} type="button"
                className={`app-carousel__dot${i === active ? ' is-active' : ''}`}
                style={i === active ? { background: accentColor } : undefined}
                onClick={(e) => { e.stopPropagation(); go(i); }}
                aria-label={`切换到第 ${i + 1} 项`}
              />
            ))}
          </div>
          <button type="button" className="app-carousel__arrow"
            onClick={(e) => { e.stopPropagation(); go(active + 1); }} aria-label="下一个">
            <ChevronRight size={14} />
          </button>
        </div>
      )}
    </div>
  );
};

// ─────────────────────────────────────────────────────────────────────────────
// Agent App list row
// ─────────────────────────────────────────────────────────────────────────────

const AgentAppRow: React.FC<{ app: AppCardModel; onOpen: (app: AppCardModel) => void }> = ({ app, onOpen }) => {
  const { t } = useTranslation('scenes/apps');
  const Icon = app.kind === 'mode-app' ? Cpu : Bot;

  return (
    <button type="button" className="apps-list-row" onClick={() => onOpen(app)}>
      <span className="apps-list-row__icon apps-list-row__icon--agent"><Icon size={18} /></span>
      <span className="apps-list-row__body">
        <span className="apps-list-row__head">
          <span className="apps-list-row__name">{t(app.nameKey)}</span>
          <Badge variant={app.kind === 'mode-app' ? 'accent' : 'purple'}>{t(app.badgeKey)}</Badge>
        </span>
        <span className="apps-list-row__desc">{t(app.descriptionKey)}</span>
        <span className="apps-list-row__meta">
          {app.kind === 'mode-app'
            ? t('page.containsAgents', { count: app.includedAgents.length })
            : t('page.directAgentDetail')}
        </span>
      </span>
      <span className="apps-list-row__chev"><ChevronRight size={14} /></span>
    </button>
  );
};

// ─────────────────────────────────────────────────────────────────────────────
// Live App list row
// ─────────────────────────────────────────────────────────────────────────────

const LiveAppRow: React.FC<{
  app: LiveAppMeta;
  isRunning: boolean;
  onOpenDetails: (app: LiveAppMeta) => void;
  onOpen: (id: string) => void;
  onStop: (id: string) => Promise<void>;
  onDelete: (id: string) => void;
}> = ({ app, isRunning, onOpenDetails, onOpen, onStop, onDelete }) => {
  const { t } = useTranslation('scenes/apps');

  return (
    <div
      className={`apps-list-row apps-list-row--live${isRunning ? ' is-running' : ''}`}
      onClick={() => onOpenDetails(app)}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && onOpenDetails(app)}
    >
      <span className="apps-list-row__icon apps-list-row__icon--live">
        {renderLiveAppIcon(app.icon || 'box', 18)}
      </span>
      <span className="apps-list-row__body">
        <span className="apps-list-row__head">
          <span className="apps-list-row__name">{app.name}</span>
          {isRunning && <span className="apps-list-row__run-dot" />}
          <span className="apps-list-row__version">v{app.version}</span>
        </span>
        {app.description ? <span className="apps-list-row__desc">{app.description}</span> : null}
      </span>
      <div className="apps-list-row__actions" onClick={(e) => e.stopPropagation()}>
        <button
          type="button"
          className="apps-list-row__action apps-list-row__action--primary"
          onClick={() => onOpen(app.id)}
          title={t('liveApp.card.start')}
        >
          <Play size={13} fill="currentColor" strokeWidth={0} />
        </button>
        {isRunning ? (
          <button type="button" className="apps-list-row__action apps-list-row__action--stop"
            onClick={() => void onStop(app.id)} title={t('liveApp.card.stop')}>
            <Square size={12} />
          </button>
        ) : (
          <button type="button" className="apps-list-row__action apps-list-row__action--danger"
            onClick={() => onDelete(app.id)} title={t('liveApp.card.delete')}>
            <Trash2 size={12} />
          </button>
        )}
      </div>
    </div>
  );
};

export default AppsScene;

import React, { useState, useMemo, useCallback, useEffect } from 'react';
import {
  Box,
  FolderPlus,
  LayoutGrid,
  PackagePlus,
  Play,
  Sparkles,
  Square,
  Tag,
  Trash2,
} from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { useSceneManager } from '@/app/hooks/useSceneManager';
import MiniAppCard from '../components/MiniAppCard';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import {
  miniAppMarketAPI,
  type MarketPackageInspection,
} from '@/infrastructure/api/service-api/MiniAppMarketAPI';
import { createLogger } from '@/shared/utils/logger';
import { Search, ConfirmDialog, Button, Badge } from '@/component-library';
import {
  GalleryDetailModal,
  GalleryEmpty,
  GalleryGrid,
  GalleryLayout,
  GalleryPageHeader,
  GallerySkeleton,
  GalleryZone,
} from '@/app/components';
import type { SceneTabId } from '@/app/components/SceneBar/types';
import { getMiniAppIconGradient, renderMiniAppIcon } from '../utils/miniAppIcons';
import { loadInstalledMarketOrigins } from '../utils/loadInstalledMarketOrigins';
import { pickLocalizedString, pickLocalizedTags } from '../utils/pickLocalizedString';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useMiniAppStore } from '../miniAppStore';
import { useI18n } from '@/infrastructure/i18n';
import { useGallerySceneAutoRefresh } from '@/app/hooks/useGallerySceneAutoRefresh';
import { useNotification } from '@/shared/notification-system';
import './MiniAppGalleryView.scss';

const log = createLogger('MiniAppGalleryView');

const MiniAppGalleryView: React.FC = () => {
  const apps = useMiniAppStore((state) => state.apps);
  const loading = useMiniAppStore((state) => state.loading);
  const runningWorkerIds = useMiniAppStore((state) => state.runningWorkerIds);
  const customizingAppIds = useMiniAppStore((state) => state.customizingAppIds);
  const marketOrigins = useMiniAppStore((state) => state.marketOrigins);
  const setApps = useMiniAppStore((state) => state.setApps);
  const setLoading = useMiniAppStore((state) => state.setLoading);
  const setMarketOrigins = useMiniAppStore((state) => state.setMarketOrigins);
  const setRunningWorkerIds = useMiniAppStore((state) => state.setRunningWorkerIds);
  const markWorkerStopped = useMiniAppStore((state) => state.markWorkerStopped);
  const { workspacePath } = useCurrentWorkspace();
  const notification = useNotification();
  const { openScene, activateScene, closeScene, openTabs } = useSceneManager();
  const { t, currentLanguage } = useI18n('scenes/miniapp');

  const [search, setSearch] = useState('');
  const [categoryFilter, setCategoryFilter] = useState('all');
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);
  const [selectedApp, setSelectedApp] = useState<MiniAppMeta | null>(null);
  const [pendingPackage, setPendingPackage] = useState<{
    path: string;
    inspection: MarketPackageInspection;
  } | null>(null);

  const openTabIds = useMemo(() => new Set(openTabs.map((tab) => tab.id)), [openTabs]);
  const runningIdSet = useMemo(() => new Set(runningWorkerIds), [runningWorkerIds]);
  const customizingIdSet = useMemo(() => new Set(customizingAppIds), [customizingAppIds]);

  const runningApps = useMemo(
    () =>
      runningWorkerIds
        .map((id) => apps.find((app) => app.id === id))
        .filter((app): app is MiniAppMeta => Boolean(app)),
    [runningWorkerIds, apps]
  );

  const categories = useMemo(() => {
    const values = Array.from(new Set(apps.map((app) => app.category).filter(Boolean)));
    return ['all', ...values];
  }, [apps]);

  const filtered = useMemo(() => {
    return apps.filter((app) => {
      const keyword = search.toLowerCase();
      // Search across the localized strings + raw fallback so users can search
      // either the displayed text OR the author's original wording.
      const localizedName = pickLocalizedString(app, currentLanguage, 'name').toLowerCase();
      const localizedDesc = pickLocalizedString(app, currentLanguage, 'description').toLowerCase();
      const localizedTags = pickLocalizedTags(app, currentLanguage).map((t) => t.toLowerCase());
      const matchSearch =
        !search ||
        localizedName.includes(keyword) ||
        localizedDesc.includes(keyword) ||
        app.name.toLowerCase().includes(keyword) ||
        app.description.toLowerCase().includes(keyword) ||
        localizedTags.some((tag) => tag.includes(keyword)) ||
        app.tags.some((tag) => tag.toLowerCase().includes(keyword));
      const matchCategory = categoryFilter === 'all' || app.category === categoryFilter;
      return matchSearch && matchCategory;
    });
  }, [apps, search, categoryFilter, currentLanguage]);

  const handleOpenApp = useCallback(
    (appId: string) => {
      setSelectedApp(null);
      const tabId: SceneTabId = `miniapp:${appId}`;
      if (openTabIds.has(tabId)) {
        activateScene(tabId);
      } else {
        openScene(tabId);
      }
    },
    [openTabIds, activateScene, openScene]
  );

  const handleStopRunning = useCallback(
    async (appId: string) => {
      const tabId: SceneTabId = `miniapp:${appId}`;
      try {
        await miniAppAPI.workerStop(appId);
      } catch (error) {
        log.warn('Stop worker failed, removing local running state', error);
      } finally {
        markWorkerStopped(appId);
        if (openTabIds.has(tabId)) {
          closeScene(tabId);
        }
      }
    },
    [markWorkerStopped, closeScene, openTabIds]
  );

  const handleDeleteRequest = (appId: string) => {
    setPendingDeleteId(appId);
  };

  const handleDeleteConfirm = async () => {
    if (!pendingDeleteId) return;
    const appId = pendingDeleteId;
    setPendingDeleteId(null);
    try {
      await miniAppAPI.deleteMiniApp(appId);
      if (selectedApp?.id === appId) {
        setSelectedApp(null);
      }
      setApps(apps.filter((app) => app.id !== appId));
      markWorkerStopped(appId);
      const tabId: SceneTabId = `miniapp:${appId}`;
      if (openTabIds.has(tabId)) {
        closeScene(tabId);
      }
    } catch (error) {
      log.error('Delete failed', error);
    }
  };

  const refetchMiniAppGallery = useCallback(async () => {
    setLoading(true);
    try {
      const [refreshed, running, origins] = await Promise.all([
        miniAppAPI.listMiniApps(),
        miniAppAPI.workerListRunning(),
        loadInstalledMarketOrigins(),
      ]);
      setApps(refreshed);
      setRunningWorkerIds(running);
      setMarketOrigins(origins);
    } catch (error) {
      log.error('Failed to refresh miniapp gallery', error);
    } finally {
      setLoading(false);
    }
  }, [setApps, setLoading, setMarketOrigins, setRunningWorkerIds]);

  useGallerySceneAutoRefresh({
    sceneId: 'miniapps',
    refetch: refetchMiniAppGallery,
  });

  const handleAddFromFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('selectFolderTitle'),
      });
      const path = Array.isArray(selected) ? selected[0] : selected;
      if (!path) return;

      setLoading(true);
      const app = await miniAppAPI.importFromPath(path, workspacePath || undefined);
      setApps([app, ...apps]);
      handleOpenApp(app.id);
    } catch (error) {
      log.error('Import from folder failed', error);
    } finally {
      setLoading(false);
    }
  };

  const preparePackageImport = useCallback(async (path: string) => {
    if (!path.toLowerCase().endsWith('.bfminiapp')) return;
    try {
      const inspection = await miniAppMarketAPI.inspectPackage(path);
      setPendingPackage({ path, inspection });
    } catch (error) {
      log.error('Inspect MiniApp package failed', error);
      notification.error(t('market.import.invalid', { error: String(error) }));
    }
  }, [notification, t]);

  const handleAddPackage = async () => {
    const selected = await open({
      directory: false,
      multiple: false,
      title: t('market.import.choose'),
      filters: [{ name: t('market.import.packageFile'), extensions: ['bfminiapp'] }],
    });
    const path = Array.isArray(selected) ? selected[0] : selected;
    if (path) await preparePackageImport(path);
  };

  useEffect(() => {
    let stop: (() => void) | undefined;
    let cancelled = false;
    void import('@tauri-apps/api/webview')
      .then(({ getCurrentWebview }) =>
        getCurrentWebview().onDragDropEvent((event) => {
          if (cancelled || event.payload.type !== 'drop') return;
          const packagePath = event.payload.paths.find((path) =>
            path.toLowerCase().endsWith('.bfminiapp'),
          );
          if (packagePath) void preparePackageImport(packagePath);
        }))
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
        } else {
          stop = unlisten;
        }
      })
      .catch((error) => log.warn('MiniApp package drag-and-drop unavailable', error));
    return () => {
      cancelled = true;
      stop?.();
    };
  }, [preparePackageImport]);

  const handlePackageImportConfirm = async () => {
    if (!pendingPackage) return;
    const selected = pendingPackage;
    setPendingPackage(null);
    setLoading(true);
    try {
      const app = await miniAppMarketAPI.importPackage(selected.path, true);
      setApps([app, ...apps]);
      notification.success(t('market.import.imported', { name: app.name }));
      handleOpenApp(app.id);
    } catch (error) {
      log.error('Import MiniApp package failed', error);
      notification.error(t('market.import.failed', { error: String(error) }));
    } finally {
      setLoading(false);
    }
  };

  const renderGrid = () => {
    if (loading && apps.length === 0) {
      return <GallerySkeleton count={8} cardHeight={152} />;
    }

    if (filtered.length === 0) {
      return (
        <GalleryEmpty
          icon={
            apps.length === 0
              ? <Sparkles size={36} strokeWidth={1.2} />
              : <LayoutGrid size={36} strokeWidth={1.2} />
          }
          message={apps.length === 0
            ? t('empty.generate')
            : t('empty.noMatch')}
        />
      );
    }

    return (
      <GalleryGrid minCardWidth={360}>
        {filtered.map((app, index) => (
          <MiniAppCard
            key={app.id}
            app={app}
            index={index}
            isRunning={runningIdSet.has(app.id)}
            isCustomizing={customizingIdSet.has(app.id)}
            marketReleaseNumber={marketOrigins[app.id]?.releaseNumber}
            onOpenDetails={setSelectedApp}
            onOpen={handleOpenApp}
            onDelete={handleDeleteRequest}
          />
        ))}
      </GalleryGrid>
    );
  };

  return (
    <GalleryLayout data-bf-component="miniapp-gallery-view" data-bf-part="root" className="miniapp-gallery">
      <GalleryPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
        actions={(
          <>
            <Search value={search} onChange={setSearch} placeholder={t('searchPlaceholder')} size="small" />
            <button
              type="button"
              className="gallery-action-btn gallery-action-btn--primary"
              onClick={handleAddFromFolder}
              disabled={loading}
              title={t('importFromFolder')}
            >
              <FolderPlus size={15} />
            </button>
            <button
              type="button"
              className="gallery-action-btn"
              onClick={() => void handleAddPackage()}
              disabled={loading}
              title={t('market.import.action')}
            >
              <PackagePlus size={15} />
            </button>
          </>
        )}
      />

      <div data-bf-component="miniapp-gallery-view" data-bf-part="content" className="gallery-zones">
        <GalleryZone
          title={t('running')}
          tools={runningApps.length > 0 ? <span className="gallery-zone-badge">{runningApps.length}</span> : null}
        >
          {runningApps.length > 0 ? (
            <GalleryGrid minCardWidth={360}>
              {runningApps.map((app, index) => (
                <MiniAppCard
                  key={app.id}
                  app={app}
                  index={index}
                  isRunning
                  isCustomizing={customizingIdSet.has(app.id)}
                  marketReleaseNumber={marketOrigins[app.id]?.releaseNumber}
                  onOpenDetails={setSelectedApp}
                  onOpen={handleOpenApp}
                  onDelete={handleDeleteRequest}
                  onStop={handleStopRunning}
                />
              ))}
            </GalleryGrid>
          ) : (
            <div className="gallery-run-empty">
              {t('noRunningApps')}
            </div>
          )}
        </GalleryZone>

        <GalleryZone
          title={t('allApps')}
          tools={(
            <>
              {categories.length > 1 ? (
                <div data-bf-component="miniapp-gallery-view" data-bf-part="categoryFilters" className="gallery-chip-row">
                  {categories.map((category) => (
                    <button
                      data-bf-component="miniapp-gallery-view"
                      data-bf-part="categoryFilter"
                      key={category}
                      type="button"
                      className={[
                        'gallery-cat-chip',
                        categoryFilter === category && 'gallery-cat-chip--active',
                      ]
                        .filter(Boolean)
                        .join(' ')}
                      onClick={() => setCategoryFilter(category)}
                    >
                      {category === 'all' ? t('all') : category}
                    </button>
                  ))}
                </div>
              ) : null}
              <span className="gallery-zone-count">{t('count', { count: filtered.length })}</span>
            </>
          )}
        >
          {renderGrid()}
        </GalleryZone>
      </div>

      <GalleryDetailModal
        isOpen={Boolean(selectedApp)}
        onClose={() => setSelectedApp(null)}
        icon={selectedApp ? renderMiniAppIcon(selectedApp.icon || 'box', 24) : <Box size={24} />}
        iconGradient={selectedApp ? getMiniAppIconGradient(selectedApp.icon || 'box') : undefined}
        title={selectedApp ? pickLocalizedString(selectedApp, currentLanguage, 'name') : ''}
        badges={selectedApp?.category ? <Badge variant="info">{selectedApp.category}</Badge> : null}
        description={selectedApp ? pickLocalizedString(selectedApp, currentLanguage, 'description') : undefined}
        meta={selectedApp ? (
          <span>v{marketOrigins[selectedApp.id]?.releaseNumber ?? selectedApp.version}</span>
        ) : null}
        actions={selectedApp ? (
          <>
            {runningIdSet.has(selectedApp.id) ? (
              <Button variant="secondary" size="small" onClick={() => void handleStopRunning(selectedApp.id)}>
                <Square size={14} />
                {t('detail.stop')}
              </Button>
            ) : null}
            <Button variant="danger" size="small" onClick={() => setPendingDeleteId(selectedApp.id)}>
              <Trash2 size={14} />
              {t('detail.delete')}
            </Button>
            <Button variant="primary" size="small" onClick={() => handleOpenApp(selectedApp.id)}>
              <Play size={14} />
              {t('detail.open')}
            </Button>
          </>
        ) : null}
      >
        {selectedApp ? (() => {
          const detailTags = pickLocalizedTags(selectedApp, currentLanguage);
          return detailTags.length ? (
            <div data-bf-component="miniapp-gallery-view" data-bf-part="detailTags" className="miniapp-gallery__detail-tags">
              {detailTags.map((tag) => (
                <span key={tag} className="miniapp-gallery__detail-tag">
                  <Tag size={11} />
                  {tag}
                </span>
              ))}
            </div>
          ) : null;
        })() : null}
      </GalleryDetailModal>

      <ConfirmDialog
        isOpen={pendingDeleteId !== null}
        onClose={() => setPendingDeleteId(null)}
        onConfirm={handleDeleteConfirm}
        title={t('confirmDelete.title', { name: apps.find((app) => app.id === pendingDeleteId)?.name ?? '' })}
        message={t('confirmDelete.message')}
        type="warning"
        confirmDanger
        confirmText={t('confirmDelete.confirm')}
        cancelText={t('confirmDelete.cancel')}
      />

      <ConfirmDialog
        isOpen={pendingPackage !== null}
        onClose={() => setPendingPackage(null)}
        onConfirm={() => void handlePackageImportConfirm()}
        title={t('market.import.confirmTitle', {
          name: pendingPackage?.inspection.name ?? '',
        })}
        message={t('market.import.confirmMessage', {
          description: pendingPackage?.inspection.description ?? '',
          permissions: [
            ...(pendingPackage?.inspection.permissionDiff.added ?? []),
            ...(pendingPackage?.inspection.permissionDiff.expanded ?? []),
          ].join(', ') || t('market.detail.noPermissions'),
          sha256: pendingPackage?.inspection.packageSha256 ?? '',
        })}
        type="warning"
        confirmText={t('market.import.confirm')}
        cancelText={t('confirmDelete.cancel')}
      />
    </GalleryLayout>
  );
};

export default MiniAppGalleryView;

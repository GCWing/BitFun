import React, { useState, useMemo, useCallback } from 'react';
import {
  Activity,
  ChevronDown,
  FolderPlus,
  LayoutGrid,
  Play,
  RefreshCw,
  Sparkles,
  Square,
} from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { useToolboxStore } from '../toolboxStore';
import { useSceneManager } from '@/app/hooks/useSceneManager';
import MiniAppCard from '../components/MiniAppCard';
import type { MiniAppMeta } from '@/infrastructure/api/service-api/MiniAppAPI';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { createLogger } from '@/shared/utils/logger';
import { Search, Empty, ConfirmDialog } from '@/component-library';
import type { SceneTabId } from '@/app/components/SceneBar/types';
import './GalleryView.scss';

const log = createLogger('GalleryView');

const GalleryView: React.FC = () => {
  const apps = useToolboxStore((s) => s.apps);
  const loading = useToolboxStore((s) => s.loading);
  const runningWorkerIds = useToolboxStore((s) => s.runningWorkerIds);
  const setApps = useToolboxStore((s) => s.setApps);
  const setLoading = useToolboxStore((s) => s.setLoading);
  const markWorkerStopped = useToolboxStore((s) => s.markWorkerStopped);
  const { openScene, activateScene, closeScene, openTabs } = useSceneManager();

  const [search, setSearch] = useState('');
  const [categoryFilter, setCategoryFilter] = useState('all');
  const [runningCollapsed, setRunningCollapsed] = useState(false);
  const [pendingDeleteId, setPendingDeleteId] = useState<string | null>(null);

  const openTabIds = useMemo(() => new Set(openTabs.map((tab) => tab.id)), [openTabs]);
  const runningIdSet = useMemo(() => new Set(runningWorkerIds), [runningWorkerIds]);

  const runningApps = useMemo(
    () =>
      runningWorkerIds
        .map((id) => apps.find((app) => app.id === id))
        .filter((app): app is MiniAppMeta => Boolean(app)),
    [runningWorkerIds, apps]
  );

  const categories = useMemo(() => {
    const cats = Array.from(new Set(apps.map((a) => a.category).filter(Boolean)));
    return ['all', ...cats];
  }, [apps]);

  const filtered = useMemo(() => {
    return apps.filter((a) => {
      const keyword = search.toLowerCase();
      const matchSearch =
        !search ||
        a.name.toLowerCase().includes(keyword) ||
        a.description.toLowerCase().includes(keyword) ||
        a.tags.some((t) => t.toLowerCase().includes(keyword));
      const matchCategory = categoryFilter === 'all' || a.category === categoryFilter;
      return matchSearch && matchCategory;
    });
  }, [apps, search, categoryFilter]);

  const handleOpenApp = useCallback(
    (appId: string) => {
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
      setApps(apps.filter((a) => a.id !== appId));
      markWorkerStopped(appId);
      const tabId: SceneTabId = `miniapp:${appId}`;
      if (openTabIds.has(tabId)) {
        closeScene(tabId);
      }
    } catch (error) {
      log.error('Delete failed', error);
    }
  };

  const handleRefresh = async () => {
    setLoading(true);
    try {
      const refreshed = await miniAppAPI.listMiniApps();
      setApps(refreshed);
    } finally {
      setLoading(false);
    }
  };

  const handleAddFromFolder = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: '选择 MiniApp 目录（需包含 meta.json 与 source/）',
      });
      const path = Array.isArray(selected) ? selected[0] : selected;
      if (!path) return;

      setLoading(true);
      const app = await miniAppAPI.importFromPath(path);
      setApps([app, ...apps]);
      handleOpenApp(app.id);
    } catch (error) {
      log.error('Import from folder failed', error);
    } finally {
      setLoading(false);
    }
  };

  const renderGrid = () => {
    if (loading && apps.length === 0) {
      return (
        <div className="toolbox-gallery__skeleton-grid">
          {Array.from({ length: 8 }).map((_, i) => (
            <div
              key={i}
              className="toolbox-gallery__skeleton-card"
              style={{ '--card-index': i } as React.CSSProperties}
            />
          ))}
        </div>
      );
    }

    if (filtered.length === 0) {
      return (
        <div className="toolbox-gallery__empty">
          <Empty
            image={
              apps.length === 0 ? (
                <Sparkles size={36} strokeWidth={1.2} className="toolbox-gallery__empty-icon" />
              ) : (
                <LayoutGrid size={36} strokeWidth={1.2} className="toolbox-gallery__empty-icon" />
              )
            }
            imageSize={48}
            description={
              apps.length === 0
                ? '还没有 MiniApp，和 AI 对话生成第一个吧。'
                : '没有匹配的应用。'
            }
          />
        </div>
      );
    }

    return (
      <div className="toolbox-gallery__grid">
        {filtered.map((app, index) => (
          <MiniAppCard
            key={app.id}
            app={app}
            index={index}
            isRunning={runningIdSet.has(app.id)}
            onOpen={handleOpenApp}
            onDelete={handleDeleteRequest}
          />
        ))}
      </div>
    );
  };

  return (
    <div className="toolbox-gallery">
      {/* Scrollable body */}
      <div className="toolbox-gallery__body">
        <div className="toolbox-gallery__body-inner">
          {/* Hero — big centered title */}
          <div className="toolbox-gallery__hero">
            <h2 className="toolbox-gallery__hero-title">工具箱</h2>
            <p className="toolbox-gallery__hero-sub">统一查看、启动和切换 MiniApp</p>
          </div>

          {/* Search zone — centered row with search + actions, sticky when scrolling */}
          <div className="toolbox-gallery__search-zone">
            <div className="toolbox-gallery__search-inner">
              <Search value={search} onChange={setSearch} placeholder="搜索应用..." size="small" />
              <button
                type="button"
                className="toolbox-gallery__action-btn toolbox-gallery__action-btn--primary"
                onClick={handleAddFromFolder}
                disabled={loading}
                title="从文件夹导入"
              >
                <FolderPlus size={15} />
              </button>
              <button
                type="button"
                className="toolbox-gallery__action-btn"
                onClick={handleRefresh}
                disabled={loading}
                title="刷新列表"
              >
                <RefreshCw
                  size={15}
                  className={loading ? 'toolbox-gallery__spinning' : undefined}
                />
              </button>
            </div>
          </div>

          {/* Zones content — centered max-width container */}
          <div className="toolbox-gallery__zones">

          {/* Running zone — only when apps are running */}
          {runningApps.length > 0 && (
          <section className="toolbox-gallery__zone">
            <div
              className="toolbox-gallery__zone-header toolbox-gallery__zone-header--clickable"
              onClick={() => setRunningCollapsed((v) => !v)}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => e.key === 'Enter' && setRunningCollapsed((v) => !v)}
            >
              <Activity size={13} className="toolbox-gallery__zone-icon toolbox-gallery__zone-icon--running" />
              <span className="toolbox-gallery__zone-label">运行中</span>
              <span className="toolbox-gallery__zone-badge">{runningApps.length}</span>
              <ChevronDown
                size={13}
                className={[
                  'toolbox-gallery__zone-chevron',
                  runningCollapsed && 'toolbox-gallery__zone-chevron--collapsed',
                ]
                  .filter(Boolean)
                  .join(' ')}
              />
            </div>

            {!runningCollapsed && (
              <div className="toolbox-gallery__run-strip">
                {runningApps.map((app) => (
                  <div
                    key={app.id}
                    className="toolbox-gallery__run-tile"
                    onClick={() => handleOpenApp(app.id)}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => e.key === 'Enter' && handleOpenApp(app.id)}
                  >
                    <span className="toolbox-gallery__run-dot" />
                    <span className="toolbox-gallery__run-name">{app.name}</span>
                    {app.category && (
                      <span className="toolbox-gallery__run-cat">{app.category}</span>
                    )}
                    <div className="toolbox-gallery__run-actions">
                      <button
                        type="button"
                        className="toolbox-gallery__run-btn"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleOpenApp(app.id);
                        }}
                        title="打开"
                      >
                        <Play size={12} fill="currentColor" strokeWidth={0} />
                      </button>
                      <button
                        type="button"
                        className="toolbox-gallery__run-btn toolbox-gallery__run-btn--danger"
                        onClick={(e) => {
                          e.stopPropagation();
                          void handleStopRunning(app.id);
                        }}
                        title="停止"
                      >
                        <Square size={11} />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </section>
        )}

        {/* All apps zone */}
        <section className="toolbox-gallery__zone">
          <div className="toolbox-gallery__zone-header">
            <LayoutGrid size={13} className="toolbox-gallery__zone-icon" />
            <span className="toolbox-gallery__zone-label">全部应用</span>
            {categories.length > 1 && (
              <div className="toolbox-gallery__zone-cats">
                {categories.map((cat) => (
                  <button
                    key={cat}
                    type="button"
                    className={[
                      'toolbox-gallery__cat-chip',
                      categoryFilter === cat && 'toolbox-gallery__cat-chip--active',
                    ]
                      .filter(Boolean)
                      .join(' ')}
                    onClick={() => setCategoryFilter(cat)}
                  >
                    {cat === 'all' ? '全部' : cat}
                  </button>
                ))}
              </div>
            )}
            <span className="toolbox-gallery__zone-count">{filtered.length} 个</span>
          </div>

          {renderGrid()}
        </section>
          </div>{/* end __zones */}
        </div>
      </div>

      <ConfirmDialog
        isOpen={pendingDeleteId !== null}
        onClose={() => setPendingDeleteId(null)}
        onConfirm={handleDeleteConfirm}
        title={`删除 "${apps.find((a) => a.id === pendingDeleteId)?.name ?? ''}"？`}
        message="此操作不可撤销，应用及其所有数据将被永久删除。"
        type="warning"
        confirmDanger
        confirmText="删除"
        cancelText="取消"
      />
    </div>
  );
};

export default GalleryView;

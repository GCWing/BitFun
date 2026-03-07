/**
 * ToolboxSection — inline list under the Mini App nav item.
 *
 * Prioritises three things:
 *   - open the Mini App gallery
 *   - quick access to running apps
 *   - visibility for recently opened apps that still have tabs
 */
import React, { useCallback, useEffect } from 'react';
import { FolderPlus, Puzzle, Sparkles, Circle, Square } from 'lucide-react';
import { useSceneManager } from '@/app/hooks/useSceneManager';
import { useToolboxStore } from '@/app/scenes/toolbox/toolboxStore';
import { useMiniAppList } from '@/app/scenes/toolbox/hooks/useMiniAppList';
import { miniAppAPI } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { Tooltip } from '@/component-library';
import type { SceneTabId } from '@/app/components/SceneBar/types';
import './ToolboxSection.scss';

const ToolboxSection: React.FC = () => {
  const { t } = useI18n('common');
  const { openScene, activateScene, closeScene, openTabs } = useSceneManager();
  useMiniAppList();
  const openedAppIds = useToolboxStore((s) => s.openedAppIds);
  const runningWorkerIds = useToolboxStore((s) => s.runningWorkerIds);
  const apps = useToolboxStore((s) => s.apps);
  const markWorkerStopped = useToolboxStore((s) => s.markWorkerStopped);
  const visibleAppIds = Array.from(new Set([...runningWorkerIds, ...openedAppIds]));

  useEffect(() => {
    miniAppAPI.workerListRunning().then((ids) => {
      useToolboxStore.getState().setRunningWorkerIds(ids);
    }).catch(() => {});
  }, []);

  const openTabIds = new Set(openTabs.map((tab) => tab.id));

  const handleOpenGallery = useCallback(() => {
    openScene('toolbox');
  }, [openScene]);

  const runningApps = visibleAppIds.filter((appId) => runningWorkerIds.includes(appId));
  const openedOnlyApps = visibleAppIds.filter((appId) => !runningWorkerIds.includes(appId));

  const handleRowClick = useCallback(
    (appId: string) => {
      const tabId: SceneTabId = `miniapp:${appId}`;
      if (openTabIds.has(tabId)) {
        activateScene(tabId);
      } else {
        openScene(tabId);
      }
    },
    [openTabIds, openScene, activateScene]
  );

  const handleStop = useCallback(
    async (appId: string, e: React.MouseEvent) => {
      e.stopPropagation();
      try {
        await miniAppAPI.workerStop(appId);
        markWorkerStopped(appId);
        const tabId: SceneTabId = `miniapp:${appId}`;
        if (openTabIds.has(tabId)) {
          closeScene(tabId);
        }
      } catch {
        markWorkerStopped(appId);
        const tabId: SceneTabId = `miniapp:${appId}`;
        if (openTabIds.has(tabId)) {
          closeScene(tabId);
        }
      }
    },
    [markWorkerStopped, closeScene, openTabIds]
  );

  return (
    <div className="bitfun-nav-panel__inline-list">
      <div className="bitfun-nav-panel__inline-action-row">
        <button
          type="button"
          className="bitfun-nav-panel__inline-action is-code"
          onClick={handleOpenGallery}
          title={t('nav.toolbox.openGallery')}
        >
          <Sparkles size={12} />
          <span>{t('nav.toolbox.openGallery')}</span>
        </button>
        <button
          type="button"
          className="bitfun-nav-panel__inline-action"
          onClick={handleOpenGallery}
          title={t('nav.toolbox.newMiniApp')}
        >
          <FolderPlus size={12} />
          <span>{t('nav.toolbox.newMiniApp')}</span>
        </button>
      </div>

      {visibleAppIds.length === 0 ? (
        <div className="bitfun-nav-panel__inline-empty">
          {t('nav.toolbox.noApps')}
        </div>
      ) : (
        <>
          {runningApps.length > 0 && (
            <>
              <div className="bitfun-nav-panel__inline-empty bitfun-nav-panel__inline-empty--label">
                {t('nav.toolbox.runningApps')}
              </div>
              {runningApps.map((appId) => {
                const app = apps.find((a) => a.id === appId);
                const name = app?.name ?? appId;
                return (
                  <div
                    key={appId}
                    role="button"
                    tabIndex={0}
                    className="bitfun-nav-panel__inline-item"
                    onClick={() => handleRowClick(appId)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        handleRowClick(appId);
                      }
                    }}
                    title={name}
                  >
                    <Puzzle size={12} className="bitfun-nav-panel__inline-item-icon" />
                    <span className="bitfun-nav-panel__inline-item-label">{name}</span>
                    <Circle
                      size={6}
                      className="bitfun-nav-panel__shell-dot is-running"
                    />
                    <div className="bitfun-nav-panel__inline-item-actions">
                      <Tooltip content={t('nav.toolbox.stop')}>
                        <button
                          type="button"
                          className="bitfun-nav-panel__inline-item-action-btn bitfun-nav-panel__inline-item-action-btn--toolbox"
                          onClick={(e) => handleStop(appId, e)}
                        >
                          <Square size={10} />
                        </button>
                      </Tooltip>
                    </div>
                  </div>
                );
              })}
            </>
          )}

          {openedOnlyApps.length > 0 && (
            <>
              <div className="bitfun-nav-panel__inline-empty bitfun-nav-panel__inline-empty--label">
                {t('nav.toolbox.myApps')}
              </div>
              {openedOnlyApps.map((appId) => {
                const app = apps.find((a) => a.id === appId);
                const name = app?.name ?? appId;
                return (
                  <div
                    key={appId}
                    role="button"
                    tabIndex={0}
                    className="bitfun-nav-panel__inline-item"
                    onClick={() => handleRowClick(appId)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        handleRowClick(appId);
                      }
                    }}
                    title={name}
                  >
                    <Puzzle size={12} className="bitfun-nav-panel__inline-item-icon" />
                    <span className="bitfun-nav-panel__inline-item-label">{name}</span>
                  </div>
                );
              })}
            </>
          )}
        </>
      )}
    </div>
  );
};

export default ToolboxSection;

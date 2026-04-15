/**
 * WelcomeScene — landing page shown on app start inside SceneViewport.
 *
 * Two modes:
 *  - Has workspace: welcome header + new-session shortcuts + workspace switching.
 *  - No workspace: branding + open/create project.
 */

import React, { useState, useCallback, useMemo } from 'react';
import {
  FolderOpen, Clock, FolderPlus, Trash2,
} from 'lucide-react';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { useOverlayStore } from '@/app/stores/overlayStore';
import { useI18n } from '@/infrastructure/i18n';
import { Tooltip } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import type { WorkspaceInfo } from '@/shared/types';
import { getRecentWorkspaceLineParts } from '@/shared/utils/recentWorkspaceDisplay';
import './WelcomeScene.scss';

const log = createLogger('WelcomeScene');

const WelcomeScene: React.FC = () => {
  const { t } = useI18n('common');
  const {
    hasWorkspace, currentWorkspace, recentWorkspaces,
    openWorkspace, switchWorkspace, removeWorkspaceFromRecent,
  } = useWorkspaceContext();
  const closeOverlay = useOverlayStore(s => s.closeOverlay);
  const [isSelecting, setIsSelecting] = useState(false);
  const [welcomeMessageIndex] = useState(
    () => Math.floor(Math.random() * 4),
  );
  const welcomeMessages = useMemo(
    () => [
      t('welcomeScene.messages.message1'),
      t('welcomeScene.messages.message2'),
      t('welcomeScene.messages.message3'),
      t('welcomeScene.messages.message4'),
    ],
    [t],
  );
  const welcomeMessage = welcomeMessages[welcomeMessageIndex % welcomeMessages.length];

  const displayRecentWorkspaces = useMemo(
    () => (hasWorkspace
      ? recentWorkspaces.filter(ws => ws.id !== currentWorkspace?.id)
      : recentWorkspaces
    ).slice(0, 5),
    [hasWorkspace, recentWorkspaces, currentWorkspace?.id],
  );

  const handleOpenFolder = useCallback(async () => {
    try {
      setIsSelecting(true);
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('startup.selectWorkspaceDirectory'),
      });
      if (selected && typeof selected === 'string') {
        await openWorkspace(selected);
        closeOverlay();
      }
    } catch (e) {
      log.error('Failed to open folder', e);
    } finally {
      setIsSelecting(false);
    }
  }, [openWorkspace, closeOverlay, t]);

  const handleNewProject = useCallback(() => {
    window.dispatchEvent(new Event('nav:new-project'));
  }, []);

  const handleSwitchWorkspace = useCallback(async (workspace: WorkspaceInfo) => {
    try {
      await switchWorkspace(workspace);
      closeOverlay();
    } catch (e) {
      log.error('Failed to switch workspace', e);
    }
  }, [switchWorkspace, closeOverlay]);

  const handleRemoveFromRecent = useCallback(async (workspaceId: string) => {
    try {
      await removeWorkspaceFromRecent(workspaceId);
    } catch (e) {
      log.error('Failed to remove workspace from recent', e);
    }
  }, [removeWorkspaceFromRecent]);

  const formatDate = useCallback((dateString: string) => {
    try {
      const date = new Date(dateString);
      const now = new Date();
      const diffMs = Math.abs(now.getTime() - date.getTime());
      const diffDays = Math.ceil(diffMs / (1000 * 60 * 60 * 24));
      if (diffDays <= 1) return t('time.yesterday');
      if (diffDays < 7) return t('startup.daysAgo', { count: diffDays });
      if (diffDays < 30) return t('startup.weeksAgo', { count: Math.ceil(diffDays / 7) });
      return date.toLocaleDateString();
    } catch {
      return '';
    }
  }, [t]);

  return (
    <div className="welcome-scene">
      <div className="welcome-scene__content">
        <div className="welcome-scene__greeting">
          <h1 className="welcome-scene__title">{t('welcomeScene.firstTime.title')}</h1>
          <p className="welcome-scene__greeting-label">{welcomeMessage}</p>
        </div>

        <div className="welcome-scene__divider" />

        <section className="welcome-scene__switch">
          <div className="welcome-scene__switch-header">
            <span className="welcome-scene__section-label">
              <Clock size={12} />
              {t('welcomeScene.recentWorkspaces')}
            </span>
            <div className="welcome-scene__switch-actions">
              <button
                className="welcome-scene__link-btn"
                onClick={() => void handleOpenFolder()}
                disabled={isSelecting}
              >
                <FolderOpen size={12} />
                {t('welcomeScene.openOtherProject')}
              </button>
              <button className="welcome-scene__link-btn" onClick={handleNewProject}>
                <FolderPlus size={12} />
                {t('welcomeScene.newProject')}
              </button>
            </div>
          </div>

          {displayRecentWorkspaces.length > 0 ? (
            <div className="welcome-scene__recent-list">
              {displayRecentWorkspaces.map(ws => {
                const { hostPrefix, folderLabel, tooltip } = getRecentWorkspaceLineParts(ws);
                return (
                <div key={ws.id} className="welcome-scene__recent-row">
                  <Tooltip content={tooltip} placement="right" followCursor>
                    <button
                      type="button"
                      className="welcome-scene__recent-item"
                      onClick={() => { void handleSwitchWorkspace(ws); }}
                    >
                      <FolderOpen size={13} />
                      <span className="welcome-scene__recent-name">
                        {hostPrefix ? (
                          <>
                            <span className="welcome-scene__recent-host">{hostPrefix}</span>
                            <span className="welcome-scene__recent-host-sep" aria-hidden>
                              {' · '}
                            </span>
                          </>
                        ) : null}
                        {folderLabel}
                      </span>
                    </button>
                  </Tooltip>
                  <button
                    type="button"
                    className="welcome-scene__recent-time-btn"
                    title={t('welcomeScene.removeFromRecent')}
                    aria-label={t('welcomeScene.removeFromRecent')}
                    onClick={() => { void handleRemoveFromRecent(ws.id); }}
                  >
                    <span className="welcome-scene__recent-time-btn__label">
                      {formatDate(ws.lastAccessed)}
                    </span>
                    <span className="welcome-scene__recent-time-btn__icon" aria-hidden>
                      <Trash2 size={15} strokeWidth={2} />
                    </span>
                  </button>
                </div>
                );
              })}
            </div>
          ) : (
            <p className="welcome-scene__no-recent">{t('welcomeScene.noRecentWorkspaces')}</p>
          )}
        </section>

      </div>
    </div>
  );
};

export default WelcomeScene;

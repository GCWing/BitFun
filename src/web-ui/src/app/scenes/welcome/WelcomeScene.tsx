/**
 * WelcomeScene — landing page shown on app start inside SceneViewport.
 *
 * Two modes:
 *  - Has workspace: welcome header + new-session shortcuts + workspace switching.
 *  - No workspace: branding + open/create project.
 */

import React, { useState, useCallback, useMemo } from 'react';
import {
  MessageSquare, Users, GitBranch,
  FolderOpen, Clock, FolderPlus,
} from 'lucide-react';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useI18n } from '@/infrastructure/i18n';
import { useGitBasicInfo } from '@/tools/git/hooks/useGitState';
import { FlowChatManager } from '@/flow_chat/services/FlowChatManager';
import { Tooltip } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import type { SceneTabId } from '@/app/components/SceneBar/types';
import type { WorkspaceInfo } from '@/shared/types';
import './WelcomeScene.scss';

const log = createLogger('WelcomeScene');

const WelcomeScene: React.FC = () => {
  const { t } = useI18n('common');
  const {
    hasWorkspace, currentWorkspace, recentWorkspaces,
    workspacePath, openWorkspace, switchWorkspace,
  } = useWorkspaceContext();
  const openScene = useSceneStore(s => s.openScene);
  const { isRepository, currentBranch } = useGitBasicInfo(workspacePath || '');
  const [isSelecting, setIsSelecting] = useState(false);

  const otherWorkspaces = useMemo(
    () => recentWorkspaces
      .filter(ws => ws.id !== currentWorkspace?.id)
      .slice(0, 5),
    [recentWorkspaces, currentWorkspace?.id],
  );

  const handleNewCodeSession = useCallback(async () => {
    try {
      if (hasWorkspace) {
        const flowChatManager = FlowChatManager.getInstance();
        await flowChatManager.createChatSession({});
      }
    } catch (e) {
      log.error('Failed to create code session', e);
    }
    openScene('session' as SceneTabId);
  }, [hasWorkspace, openScene]);

  // Cowork is coming soon — open session scene for now
  const handleNewCoworkSession = useCallback(() => {
    openScene('session' as SceneTabId);
  }, [openScene]);

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
        openScene('session' as SceneTabId);
      }
    } catch (e) {
      log.error('Failed to open folder', e);
    } finally {
      setIsSelecting(false);
    }
  }, [openWorkspace, openScene, t]);

  const handleNewProject = useCallback(() => {
    window.dispatchEvent(new Event('nav:new-project'));
  }, []);

  const handleSwitchWorkspace = useCallback(async (workspace: WorkspaceInfo) => {
    try {
      await switchWorkspace(workspace);
      openScene('session' as SceneTabId);
    } catch (e) {
      log.error('Failed to switch workspace', e);
    }
  }, [switchWorkspace, openScene]);

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

  if (hasWorkspace) {
    return (
      <div className="welcome-scene">
        <div className="welcome-scene__content">

          {/* Greeting */}
          <div className="welcome-scene__greeting">
            <p className="welcome-scene__greeting-label">{t('welcomeScene.welcomeBack')}</p>
            <h1 className="welcome-scene__workspace-title">{currentWorkspace?.name}</h1>
            <div className="welcome-scene__workspace-meta">
              {isRepository && currentBranch && (
                <span className="welcome-scene__meta-tag">
                  <GitBranch size={11} />
                  <span>{currentBranch}</span>
                </span>
              )}
            </div>
          </div>

          <div className="welcome-scene__divider" />

          {/* Session actions */}
          <div className="welcome-scene__sessions">
            <button className="welcome-scene__session-btn" onClick={handleNewCodeSession}>
              <MessageSquare size={16} />
              <div className="welcome-scene__session-btn-text">
                <span className="welcome-scene__session-btn-label">{t('welcomeScene.newCodeSession')}</span>
                <span className="welcome-scene__session-btn-desc">{t('welcomeScene.newCodeSessionDesc')}</span>
              </div>
            </button>

            <button className="welcome-scene__session-btn" onClick={handleNewCoworkSession}>
              <Users size={16} />
              <div className="welcome-scene__session-btn-text">
                <span className="welcome-scene__session-btn-label">
                  {t('welcomeScene.newCoworkSession')}
                </span>
                <span className="welcome-scene__session-btn-desc">{t('welcomeScene.newCoworkSessionDesc')}</span>
              </div>
            </button>
          </div>

          {/* Switch workspace section */}
          <div className="welcome-scene__switch">
            <div className="welcome-scene__switch-header">
              <span className="welcome-scene__section-label">
                <Clock size={12} />
                {t('welcomeScene.recentWorkspaces')}
              </span>
              <div className="welcome-scene__switch-actions">
                <button
                  className="welcome-scene__link-btn"
                  onClick={handleOpenFolder}
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

            {otherWorkspaces.length > 0 ? (
              <div className="welcome-scene__recent-list">
                {otherWorkspaces.map(ws => (
                  <Tooltip key={ws.id} content={ws.rootPath} placement="right" followCursor>
                    <button
                      className="welcome-scene__recent-item"
                      onClick={() => { void handleSwitchWorkspace(ws); }}
                    >
                      <FolderOpen size={13} />
                      <span className="welcome-scene__recent-name">{ws.name}</span>
                      <span className="welcome-scene__recent-time">{formatDate(ws.lastAccessed)}</span>
                    </button>
                  </Tooltip>
                ))}
              </div>
            ) : (
              <p className="welcome-scene__no-recent">{t('welcomeScene.noOtherWorkspaces')}</p>
            )}
          </div>

        </div>
      </div>
    );
  }

  return (
    <div className="welcome-scene welcome-scene--first-time">
      <div className="welcome-scene__content">

        {/* Logo + greeting */}
        <div className="welcome-scene__greeting">
          <div className="welcome-scene__logo">
            <img src="/Logo-ICON.png" alt="BitFun" className="welcome-scene__logo-img" />
          </div>
          <h1 className="welcome-scene__workspace-title">{t('welcomeScene.firstTime.title')}</h1>
          <p className="welcome-scene__greeting-label">{t('welcomeScene.firstTime.subtitle')}</p>
        </div>

        <div className="welcome-scene__divider" />

        {/* Cowork session — available even without a workspace */}
        <div className="welcome-scene__sessions welcome-scene__sessions--single">
          <button className="welcome-scene__session-btn" onClick={handleNewCoworkSession}>
            <Users size={16} />
            <div className="welcome-scene__session-btn-text">
              <span className="welcome-scene__session-btn-label">
                {t('welcomeScene.newCoworkSession')}
              </span>
              <span className="welcome-scene__session-btn-desc">
                {t('welcomeScene.newCoworkSessionDesc')}
              </span>
            </div>
          </button>
        </div>

        {/* Workspace section: hint + open/new actions */}
        <div className="welcome-scene__switch">
          <div className="welcome-scene__switch-header">
            <span className="welcome-scene__section-label">
              <FolderOpen size={12} />
              {t('welcomeScene.firstTime.noWorkspaceHint')}
            </span>
            <div className="welcome-scene__switch-actions">
              <button
                className="welcome-scene__link-btn"
                onClick={handleOpenFolder}
                disabled={isSelecting}
              >
                <FolderOpen size={12} />
                {t('welcomeScene.firstTime.openProject')}
              </button>
              <button className="welcome-scene__link-btn" onClick={handleNewProject}>
                <FolderPlus size={12} />
                {t('welcomeScene.firstTime.newProject')}
              </button>
            </div>
          </div>

          {recentWorkspaces.length > 0 && (
            <div className="welcome-scene__recent-list">
              {recentWorkspaces.slice(0, 5).map(ws => (
                <Tooltip key={ws.id} content={ws.rootPath} placement="right" followCursor>
                  <button
                    className="welcome-scene__recent-item"
                    onClick={() => { void handleSwitchWorkspace(ws); }}
                  >
                    <FolderOpen size={13} />
                    <span className="welcome-scene__recent-name">{ws.name}</span>
                    <span className="welcome-scene__recent-time">{formatDate(ws.lastAccessed)}</span>
                  </button>
                </Tooltip>
              ))}
            </div>
          )}
        </div>

      </div>
    </div>
  );
};

export default WelcomeScene;

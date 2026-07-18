import React, { useEffect, useState, useCallback } from 'react';
import LanguageToggleButton from '../components/LanguageToggleButton';
import { useI18n } from '../i18n';
import {
  RemoteSessionManager,
  WorkspaceInfo,
  RecentWorkspaceEntry,
} from '../services/RemoteSessionManager';

interface WorkspacePageProps {
  sessionMgr: RemoteSessionManager;
  onReady: () => void;
}

const WorkspacePage: React.FC<WorkspacePageProps> = ({ sessionMgr, onReady }) => {
  const { t } = useI18n();
  const [workspaceInfo, setWorkspaceInfo] = useState<WorkspaceInfo | null>(null);
  const [recentWorkspaces, setRecentWorkspaces] = useState<RecentWorkspaceEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [switching, setSwitching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showRecent, setShowRecent] = useState(false);

  const loadWorkspaceInfo = useCallback(async () => {
    try {
      const info = await sessionMgr.getWorkspaceInfo();
      setWorkspaceInfo(info);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }, [sessionMgr]);

  const loadRecentWorkspaces = useCallback(async () => {
    try {
      const list = await sessionMgr.listRecentWorkspaces();
      setRecentWorkspaces(list);
    } catch (e: any) {
      setError(e.message);
    }
  }, [sessionMgr]);

  useEffect(() => {
    loadWorkspaceInfo();
  }, [loadWorkspaceInfo]);

  const handleShowRecent = async () => {
    setShowRecent(true);
    await loadRecentWorkspaces();
  };

  const handleSelectWorkspace = useCallback(async (workspace: RecentWorkspaceEntry) => {
    if (switching) return;
    setSwitching(true);
    setError(null);
    try {
      const result = await sessionMgr.setWorkspace(workspace.path, {
        remoteConnectionId: workspace.remote_connection_id,
        remoteSshHost: workspace.remote_ssh_host,
      });
      if (result.success) {
        await loadWorkspaceInfo();
        setShowRecent(false);
      } else {
        setError(result.error || t('workspace.failedToSetWorkspace'));
      }
    } catch (e: any) {
      setError(e.message);
    } finally {
      setSwitching(false);
    }
  }, [loadWorkspaceInfo, sessionMgr, switching, t]);

  if (loading) {
    return (
      <div className="workspace-page">
        <div className="workspace-page__loading">
          <div className="spinner" />
          <span>{t('workspace.loadingInfo')}</span>
        </div>
      </div>
    );
  }

  return (
    <div className="workspace-page">
      <div className="workspace-page__header">
        <h1>{t('shared.features.workspace')}</h1>
        <LanguageToggleButton />
      </div>

      <div className="workspace-page__content">
        {workspaceInfo?.has_workspace ? (
          <div className="workspace-page__current">
            <div className="workspace-page__current-label">{t('workspace.currentWorkspace')}</div>
            <div className="workspace-page__current-card">
              <div className="workspace-page__project-name">
                {workspaceInfo.project_name || t('workspace.unknownProject')}
              </div>
              <div className="workspace-page__project-path">{workspaceInfo.path}</div>
              {workspaceInfo.git_branch && (
                <div className="workspace-page__git-branch">
                  <svg width="12" height="12" viewBox="0 0 16 16" fill="none"><circle cx="5" cy="4" r="2" stroke="currentColor" strokeWidth="1.3"/><circle cx="11" cy="4" r="2" stroke="currentColor" strokeWidth="1.3"/><circle cx="5" cy="12" r="2" stroke="currentColor" strokeWidth="1.3"/><path d="M5 6V10M11 6V8C11 9.1046 10.1046 10 9 10H5" stroke="currentColor" strokeWidth="1.3"/></svg>
                  {workspaceInfo.git_branch}
                </div>
              )}
            </div>
            <div className="workspace-page__actions">
              <button className="workspace-page__btn workspace-page__btn--primary" onClick={onReady}>
                {t('common.continue')}
              </button>
              <button className="workspace-page__btn workspace-page__btn--secondary" onClick={handleShowRecent}>
                {t('common.switch')}
              </button>
            </div>
          </div>
        ) : (
          <div className="workspace-page__no-workspace">
            <div className="workspace-page__no-workspace-icon">
              <svg width="40" height="40" viewBox="0 0 16 16" fill="none"><path d="M2 4V12C2 12.5523 2.44772 13 3 13H13C13.5523 13 14 12.5523 14 12V6C14 5.44772 13.5523 5 13 5H8L6.5 3H3C2.44772 3 2 3.44772 2 4Z" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round"/></svg>
            </div>
            <div className="workspace-page__no-workspace-text">
              {t('workspace.noWorkspaceOpen')}
            </div>
            <div className="workspace-page__no-workspace-hint">
              {t('workspace.noWorkspaceHint')}
            </div>
            {!showRecent && (
              <button className="workspace-page__btn workspace-page__btn--primary" onClick={handleShowRecent}>
                {t('workspace.selectWorkspace')}
              </button>
            )}
          </div>
        )}

        {showRecent && (
          <div className="workspace-page__recent">
            <div className="workspace-page__recent-label">{t('workspace.recentWorkspaces')}</div>
            {recentWorkspaces.length === 0 ? (
              <div className="workspace-page__recent-empty">
                {t('workspace.noRecentWorkspaces')}
              </div>
            ) : (
              <div className="workspace-page__recent-list">
                {recentWorkspaces.map((ws) => (
                  <button
                    key={`${ws.remote_connection_id ?? 'local'}:${ws.path}`}
                    className="workspace-page__recent-item"
                    onClick={() => handleSelectWorkspace(ws)}
                    disabled={switching}
                  >
                    <div className="workspace-page__recent-item-name">{ws.name}</div>
                    <div className="workspace-page__recent-item-path">{ws.path}</div>
                  </button>
                ))}
              </div>
            )}
            {workspaceInfo?.has_workspace && (
              <button
                className="workspace-page__btn workspace-page__btn--secondary"
                onClick={() => setShowRecent(false)}
              >
                {t('common.cancel')}
              </button>
            )}
          </div>
        )}

        {switching && (
          <div className="workspace-page__switching">
            <div className="spinner spinner--sm" />
            <span>{t('workspace.openingWorkspace')}</span>
          </div>
        )}

        {error && <div className="workspace-page__error">{error}</div>}
      </div>
    </div>
  );
};

export default WorkspacePage;

/**
 * GitScene — Git scene content. Renders view by activeView from gitSceneStore.
 * Left nav is GitNav (registered in nav-registry). Handles not-repo and loading.
 */

import React, { useState, useRef, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { GitBranch, Plus, RefreshCw } from 'lucide-react';
import { useGitSceneStore } from './gitSceneStore';
import { WorkingCopyView, BranchesView, GraphView } from './views';
import { useGitState } from '@/tools/git/hooks';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { IconButton, CubeLoading } from '@/component-library';
import { globalEventBus } from '@/infrastructure/event-bus';
import './GitScene.scss';

interface GitSceneProps {
  workspacePath?: string;
}

const GitScene: React.FC<GitSceneProps> = ({ workspacePath: workspacePathProp }) => {
  const { workspace } = useCurrentWorkspace();
  const workspacePath = workspacePathProp ?? workspace?.rootPath ?? '';
  const { t } = useTranslation('panels/git');
  const activeView = useGitSceneStore((s) => s.activeView);

  const [forceReset, setForceReset] = useState(false);
  const loadingTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  const {
    isRepository,
    isLoading: statusLoading,
    refresh,
  } = useGitState({
    repositoryPath: workspacePath,
    isActive: true,
    refreshOnMount: true,
    layers: ['basic', 'status'],
  });

  const repoLoading = statusLoading && !isRepository;
  const handleRefresh = useCallback(() => refresh({ force: true, layers: ['basic', 'status'], reason: 'manual' }), [refresh]);

  useEffect(() => {
    if (repoLoading || statusLoading) {
      loadingTimeoutRef.current = setTimeout(() => {
        setForceReset(true);
        setTimeout(() => {
          setForceReset(false);
          handleRefresh();
        }, 100);
      }, 10000);
    } else {
      if (loadingTimeoutRef.current) {
        clearTimeout(loadingTimeoutRef.current);
        loadingTimeoutRef.current = null;
      }
    }
    return () => {
      if (loadingTimeoutRef.current) clearTimeout(loadingTimeoutRef.current);
    };
  }, [repoLoading, statusLoading, handleRefresh]);

  const handleInitGitRepository = useCallback(() => {
    globalEventBus.emit('fill-chat-input', { content: t('init.chatPrompt') });
  }, [t]);

  if (!repoLoading && !isRepository) {
    return (
      <div className="bitfun-git-scene bitfun-git-scene--not-repository">
        <div className="bitfun-git-scene__content">
          <div className="bitfun-git-scene__init-container">
            <div className="bitfun-git-scene__init-decoration">
              <div className="bitfun-git-scene__init-line bitfun-git-scene__init-line--dashed" />
              <div className="bitfun-git-scene__init-dot" />
              <div className="bitfun-git-scene__init-line bitfun-git-scene__init-line--solid" />
            </div>
            <div className="bitfun-git-scene__init-card">
              <div className="bitfun-git-scene__init-icon">
                <GitBranch size={24} />
              </div>
              <div className="bitfun-git-scene__init-text">
                <h3>{t('init.title')}</h3>
                <p>{t('init.notRepository')}</p>
              </div>
              <button type="button" className="bitfun-git-scene__init-button" onClick={handleInitGitRepository}>
                <Plus size={14} />
                <span>{t('init.initButton')}</span>
              </button>
            </div>
            <div className="bitfun-git-scene__init-decoration">
              <div className="bitfun-git-scene__init-line bitfun-git-scene__init-line--solid" />
              <div className="bitfun-git-scene__init-dot bitfun-git-scene__init-dot--muted" />
              <div className="bitfun-git-scene__init-line bitfun-git-scene__init-line--dashed" />
            </div>
            <div className="bitfun-git-scene__init-hint">
              <span>{t('init.hint')}</span>
            </div>
          </div>
        </div>
      </div>
    );
  }

  if ((repoLoading || statusLoading) && !forceReset) {
    return (
      <div className="bitfun-git-scene bitfun-git-scene--loading">
        <div className="bitfun-git-scene__content">
          <div className="bitfun-git-scene__loading-actions">
            <IconButton size="xs" variant="ghost" onClick={() => { setForceReset(true); setTimeout(() => { setForceReset(false); handleRefresh(); }, 100); }} tooltip={t('actions.forceRefresh')}>
              <RefreshCw size={14} />
            </IconButton>
          </div>
          <div className="bitfun-git-scene__loading-state">
            <CubeLoading size="medium" text={t('loading.text')} />
            <p className="bitfun-git-scene__loading-hint">{t('loading.hint')}</p>
          </div>
        </div>
      </div>
    );
  }

  const renderView = () => {
    switch (activeView) {
      case 'branches':
        return <BranchesView workspacePath={workspacePath} />;
      case 'graph':
        return <GraphView workspacePath={workspacePath} />;
      case 'working-copy':
      default:
        return <WorkingCopyView workspacePath={workspacePath} />;
    }
  };

  return <div className="bitfun-git-scene">{renderView()}</div>;
};

export default GitScene;

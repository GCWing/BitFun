/**
 * GitNav — scene-specific left-side navigation for the Git scene.
 *
 * Layout: header (title) + repo status (branch, sync) + nav items (working-copy, history, branches, graph).
 */

import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { GitBranch, Layers2, ArrowUp, ArrowDown, RefreshCw } from 'lucide-react';
import { useGitSceneStore, type GitSceneView } from './gitSceneStore';
import { useGitState } from '../../../tools/git/hooks';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { IconButton } from '@/component-library';
import './GitNav.scss';

const NAV_ITEMS: { id: GitSceneView; icon: React.ElementType; labelKey: string }[] = [
  { id: 'working-copy', icon: GitBranch, labelKey: 'tabs.changes' },
  { id: 'branches', icon: Layers2, labelKey: 'tabs.branches' },
  { id: 'graph', icon: Layers2, labelKey: 'tabs.branchGraph' },
];

const GitNav: React.FC = () => {
  const { workspace } = useCurrentWorkspace();
  const workspacePath = workspace?.rootPath ?? '';
  const { t } = useTranslation('panels/git');
  const activeView = useGitSceneStore((s) => s.activeView);
  const setActiveView = useGitSceneStore((s) => s.setActiveView);

  const {
    isRepository,
    currentBranch,
    ahead,
    behind,
    staged,
    unstaged,
    untracked,
    refresh,
  } = useGitState({
    repositoryPath: workspacePath,
    isActive: true,
    refreshOnMount: true,
    layers: ['basic', 'status'],
  });

  const changeCount = (staged?.length ?? 0) + (unstaged?.length ?? 0) + (untracked?.length ?? 0);
  const branchCount = 0; // Will be filled when branches view loads; optional badge

  const handleViewClick = useCallback(
    (view: GitSceneView) => {
      setActiveView(view);
    },
    [setActiveView]
  );

  return (
    <div className="bitfun-git-scene-nav">
      <div className="bitfun-git-scene-nav__header">
        <span className="bitfun-git-scene-nav__title">{t('title')}</span>
      </div>

      {isRepository && (
        <div className="bitfun-git-scene-nav__status">
          <div className="bitfun-git-scene-nav__branch-row">
            <GitBranch size={12} aria-hidden />
            <span className="bitfun-git-scene-nav__branch-name" title={currentBranch ?? undefined}>
              {currentBranch ?? t('common.unknown')}
            </span>
          </div>
          {(ahead > 0 || behind > 0) && (
            <div className="bitfun-git-scene-nav__sync-badges">
              {ahead > 0 && (
                <span title={t('status.ahead')}>
                  <ArrowUp size={10} /> {ahead}
                </span>
              )}
              {behind > 0 && (
                <span title={t('status.behind')}>
                  <ArrowDown size={10} /> {behind}
                </span>
              )}
            </div>
          )}
          <div className="bitfun-git-scene-nav__actions-row">
            <IconButton size="xs" variant="ghost" onClick={() => refresh({ force: true })} tooltip={t('actions.refresh')}>
              <RefreshCw size={14} />
            </IconButton>
          </div>
        </div>
      )}

      <div className="bitfun-git-scene-nav__sections">
        {NAV_ITEMS.map(({ id, icon: Icon, labelKey }) => (
          <button
            key={id}
            type="button"
            className={['bitfun-git-scene-nav__item', activeView === id && 'is-active'].filter(Boolean).join(' ')}
            onClick={() => handleViewClick(id)}
          >
            <span>
              <Icon size={14} style={{ marginRight: 6, verticalAlign: 'middle' }} aria-hidden />
              {t(labelKey)}
            </span>
            {id === 'working-copy' && changeCount > 0 && (
              <span className="bitfun-git-scene-nav__item-badge">({changeCount})</span>
            )}
            {id === 'branches' && branchCount > 0 && (
              <span className="bitfun-git-scene-nav__item-badge">({branchCount})</span>
            )}
          </button>
        ))}
      </div>
    </div>
  );
};

export default GitNav;

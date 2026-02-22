/**
 * WorkspaceHeader — NavPanel top section showing current workspace name.
 *
 * Extracted from StatusBar. Displays folder icon + workspace name.
 * Clicking the card opens a context-menu style dropdown to switch workspace.
 * When no workspace is open, shows a prompt to open one.
 */

import React, { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { FolderOpen, GitBranch, History, FolderSearch, Plus } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useCurrentWorkspace } from '../../../../infrastructure/contexts/WorkspaceContext';
import { useWorkspaceContext } from '../../../../infrastructure/contexts/WorkspaceContext';
import { useI18n } from '../../../../infrastructure/i18n';
import { useGitBasicInfo } from '../../../../tools/git/hooks/useGitState';
import './WorkspaceHeader.scss';

interface WorkspaceHeaderProps {
  className?: string;
}

const WorkspaceHeader: React.FC<WorkspaceHeaderProps> = ({ className = '' }) => {
  const { t } = useI18n('common');
  const { workspaceName, workspacePath } = useCurrentWorkspace();
  const { currentWorkspace, recentWorkspaces, switchWorkspace, openWorkspace } = useWorkspaceContext();
  const { isRepository, currentBranch } = useGitBasicInfo(workspacePath || '');
  const [showMenu, setShowMenu] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const visibleRecentWorkspaces = useMemo(
    () => recentWorkspaces.filter(workspace => workspace.id !== currentWorkspace?.id),
    [recentWorkspaces, currentWorkspace?.id]
  );

  const handleCardClick = useCallback(() => setShowMenu(p => !p), []);

  useEffect(() => {
    if (!showMenu) return;
    const onMouseDown = (e: MouseEvent) => {
      if (!containerRef.current?.contains(e.target as Node)) setShowMenu(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setShowMenu(false);
    };
    document.addEventListener('mousedown', onMouseDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('mousedown', onMouseDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [showMenu]);

  const handleSwitchWorkspace = useCallback(async (workspaceId: string) => {
    const targetWorkspace = recentWorkspaces.find(item => item.id === workspaceId);
    if (!targetWorkspace) return;

    setShowMenu(false);
    try {
      await switchWorkspace(targetWorkspace);
    } catch {}
  }, [recentWorkspaces, switchWorkspace]);

  const handleOpenFolder = useCallback(async () => {
    setShowMenu(false);
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === 'string') {
        await openWorkspace(selected);
      }
    } catch {}
  }, [openWorkspace]);

  // No workspace — show a placeholder card
  if (!workspaceName) {
    return (
      <div ref={containerRef} className={`bitfun-workspace-header ${showMenu ? 'is-expanded' : ''} ${className}`}>
        <button
          type="button"
          className="bitfun-workspace-header__card bitfun-workspace-header__card--empty"
          onClick={handleCardClick}
          aria-expanded={showMenu}
          aria-haspopup="menu"
          aria-label={t('header.switchWorkspace')}
        >
          <div className="bitfun-workspace-header__identity">
            <FolderSearch size={14} className="bitfun-workspace-header__empty-icon" aria-hidden="true" />
            <span className="bitfun-workspace-header__name bitfun-workspace-header__name--muted">
              {t('header.noWorkspaceOpen')}
            </span>
          </div>
        </button>

        <div className="bitfun-workspace-header__menu" role="menu" aria-hidden={!showMenu}>
          <button
            type="button"
            className="bitfun-workspace-header__menu-item"
            role="menuitem"
            onClick={() => { void handleOpenFolder(); }}
          >
            <FolderOpen size={13} aria-hidden="true" />
            <span className="bitfun-workspace-header__menu-item-main">{t('header.openProject')}</span>
          </button>

          {visibleRecentWorkspaces.length > 0 && (
            <>
              <div className="bitfun-workspace-header__menu-section-title">
                <History size={12} aria-hidden="true" />
                <span>{t('header.recentWorkspaces')}</span>
              </div>
              <div className="bitfun-workspace-header__menu-workspaces">
                {visibleRecentWorkspaces.map((workspace) => (
                  <Tooltip key={workspace.id} content={workspace.rootPath} placement="right" followCursor>
                    <button
                      type="button"
                      className="bitfun-workspace-header__menu-item bitfun-workspace-header__menu-item--workspace"
                      role="menuitem"
                      onClick={() => { void handleSwitchWorkspace(workspace.id); }}
                    >
                      <FolderOpen size={13} aria-hidden="true" />
                      <span className="bitfun-workspace-header__menu-item-main">{workspace.name}</span>
                    </button>
                  </Tooltip>
                ))}
              </div>
            </>
          )}
        </div>
      </div>
    );
  }

  const cardButton = (
    <button
      type="button"
      className="bitfun-workspace-header__card"
      onClick={handleCardClick}
      aria-expanded={showMenu}
      aria-haspopup="menu"
      aria-label={t('header.switchWorkspace')}
    >
      <div className="bitfun-workspace-header__identity">
        <span className="bitfun-workspace-header__name">{workspaceName}</span>
        {isRepository && currentBranch && (
          <span className="bitfun-workspace-header__branch">
            <GitBranch size={11} aria-hidden="true" />
            <span>{currentBranch}</span>
          </span>
        )}
      </div>
    </button>
  );

  return (
    <div ref={containerRef} className={`bitfun-workspace-header ${showMenu ? 'is-expanded' : ''} ${className}`}>
      {workspacePath ? (
        <Tooltip content={workspacePath} placement="right" followCursor>
          {cardButton}
        </Tooltip>
      ) : (
        cardButton
      )}

      <div className="bitfun-workspace-header__menu" role="menu" aria-hidden={!showMenu}>
          <div className="bitfun-workspace-header__menu-section-title">
            <History size={12} aria-hidden="true" />
            <span>{t('header.recentWorkspaces')}</span>
          </div>

          {visibleRecentWorkspaces.length === 0 ? (
            <div className="bitfun-workspace-header__menu-empty">
              <span>{t('header.noRecentWorkspaces')}</span>
            </div>
          ) : (
            <div className="bitfun-workspace-header__menu-workspaces">
              {visibleRecentWorkspaces.map((workspace) => (
                <Tooltip key={workspace.id} content={workspace.rootPath} placement="right" followCursor>
                  <button
                    type="button"
                    className="bitfun-workspace-header__menu-item bitfun-workspace-header__menu-item--workspace"
                    role="menuitem"
                    onClick={() => { void handleSwitchWorkspace(workspace.id); }}
                  >
                    <FolderOpen size={13} aria-hidden="true" />
                    <span className="bitfun-workspace-header__menu-item-main">{workspace.name}</span>
                  </button>
                </Tooltip>
              ))}
            </div>
          )}

          <div className="bitfun-workspace-header__menu-divider" />
          <button
            type="button"
            className="bitfun-workspace-header__menu-item bitfun-workspace-header__menu-item--open"
            role="menuitem"
            onClick={() => { void handleOpenFolder(); }}
          >
            <Plus size={13} aria-hidden="true" />
            <span className="bitfun-workspace-header__menu-item-main">{t('header.openProject')}</span>
          </button>
      </div>
    </div>
  );
};

export default WorkspaceHeader;

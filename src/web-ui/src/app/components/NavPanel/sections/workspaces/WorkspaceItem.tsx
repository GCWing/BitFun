import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Folder, FolderOpen, MoreHorizontal, GitBranch, FolderSearch, Plus, ChevronDown, Trash2, RotateCcw } from 'lucide-react';
import { ConfirmDialog, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { useNavSceneStore } from '@/app/stores/navSceneStore';
import { useApp } from '@/app/hooks/useApp';
import { useGitBasicInfo } from '@/tools/git/hooks/useGitState';
import { workspaceAPI, gitAPI } from '@/infrastructure/api';
import { notificationService } from '@/shared/notification-system';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { BranchSelectModal, type BranchSelectResult } from '../../../panels/BranchSelectModal';
import SessionsSection from '../sessions/SessionsSection';
import { WorkspaceKind, type WorkspaceInfo } from '@/shared/types';

interface WorkspaceItemProps {
  workspace: WorkspaceInfo;
  isActive: boolean;
  isSingle?: boolean;
  draggable?: boolean;
  isDragging?: boolean;
  onDragStart?: React.DragEventHandler<HTMLDivElement>;
  onDragEnd?: React.DragEventHandler<HTMLDivElement>;
}

const WorkspaceItem: React.FC<WorkspaceItemProps> = ({
  workspace,
  isActive,
  isSingle = false,
  draggable = false,
  isDragging = false,
  onDragStart,
  onDragEnd,
}) => {
  const { t } = useI18n('common');
  const { setActiveWorkspace, closeWorkspaceById, deleteAssistantWorkspace, resetAssistantWorkspace } = useWorkspaceContext();
  const { switchLeftPanelTab } = useApp();
  const openNavScene = useNavSceneStore(s => s.openNavScene);
  const { isRepository, currentBranch } = useGitBasicInfo(workspace.rootPath);
  const [menuOpen, setMenuOpen] = useState(false);
  const [worktreeModalOpen, setWorktreeModalOpen] = useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [resetDialogOpen, setResetDialogOpen] = useState(false);
  const [isDeletingAssistant, setIsDeletingAssistant] = useState(false);
  const [isResettingWorkspace, setIsResettingWorkspace] = useState(false);
  const [sessionsCollapsed, setSessionsCollapsed] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuAnchorRef = useRef<HTMLDivElement>(null);
  const menuPopoverRef = useRef<HTMLDivElement>(null);
  const [menuPosition, setMenuPosition] = useState<{ top: number; left: number } | null>(null);
  const isNamedAssistantWorkspace =
    workspace.workspaceKind === WorkspaceKind.Assistant &&
    Boolean(workspace.assistantId);
  const isDefaultAssistantWorkspace =
    workspace.workspaceKind === WorkspaceKind.Assistant &&
    !workspace.assistantId;
  const workspaceDisplayName =
    workspace.workspaceKind === WorkspaceKind.Assistant
      ? workspace.identity?.name?.trim() || workspace.name
      : workspace.name;

  const updateMenuPosition = useCallback(() => {
    const anchor = menuAnchorRef.current;
    if (!anchor) return;

    const rect = anchor.getBoundingClientRect();
    const viewportPadding = 8;
    const estimatedWidth = 240;
    const maxLeft = window.innerWidth - estimatedWidth - viewportPadding;

    setMenuPosition({
      top: Math.max(viewportPadding, rect.bottom + 6),
      left: Math.max(viewportPadding, Math.min(rect.left, maxLeft)),
    });
  }, []);

  useEffect(() => {
    if (!menuOpen) return;
    const handleOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      const isInsideTriggerArea = menuRef.current?.contains(target);
      const isInsidePopover = menuPopoverRef.current?.contains(target);
      if (!isInsideTriggerArea && !isInsidePopover) {
        setMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', handleOutside);
    return () => document.removeEventListener('mousedown', handleOutside);
  }, [menuOpen]);

  useEffect(() => {
    if (!menuOpen) return;

    updateMenuPosition();

    const handleViewportChange = () => updateMenuPosition();
    window.addEventListener('resize', handleViewportChange);
    window.addEventListener('scroll', handleViewportChange, true);

    return () => {
      window.removeEventListener('resize', handleViewportChange);
      window.removeEventListener('scroll', handleViewportChange, true);
    };
  }, [menuOpen, updateMenuPosition]);

  const handleActivate = useCallback(async () => {
    if (!isActive) {
      await setActiveWorkspace(workspace.id);
    }
  }, [isActive, setActiveWorkspace, workspace.id]);

  const handleCollapseToggle = useCallback(() => {
    setSessionsCollapsed(prev => !prev);
  }, []);

  const handleCardNameClick = useCallback(async () => {
    if (!isActive) {
      await setActiveWorkspace(workspace.id);
    } else {
      setSessionsCollapsed(prev => !prev);
    }
  }, [isActive, setActiveWorkspace, workspace.id]);

  const handleCloseWorkspace = useCallback(async () => {
    setMenuOpen(false);
    try {
      await closeWorkspaceById(workspace.id);
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.closeFailed'),
        { duration: 4000 }
      );
    }
  }, [closeWorkspaceById, t, workspace.id]);

  const handleRequestDeleteAssistant = useCallback(() => {
    setMenuOpen(false);
    setDeleteDialogOpen(true);
  }, []);

  const handleRequestResetWorkspace = useCallback(() => {
    setMenuOpen(false);
    setResetDialogOpen(true);
  }, []);

  const handleConfirmDeleteAssistant = useCallback(async () => {
    if (!isNamedAssistantWorkspace || isDeletingAssistant) {
      return;
    }

    setIsDeletingAssistant(true);
    try {
      await deleteAssistantWorkspace(workspace.id);
      notificationService.success(t('nav.workspaces.assistantDeleted'), { duration: 2500 });
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.deleteAssistantFailed'),
        { duration: 4000 }
      );
    } finally {
      setIsDeletingAssistant(false);
    }
  }, [deleteAssistantWorkspace, isDeletingAssistant, isNamedAssistantWorkspace, t, workspace.id]);

  const handleConfirmResetWorkspace = useCallback(async () => {
    if (!isDefaultAssistantWorkspace || isResettingWorkspace) {
      return;
    }

    setIsResettingWorkspace(true);
    try {
      await resetAssistantWorkspace(workspace.id);
      await flowChatManager.resetWorkspaceSessions(workspace.rootPath, {
        reinitialize: isActive,
        preferredMode: 'Claw',
      });
      notificationService.success(t('nav.workspaces.workspaceReset'), { duration: 2500 });
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.resetWorkspaceFailed'),
        { duration: 4000 }
      );
    } finally {
      setIsResettingWorkspace(false);
    }
  }, [isActive, isDefaultAssistantWorkspace, isResettingWorkspace, resetAssistantWorkspace, t, workspace.id, workspace.rootPath]);

  const handleReveal = useCallback(async () => {
    setMenuOpen(false);
    try {
      await workspaceAPI.revealInExplorer(workspace.rootPath);
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.revealFailed'),
        { duration: 4000 }
      );
    }
  }, [t, workspace.rootPath]);

  const handleCreateSession = useCallback(async (mode?: 'agentic' | 'Cowork' | 'Claw') => {
    setMenuOpen(false);
    try {
      await flowChatManager.createChatSession(
        {
          workspacePath: workspace.rootPath,
        },
        mode ?? (workspace.workspaceKind === WorkspaceKind.Assistant ? 'Claw' : undefined)
      );
      await setActiveWorkspace(workspace.id);
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.createSessionFailed'),
        { duration: 4000 }
      );
    }
  }, [setActiveWorkspace, t, workspace.id, workspace.rootPath, workspace.workspaceKind]);

  const handleCreateCodeSession = useCallback(() => {
    void handleCreateSession('agentic');
  }, [handleCreateSession]);

  const handleCreateCoworkSession = useCallback(() => {
    void handleCreateSession('Cowork');
  }, [handleCreateSession]);

  const handleCreateWorktree = useCallback(async (result: BranchSelectResult) => {
    try {
      await gitAPI.addWorktree(workspace.rootPath, result.branch, result.isNew);
      notificationService.success(t('nav.workspaces.worktreeCreated'), { duration: 2500 });
    } catch (error) {
      notificationService.error(
        t('nav.workspaces.worktreeCreateFailed', {
          error: error instanceof Error ? error.message : String(error),
        }),
        { duration: 4000 }
      );
    }
  }, [t, workspace.rootPath]);

  const handleOpenFiles = useCallback(async () => {
    try {
      await handleActivate();
      switchLeftPanelTab('files');
      openNavScene('file-viewer');
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.revealFailed'),
        { duration: 4000 }
      );
    }
  }, [handleActivate, openNavScene, switchLeftPanelTab, t]);

  if (workspace.workspaceKind === WorkspaceKind.Assistant) {
    return (
      <div className={[
        'bitfun-nav-panel__assistant-item',
        isActive && 'is-active',
        isDragging && 'is-dragging',
        menuOpen && 'is-menu-open',
        sessionsCollapsed && 'is-sessions-collapsed',
        isSingle && 'is-single',
      ].filter(Boolean).join(' ')}
      aria-grabbed={draggable ? isDragging : undefined}>
        <div
          className="bitfun-nav-panel__assistant-item-card"
          draggable={draggable}
          onDragStart={onDragStart}
          onDragEnd={onDragEnd}
        >
          <button
            type="button"
            className="bitfun-nav-panel__assistant-item-collapse-btn"
            onClick={handleCollapseToggle}
            aria-label={sessionsCollapsed ? t('nav.workspaces.expandSessions') : t('nav.workspaces.collapseSessions')}
            aria-expanded={!sessionsCollapsed}
          >
            <span className="bitfun-nav-panel__assistant-item-avatar" aria-hidden="true">
              <span className="bitfun-nav-panel__assistant-item-avatar-letter">
                {workspaceDisplayName.charAt(0)}
              </span>
              <span className={`bitfun-nav-panel__assistant-item-icon-toggle${sessionsCollapsed ? ' is-collapsed' : ''}`}>
                <ChevronDown size={12} />
              </span>
            </span>
          </button>
          <button
            type="button"
            className="bitfun-nav-panel__assistant-item-name-btn"
            onClick={() => { void handleCardNameClick(); }}
          >
            <span className="bitfun-nav-panel__assistant-item-label">{workspaceDisplayName}</span>
            {isDefaultAssistantWorkspace ? (
              <span
                className="bitfun-nav-panel__assistant-item-badge"
                title={t('nav.workspaces.primaryAssistant')}
              >
                {t('nav.workspaces.primaryAssistant')}
              </span>
            ) : null}
          </button>

          <div className="bitfun-nav-panel__assistant-item-menu" ref={menuRef}>
            <Tooltip content={t('nav.items.project')} placement="right" followCursor>
              <button
                type="button"
                className="bitfun-nav-panel__assistant-item-menu-trigger"
                onClick={() => { void handleOpenFiles(); }}
              >
                <Folder size={14} />
              </button>
            </Tooltip>
            <div ref={menuAnchorRef}>
              <button
                type="button"
                className={`bitfun-nav-panel__assistant-item-menu-trigger${menuOpen ? ' is-open' : ''}`}
                onClick={() => setMenuOpen(prev => !prev)}
              >
                <MoreHorizontal size={14} />
              </button>
            </div>

            {menuOpen && menuPosition && createPortal(
              <div
                ref={menuPopoverRef}
                className="bitfun-nav-panel__workspace-item-menu-popover"
                role="menu"
                style={{ top: `${menuPosition.top}px`, left: `${menuPosition.left}px` }}
              >
                <button type="button" className="bitfun-nav-panel__workspace-item-menu-item" onClick={() => { void handleCreateSession(); }}>
                  <Plus size={13} />
                  <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.newSession')}</span>
                </button>
                {isDefaultAssistantWorkspace && (
                  <button
                    type="button"
                    className="bitfun-nav-panel__workspace-item-menu-item is-danger"
                    onClick={handleRequestResetWorkspace}
                    disabled={isResettingWorkspace}
                  >
                    <RotateCcw size={13} />
                    <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.resetWorkspace')}</span>
                  </button>
                )}
                {isNamedAssistantWorkspace && (
                  <button
                    type="button"
                    className="bitfun-nav-panel__workspace-item-menu-item is-danger"
                    onClick={handleRequestDeleteAssistant}
                    disabled={isDeletingAssistant}
                  >
                    <Trash2 size={13} />
                    <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.deleteAssistant')}</span>
                  </button>
                )}
                <button type="button" className="bitfun-nav-panel__workspace-item-menu-item" onClick={() => { void handleReveal(); }}>
                  <FolderSearch size={13} />
                  <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.reveal')}</span>
                </button>
              </div>,
              document.body
            )}
          </div>
        </div>

        <div className={`bitfun-nav-panel__assistant-item-sessions${sessionsCollapsed ? ' is-collapsed' : ''}`}>
          <SessionsSection
            workspaceId={workspace.id}
            workspacePath={workspace.rootPath}
            isActiveWorkspace={isActive}
          />
        </div>

        <ConfirmDialog
          isOpen={deleteDialogOpen}
          onClose={() => setDeleteDialogOpen(false)}
          onConfirm={() => { void handleConfirmDeleteAssistant(); }}
          title={t('nav.workspaces.deleteAssistantDialog.title', { name: workspaceDisplayName })}
          message={t('nav.workspaces.deleteAssistantDialog.message')}
          confirmText={t('nav.workspaces.actions.deleteAssistant')}
          cancelText={t('actions.cancel')}
          confirmDanger
        />
        <ConfirmDialog
          isOpen={resetDialogOpen}
          onClose={() => setResetDialogOpen(false)}
          onConfirm={() => { void handleConfirmResetWorkspace(); }}
          title={t('nav.workspaces.resetWorkspaceDialog.title', { name: workspaceDisplayName })}
          message={t('nav.workspaces.resetWorkspaceDialog.message')}
          confirmText={t('nav.workspaces.actions.resetWorkspace')}
          cancelText={t('actions.cancel')}
          confirmDanger
          preview={`${t('nav.workspaces.resetWorkspaceDialog.pathLabel')}\n${workspace.rootPath}`}
        />
      </div>
    );
  }

  return (
    <div className={[
      'bitfun-nav-panel__workspace-item',
      isActive && 'is-active',
      isDragging && 'is-dragging',
      menuOpen && 'is-menu-open',
      sessionsCollapsed && 'is-sessions-collapsed',
      isSingle && 'is-single',
    ].filter(Boolean).join(' ')}
    aria-grabbed={draggable ? isDragging : undefined}>
      <div
        className="bitfun-nav-panel__workspace-item-card"
        draggable={draggable}
        onDragStart={onDragStart}
        onDragEnd={onDragEnd}
      >
        <button
          type="button"
          className="bitfun-nav-panel__workspace-item-collapse-btn"
          onClick={handleCollapseToggle}
          aria-label={sessionsCollapsed ? t('nav.workspaces.expandSessions') : t('nav.workspaces.collapseSessions')}
          aria-expanded={!sessionsCollapsed}
        >
          <span className="bitfun-nav-panel__workspace-item-icon" aria-hidden="true">
            <span className="bitfun-nav-panel__workspace-item-icon-default">
              <FolderOpen size={14} />
            </span>
            <span className={`bitfun-nav-panel__workspace-item-icon-toggle${sessionsCollapsed ? ' is-collapsed' : ''}`}>
              <ChevronDown size={14} />
            </span>
          </span>
        </button>
        <button
          type="button"
          className="bitfun-nav-panel__workspace-item-name-btn"
          onClick={() => { void handleCardNameClick(); }}
        >
          <span className="bitfun-nav-panel__workspace-item-title">
            <span className="bitfun-nav-panel__workspace-item-label">{workspaceDisplayName}</span>
          </span>
          {currentBranch ? (
            <span className="bitfun-nav-panel__workspace-item-branch">
              <GitBranch size={11} />
              <span>{currentBranch}</span>
            </span>
          ) : null}
        </button>

        <div className="bitfun-nav-panel__workspace-item-menu" ref={menuRef}>
          <Tooltip content={t('nav.items.project')} placement="right" followCursor>
            <button
              type="button"
              className="bitfun-nav-panel__workspace-item-menu-trigger"
              onClick={() => { void handleOpenFiles(); }}
            >
              <Folder size={14} />
            </button>
          </Tooltip>
          <div ref={menuAnchorRef}>
            <button
              type="button"
              className={`bitfun-nav-panel__workspace-item-menu-trigger${menuOpen ? ' is-open' : ''}`}
              onClick={() => setMenuOpen(prev => !prev)}
            >
              <MoreHorizontal size={14} />
            </button>
          </div>

          {menuOpen && menuPosition && createPortal(
            <div
              ref={menuPopoverRef}
              className="bitfun-nav-panel__workspace-item-menu-popover"
              role="menu"
              style={{ top: `${menuPosition.top}px`, left: `${menuPosition.left}px` }}
            >
              <button type="button" className="bitfun-nav-panel__workspace-item-menu-item" onClick={handleCreateCodeSession}>
                <Plus size={13} />
                <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.newCodeSession')}</span>
              </button>
              <button type="button" className="bitfun-nav-panel__workspace-item-menu-item" onClick={handleCreateCoworkSession}>
                <Plus size={13} />
                <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.newCoworkSession')}</span>
              </button>
              <button
                type="button"
                className="bitfun-nav-panel__workspace-item-menu-item"
                onClick={() => {
                  setMenuOpen(false);
                  setWorktreeModalOpen(true);
                }}
                disabled={!isRepository}
              >
                <GitBranch size={13} />
                <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.newWorktree')}</span>
              </button>
              <button type="button" className="bitfun-nav-panel__workspace-item-menu-item" onClick={() => { void handleReveal(); }}>
                <FolderSearch size={13} />
                <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.reveal')}</span>
              </button>
              <button type="button" className="bitfun-nav-panel__workspace-item-menu-item is-danger" onClick={() => { void handleCloseWorkspace(); }}>
                <FolderOpen size={13} />
                <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.close')}</span>
              </button>
            </div>,
            document.body
          )}
        </div>
      </div>

      <div className={`bitfun-nav-panel__workspace-item-sessions${sessionsCollapsed ? ' is-collapsed' : ''}`}>
        <SessionsSection
          workspaceId={workspace.id}
          workspacePath={workspace.rootPath}
          isActiveWorkspace={isActive}
        />
      </div>

      <BranchSelectModal
        isOpen={worktreeModalOpen}
        onClose={() => setWorktreeModalOpen(false)}
        onSelect={(result) => { void handleCreateWorktree(result); }}
        repositoryPath={workspace.rootPath}
        title={t('nav.workspaces.actions.newWorktree')}
      />
    </div>
  );
};

export default WorkspaceItem;

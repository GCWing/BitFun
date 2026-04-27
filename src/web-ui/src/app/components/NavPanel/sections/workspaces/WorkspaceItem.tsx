import React, { useCallback, useContext, useEffect, useRef, useState } from 'react';

function useStickyObserver(ref: React.RefObject<HTMLDivElement | null>) {
  const [isStuck, setIsStuck] = useState(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    // Find the nearest scrollable ancestor to use as the observer root.
    // The sticky element is pinned relative to this scroll container.
    let scrollContainer: Element | null = el.parentElement;
    while (scrollContainer) {
      const style = window.getComputedStyle(scrollContainer);
      const isScrollable =
        (style.overflowY === 'auto' || style.overflowY === 'scroll') &&
        scrollContainer.scrollHeight > scrollContainer.clientHeight;
      if (isScrollable) break;
      scrollContainer = scrollContainer.parentElement;
    }

    // Place a sentinel element as a sibling right before the sticky element.
    // When the sentinel scrolls out of the top of the container,
    // the sticky element is considered "stuck".
    const sentinel = document.createElement('div');
    sentinel.style.position = 'static';
    sentinel.style.height = '1px';
    sentinel.style.width = '1px';
    sentinel.style.pointerEvents = 'none';
    sentinel.style.visibility = 'hidden';
    el.parentElement?.insertBefore(sentinel, el);

    const observer = new IntersectionObserver(
      ([entry]) => {
        // Sentinel is not intersecting → sticky element has been pushed to the top
        setIsStuck(!entry.isIntersecting);
      },
      {
        root: scrollContainer,
        threshold: 0,
        rootMargin: '-1px 0px 0px 0px',
      }
    );

    observer.observe(sentinel);

    return () => {
      observer.disconnect();
      sentinel.remove();
    };
  }, [ref]);

  return isStuck;
}
import { createPortal } from 'react-dom';
import { Folder, FolderOpen, MoreHorizontal, FolderSearch, Plus, ChevronDown, Trash2, RotateCcw, Copy, FileText, GitBranch } from 'lucide-react';
import { DotMatrixArrowRightIcon } from './DotMatrixArrowRightIcon';
import { ConfirmDialog, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { i18nService } from '@/infrastructure/i18n';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import {
  createWorktreeWorkspace,
  deleteWorktreeWorkspace,
} from '@/infrastructure/services/business/worktreeWorkspaceService';
import { useNavSceneStore } from '@/app/stores/navSceneStore';
import { useApp } from '@/app/hooks/useApp';
import { useGitBasicInfo } from '@/tools/git/hooks/useGitState';
import { workspaceAPI } from '@/infrastructure/api';
import { notificationService } from '@/shared/notification-system';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { openMainSession } from '@/flow_chat/services/openBtwSession';
import { findReusableEmptySessionId } from '@/app/utils/projectSessionWorkspace';
import { BranchSelectModal, type BranchSelectResult } from '../../../panels/BranchSelectModal';
import SessionsSection from '../sessions/SessionsSection';
import {
  WorkspaceKind,
  isLinkedWorktreeWorkspace,
  isRemoteWorkspace,
  type WorkspaceInfo,
} from '@/shared/types';
import { SSHContext } from '@/features/ssh-remote/SSHRemoteContext';

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
  const {
    openWorkspace,
    setActiveWorkspace,
    closeWorkspaceById,
    deleteAssistantWorkspace,
    resetAssistantWorkspace,
  } = useWorkspaceContext();
  const { switchLeftPanelTab } = useApp();
  const openNavScene = useNavSceneStore(s => s.openNavScene);
  const { isRepository } = useGitBasicInfo(workspace.rootPath);
  const [menuOpen, setMenuOpen] = useState(false);
  const [worktreeModalOpen, setWorktreeModalOpen] = useState(false);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deleteWorktreeDialogOpen, setDeleteWorktreeDialogOpen] = useState(false);
  const [resetDialogOpen, setResetDialogOpen] = useState(false);
  const [isDeletingAssistant, setIsDeletingAssistant] = useState(false);
  const [isDeletingWorktree, setIsDeletingWorktree] = useState(false);
  const [isResettingWorkspace, setIsResettingWorkspace] = useState(false);
  const [sessionsCollapsed, setSessionsCollapsed] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuAnchorRef = useRef<HTMLDivElement>(null);
  const menuPopoverRef = useRef<HTMLDivElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);
  const isCardStuck = useStickyObserver(cardRef);
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
  const isLinkedWorktree = isLinkedWorktreeWorkspace(workspace);

  // Remote connection status — optional: safe if not inside SSHRemoteProvider
  const sshContext = useContext(SSHContext);
  const remoteConnStatus = workspace.connectionId && sshContext
    ? (sshContext.workspaceStatuses[workspace.connectionId] ?? 'connecting')
    : undefined;

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
      await flowChatManager.resetWorkspaceSessions(workspace, {
        reinitialize: isActive,
        preferredMode: 'Claw',
        ensureAssistantBootstrap:
          isActive && workspace.workspaceKind === WorkspaceKind.Assistant,
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
  }, [isActive, isDefaultAssistantWorkspace, isResettingWorkspace, resetAssistantWorkspace, t, workspace]);

  const handleReveal = useCallback(async () => {
    setMenuOpen(false);
    if (isRemoteWorkspace(workspace)) return;
    try {
      await workspaceAPI.revealInExplorer(workspace.rootPath);
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.revealFailed'),
        { duration: 4000 }
      );
    }
  }, [t, workspace]);

  const handleCopyWorkspacePath = useCallback(async () => {
    setMenuOpen(false);
    const path = workspace.rootPath;
    if (!path) return;
    try {
      await navigator.clipboard.writeText(path);
      notificationService.success(t('contextMenu.status.copyPathSuccess'), { duration: 2000 });
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.copyPathFailed'),
        { duration: 4000 }
      );
    }
  }, [t, workspace.rootPath]);

  const handleCreateSession = useCallback(async (mode?: 'agentic' | 'Cowork' | 'Claw') => {
    setMenuOpen(false);
    const resolvedMode = mode ?? (workspace.workspaceKind === WorkspaceKind.Assistant ? 'Claw' : undefined);
    try {
      const reusableId = findReusableEmptySessionId(workspace, resolvedMode);
      if (reusableId) {
        await openMainSession(reusableId, {
          workspaceId: workspace.id,
          activateWorkspace: setActiveWorkspace,
        });
        return;
      }
      const newSessionId = await flowChatManager.createChatSession(
        {
          workspacePath: workspace.rootPath,
          ...(isRemoteWorkspace(workspace) && workspace.connectionId
            ? { remoteConnectionId: workspace.connectionId }
            : {}),
          ...(isRemoteWorkspace(workspace) && workspace.sshHost
            ? { remoteSshHost: workspace.sshHost }
            : {}),
        },
        resolvedMode
      );
      await openMainSession(newSessionId, {
        workspaceId: workspace.id,
        activateWorkspace: setActiveWorkspace,
      });
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.createSessionFailed'),
        { duration: 4000 }
      );
    }
  }, [
    setActiveWorkspace,
    t,
    workspace,
  ]);

  const handleCreateCodeSession = useCallback(() => {
    void handleCreateSession('agentic');
  }, [handleCreateSession]);

  const handleCreateCoworkSession = useCallback(() => {
    void handleCreateSession('Cowork');
  }, [handleCreateSession]);

  const handleCreateInitSession = useCallback(async () => {
    setMenuOpen(false);

    try {
      const sessionId = await flowChatManager.createChatSession(
        {
          workspacePath: workspace.rootPath,
          ...(isRemoteWorkspace(workspace) && workspace.connectionId
            ? { remoteConnectionId: workspace.connectionId }
            : {}),
          ...(isRemoteWorkspace(workspace) && workspace.sshHost
            ? { remoteSshHost: workspace.sshHost }
            : {}),
        },
        'Init'
      );

      await openMainSession(sessionId, {
        workspaceId: workspace.id,
        activateWorkspace: setActiveWorkspace,
      });

      const initPrompt = i18nService.t('flow-chat:chatInput.initPrompt', {
        defaultValue: 'Please generate or update AGENTS.md so it matches the current project. Write it in English and keep the English version complete.',
      });

      await flowChatManager.sendMessage(initPrompt, sessionId, initPrompt, 'Init');
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.initSessionFailed'),
        { duration: 4000 }
      );
    }
  }, [setActiveWorkspace, t, workspace]);

  const handleCreateWorktree = useCallback(async (result: BranchSelectResult) => {
    try {
      const created = await createWorktreeWorkspace({
        repositoryPath: workspace.rootPath,
        branch: result.branch,
        isNew: result.isNew,
        openAfterCreate: result.openAfterCreate,
        openWorkspace,
      });
      notificationService.success(
        created.openedWorkspace
          ? t('nav.workspaces.worktreeCreatedAndOpened')
          : t('nav.workspaces.worktreeCreated'),
        { duration: 2500 },
      );
    } catch (error) {
      notificationService.error(
        t(
          result.openAfterCreate
            ? 'nav.workspaces.worktreeCreateOrOpenFailed'
            : 'nav.workspaces.worktreeCreateFailed',
          {
          error: error instanceof Error ? error.message : String(error),
          },
        ),
        { duration: 4000 }
      );
    }
  }, [openWorkspace, t, workspace.rootPath]);

  const handleRequestDeleteWorktree = useCallback(() => {
    setMenuOpen(false);
    setDeleteWorktreeDialogOpen(true);
  }, []);

  const handleConfirmDeleteWorktree = useCallback(async () => {
    if (!isLinkedWorktree || isDeletingWorktree) {
      return;
    }

    setIsDeletingWorktree(true);
    try {
      await deleteWorktreeWorkspace({
        workspace,
        closeWorkspaceById,
      });
      notificationService.success(t('nav.workspaces.worktreeDeleted'), { duration: 2500 });
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.deleteWorktreeFailed'),
        { duration: 4000 },
      );
    } finally {
      setIsDeletingWorktree(false);
    }
  }, [closeWorkspaceById, isDeletingWorktree, isLinkedWorktree, t, workspace]);

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
      aria-current={isActive ? 'location' : undefined}
      aria-grabbed={draggable ? isDragging : undefined}>
        <div
          ref={cardRef}
          className={[
            'bitfun-nav-panel__assistant-item-card',
            isCardStuck && 'is-stuck',
          ].filter(Boolean).join(' ')}
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
              {isActive ? (
                <span className="bitfun-nav-panel__assistant-item-active-icon">
                  <DotMatrixArrowRightIcon size={14} />
                </span>
              ) : (
                <span className="bitfun-nav-panel__assistant-item-avatar-letter">
                  {workspaceDisplayName.charAt(0)}
                </span>
              )}
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
                <div className="bitfun-nav-panel__workspace-item-menu-divider" />
                <button
                  type="button"
                  className="bitfun-nav-panel__workspace-item-menu-item"
                  onClick={() => { void handleCopyWorkspacePath(); }}
                  disabled={!workspace.rootPath}
                >
                  <Copy size={13} />
                  <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.copyPath')}</span>
                </button>
                <button
                  type="button"
                  className="bitfun-nav-panel__workspace-item-menu-item"
                  onClick={() => { void handleReveal(); }}
                  disabled={isRemoteWorkspace(workspace)}
                >
                  <FolderSearch size={13} />
                  <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.reveal')}</span>
                </button>
                {(isDefaultAssistantWorkspace || isNamedAssistantWorkspace) ? (
                  <>
                    <div className="bitfun-nav-panel__workspace-item-menu-divider" />
                    {isDefaultAssistantWorkspace ? (
                      <button
                        type="button"
                        className="bitfun-nav-panel__workspace-item-menu-item is-danger"
                        onClick={handleRequestResetWorkspace}
                        disabled={isResettingWorkspace}
                      >
                        <RotateCcw size={13} />
                        <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.resetWorkspace')}</span>
                      </button>
                    ) : null}
                    {isNamedAssistantWorkspace ? (
                      <button
                        type="button"
                        className="bitfun-nav-panel__workspace-item-menu-item is-danger"
                        onClick={handleRequestDeleteAssistant}
                        disabled={isDeletingAssistant}
                      >
                        <Trash2 size={13} />
                        <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.deleteAssistant')}</span>
                      </button>
                    ) : null}
                  </>
                ) : null}
              </div>,
              document.body
            )}
          </div>
        </div>

        <div className={`bitfun-nav-panel__assistant-item-sessions${sessionsCollapsed ? ' is-collapsed' : ''}`}>
          <SessionsSection
            workspaceId={workspace.id}
            workspacePath={workspace.rootPath}
            remoteConnectionId={isRemoteWorkspace(workspace) ? workspace.connectionId : null}
            remoteSshHost={isRemoteWorkspace(workspace) ? workspace.sshHost : null}
            isActiveWorkspace={isActive}
            assistantLabel={workspaceDisplayName}
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
    aria-current={isActive ? 'location' : undefined}
    aria-grabbed={draggable ? isDragging : undefined}>
      <div
        ref={cardRef}
        className={[
          'bitfun-nav-panel__workspace-item-card',
          isCardStuck && 'is-stuck',
        ].filter(Boolean).join(' ')}
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
              {isActive ? (
                <span className="bitfun-nav-panel__workspace-item-active-icon">
                  <DotMatrixArrowRightIcon size={14} />
                </span>
              ) : (
                <FolderOpen size={14} />
              )}
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
          <span className={`bitfun-nav-panel__workspace-item-title${isRemoteWorkspace(workspace) ? ' is-remote' : ''}`}>
            <span className="bitfun-nav-panel__workspace-item-label">{workspaceDisplayName}</span>
            {isRemoteWorkspace(workspace) && (
              <span className="bitfun-nav-panel__workspace-item-subtitle">
                <span
                  className={`bitfun-nav-panel__workspace-item-status-dot is-${remoteConnStatus ?? 'connecting'}`}
                  aria-label={remoteConnStatus ?? 'connecting'}
                />
                <span>{workspace.connectionName}</span>
              </span>
            )}
          </span>
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
                <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.sessions.newCodeSessionShort')}</span>
              </button>
              <button type="button" className="bitfun-nav-panel__workspace-item-menu-item" onClick={handleCreateCoworkSession}>
                <Plus size={13} />
                <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.sessions.newCoworkSessionShort')}</span>
              </button>
              <button type="button" className="bitfun-nav-panel__workspace-item-menu-item" onClick={() => { void handleCreateInitSession(); }}>
                <FileText size={13} />
                <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.initAgents')}</span>
              </button>
              <div className="bitfun-nav-panel__workspace-item-menu-divider" />
              {isLinkedWorktree ? (
                <button
                  type="button"
                  className="bitfun-nav-panel__workspace-item-menu-item is-danger"
                  onClick={handleRequestDeleteWorktree}
                  disabled={isDeletingWorktree}
                >
                  <Trash2 size={13} />
                  <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.deleteWorktree')}</span>
                </button>
              ) : (
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
              )}
              <button
                type="button"
                className="bitfun-nav-panel__workspace-item-menu-item"
                onClick={() => { void handleCopyWorkspacePath(); }}
                disabled={!workspace.rootPath}
              >
                <Copy size={13} />
                <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.copyPath')}</span>
              </button>
              <button
                type="button"
                className="bitfun-nav-panel__workspace-item-menu-item"
                onClick={() => { void handleReveal(); }}
                disabled={isRemoteWorkspace(workspace)}
              >
                <FolderSearch size={13} />
                <span className="bitfun-nav-panel__workspace-item-menu-label">{t('nav.workspaces.actions.reveal')}</span>
              </button>
              <div className="bitfun-nav-panel__workspace-item-menu-divider" />
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
          remoteConnectionId={isRemoteWorkspace(workspace) ? workspace.connectionId : null}
          remoteSshHost={isRemoteWorkspace(workspace) ? workspace.sshHost : null}
          isActiveWorkspace={isActive}
        />
      </div>

      <BranchSelectModal
        isOpen={worktreeModalOpen}
        onClose={() => setWorktreeModalOpen(false)}
        onSelect={(result) => { void handleCreateWorktree(result); }}
        repositoryPath={workspace.rootPath}
        title={t('nav.workspaces.actions.newWorktree')}
        showOpenAfterCreate
        defaultOpenAfterCreate
      />
      <ConfirmDialog
        isOpen={deleteWorktreeDialogOpen}
        onClose={() => setDeleteWorktreeDialogOpen(false)}
        onConfirm={() => { void handleConfirmDeleteWorktree(); }}
        title={t('nav.workspaces.deleteWorktreeDialog.title', { name: workspaceDisplayName })}
        message={t('nav.workspaces.deleteWorktreeDialog.message')}
        confirmText={t('nav.workspaces.actions.deleteWorktree')}
        cancelText={t('actions.cancel')}
        confirmDanger
        preview={`${t('nav.workspaces.deleteWorktreeDialog.pathLabel')}\n${workspace.rootPath}`}
      />
    </div>
  );
};

export default WorkspaceItem;

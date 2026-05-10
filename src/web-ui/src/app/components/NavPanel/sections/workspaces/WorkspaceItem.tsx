import React, { useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Folder, FolderOpen, MoreHorizontal, FolderSearch, Plus, ChevronDown, Trash2, RotateCcw, Copy, FileText, GitBranch, Bot } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { DotMatrixArrowRightIcon } from './DotMatrixArrowRightIcon';
import { Button, ConfirmDialog, Modal, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { i18nService } from '@/infrastructure/i18n';
import { aiExperienceConfigService } from '@/infrastructure/config/services/AIExperienceConfigService';
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
import type { AcpClientInfo } from '@/infrastructure/api/service-api/ACPClientAPI';
import { loadWorkspaceAcpMenuClients } from './workspaceAcpMenuClients';
import { BranchSelectModal, type BranchSelectResult } from '../../../panels/BranchSelectModal';
import SessionsSection from '../sessions/SessionsSection';
import {
  WorkspaceKind,
  isLinkedWorktreeWorkspace,
  isRemoteWorkspace,
  type WorkspaceInfo,
} from '@/shared/types';
import { SSHContext } from '@/features/ssh-remote/SSHRemoteContext';
import { useWorkspaceSearchIndex } from '@/tools/file-explorer';
import { computeFixedPopoverPosition } from '@/shared/utils/fixedPopoverViewport';


interface WorkspaceItemProps {
  workspace: WorkspaceInfo;
  isActive: boolean;
  isSingle?: boolean;
  draggable?: boolean;
  isDragging?: boolean;
  onDragStart?: React.DragEventHandler<HTMLDivElement>;
  onDragEnd?: React.DragEventHandler<HTMLDivElement>;
}

function getIndexActionKind(phase?: string | null): 'build' | 'rebuild' {
  if (!phase || phase === 'needs_index' || phase === 'preparing') {
    return 'build';
  }
  return 'rebuild';
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
  const { t: tFiles } = useTranslation('panels/files');
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
  const [searchIndexModalOpen, setSearchIndexModalOpen] = useState(false);
  const [workspaceSearchEnabled, setWorkspaceSearchEnabled] = useState(
    () => aiExperienceConfigService.getSettings().enable_workspace_search,
  );
  const [acpClients, setAcpClients] = useState<AcpClientInfo[]>([]);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuAnchorRef = useRef<HTMLDivElement>(null);
  const menuPopoverRef = useRef<HTMLDivElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);
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
  const canShowSearchIndex =
    isActive
    && workspaceSearchEnabled
    && workspace.workspaceKind === WorkspaceKind.Normal
    && !isRemoteWorkspace(workspace);
  const workspaceSearchIndex = useWorkspaceSearchIndex({
    workspacePath: canShowSearchIndex ? workspace.rootPath : undefined,
    enabled: canShowSearchIndex,
  });

  useEffect(() => {
    setWorkspaceSearchEnabled(aiExperienceConfigService.getSettings().enable_workspace_search);
    return aiExperienceConfigService.addChangeListener(settings => {
      setWorkspaceSearchEnabled(settings.enable_workspace_search);
    });
  }, []);

  // Remote connection status — optional: safe if not inside SSHRemoteProvider
  const sshContext = useContext(SSHContext);
  const remoteConnStatus = workspace.connectionId && sshContext
    ? (sshContext.workspaceStatuses[workspace.connectionId] ?? 'connecting')
    : undefined;

  const searchIndexIndicator = useMemo(() => {
    if (!canShowSearchIndex) {
      return null;
    }

    const repoStatus = workspaceSearchIndex.indexStatus?.repoStatus ?? null;
    const activeTask = workspaceSearchIndex.indexStatus?.activeTask ?? null;
    const phase = repoStatus?.phase;
    const isTaskActive = activeTask?.state === 'queued' || activeTask?.state === 'running';
    const hasError = Boolean(
      workspaceSearchIndex.error
      || repoStatus?.lastError
      || activeTask?.error
      || activeTask?.state === 'failed'
    );
    const dirtyFiles = repoStatus
      ? repoStatus.dirtyFiles.modified + repoStatus.dirtyFiles.deleted + repoStatus.dirtyFiles.new
      : 0;

    let tone: 'green' | 'yellow' | 'gray' | 'red' = 'gray';
    if (hasError || phase === 'limited') {
      tone = 'red';
    } else if (!phase || phase === 'needs_index') {
      tone = 'gray';
    } else if (
      isTaskActive
      || phase === 'preparing'
      || phase === 'building'
      || phase === 'refreshing'
      || Boolean(repoStatus?.rebuildRecommended)
    ) {
      tone = 'yellow';
    } else if (phase === 'ready' || phase === 'tracking_changes') {
      tone = 'green';
    }

    const phaseLabel = tFiles(`search.index.phase.${phase ?? 'unknown'}`, {
      defaultValue: phase ?? tFiles('search.index.phase.unknown'),
    });
    const title = tFiles(`search.index.indicator.tones.${tone}`);
    const summary = repoStatus
      ? tFiles(`search.index.summary.${phase ?? 'unavailable'}`, {
          defaultValue: tFiles('search.index.summary.unavailable'),
        })
      : workspaceSearchIndex.loading
        ? tFiles('search.index.indicator.checking')
        : tFiles('search.index.summary.unavailable');
    const activeTaskLabel = activeTask
      ? tFiles(`search.index.taskState.${activeTask.state}`, {
          defaultValue: activeTask.state,
        })
      : null;
    const progressLabel = activeTask
      ? typeof activeTask.total === 'number' && activeTask.total > 0
        ? tFiles('search.index.indicator.progressKnown', {
            processed: activeTask.processed,
            total: activeTask.total,
          })
        : tFiles('search.index.indicator.progressUnknown', {
            processed: activeTask.processed,
          })
      : null;
    const progressPercent =
      activeTask && typeof activeTask.total === 'number' && activeTask.total > 0
        ? Math.max(0, Math.min(100, (activeTask.processed / activeTask.total) * 100))
        : null;
    const progressPercentLabel =
      typeof progressPercent === 'number'
        ? `${Math.round(progressPercent)}%`
        : null;
    const dirtyFilesLabel =
      repoStatus && dirtyFiles > 0
        ? tFiles('search.index.indicator.dirtyFiles', {
            modified: repoStatus.dirtyFiles.modified,
            deleted: repoStatus.dirtyFiles.deleted,
            new: repoStatus.dirtyFiles.new,
          })
        : null;
    const errorText = workspaceSearchIndex.error ?? activeTask?.error ?? repoStatus?.lastError ?? null;

    return {
      tone,
      title,
      phaseLabel,
      summary,
      activeTaskLabel,
      activeTaskMessage: activeTask?.message ?? null,
      progressLabel,
      progressPercent,
      progressPercentLabel,
      dirtyFilesLabel,
      rebuildRecommended: Boolean(repoStatus?.rebuildRecommended),
      probeHealthy: repoStatus?.probeHealthy ?? true,
      errorText,
      ariaLabel: `${tFiles('search.index.indicator.label')}: ${title} · ${phaseLabel}`,
    };
  }, [
    canShowSearchIndex,
    tFiles,
    workspaceSearchIndex.error,
    workspaceSearchIndex.indexStatus,
    workspaceSearchIndex.loading,
  ]);
  const searchIndexActionKind = getIndexActionKind(
    workspaceSearchIndex.indexStatus?.repoStatus.phase ?? null
  );
  const searchIndexActionLabel = tFiles(
    searchIndexActionKind === 'build'
      ? 'search.index.actions.build'
      : 'search.index.actions.rebuild'
  );

  const handleSearchIndexAction = useCallback(async () => {
    const result =
      searchIndexActionKind === 'build'
        ? await workspaceSearchIndex.buildIndex()
        : await workspaceSearchIndex.rebuildIndex();

    if (!result) {
      return;
    }

    notificationService.success(
      tFiles(
        searchIndexActionKind === 'build'
          ? 'notifications.searchIndexBuildStarted'
          : 'notifications.searchIndexRebuildStarted'
      ),
      { duration: 2200 }
    );
  }, [searchIndexActionKind, tFiles, workspaceSearchIndex]);

  const updateMenuPosition = useCallback(() => {
    const anchor = menuAnchorRef.current;
    if (!anchor) return;

    const rect = anchor.getBoundingClientRect();
    const viewportPadding = 8;
    const gap = 6;
    const fallbackWidth = 240;
    const fallbackHeight = 260;

    const apply = () => {
      const menuEl = menuPopoverRef.current;
      const w = menuEl?.offsetWidth ?? fallbackWidth;
      const h = menuEl?.offsetHeight ?? fallbackHeight;
      setMenuPosition(computeFixedPopoverPosition(rect, w, h, gap, viewportPadding));
    };

    apply();
    requestAnimationFrame(apply);
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

  useEffect(() => {
    let cancelled = false;

    const loadAcpClients = async () => {
      try {
        const clients = await loadWorkspaceAcpMenuClients();
        if (!cancelled) {
          setAcpClients(clients);
        }
      } catch (_error) {
        setAcpClients([]);
      }
    };

    void loadAcpClients();
    window.addEventListener('bitfun:acp-clients-changed', loadAcpClients);
    window.addEventListener('bitfun:acp-requirements-changed', loadAcpClients);
    return () => {
      cancelled = true;
      window.removeEventListener('bitfun:acp-clients-changed', loadAcpClients);
      window.removeEventListener('bitfun:acp-requirements-changed', loadAcpClients);
    };
  }, []);

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
      setSessionsCollapsed(false);
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

  const handleCreateAcpSession = useCallback(async (client: AcpClientInfo) => {
    setMenuOpen(false);
    try {
      const sessionId = await flowChatManager.createAcpChatSession(
        client.id,
        {
          workspacePath: workspace.rootPath,
          ...(isRemoteWorkspace(workspace) && workspace.connectionId
            ? { remoteConnectionId: workspace.connectionId }
            : {}),
          ...(isRemoteWorkspace(workspace) && workspace.sshHost
            ? { remoteSshHost: workspace.sshHost }
            : {}),
        },
      );
      await openMainSession(sessionId, {
        workspaceId: workspace.id,
        activateWorkspace: setActiveWorkspace,
      });
    } catch (error) {
      notificationService.error(
        error instanceof Error ? error.message : t('nav.workspaces.createSessionFailed'),
        { duration: 4000 }
      );
    }
  }, [setActiveWorkspace, t, workspace]);

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
          className="bitfun-nav-panel__assistant-item-card"
          draggable={draggable}
          onDragStart={onDragStart}
          onDragEnd={onDragEnd}
          onClick={() => { void handleCardNameClick(); }}
          style={{ cursor: 'pointer' }}
        >
          <button
            type="button"
            className="bitfun-nav-panel__assistant-item-collapse-btn"
            onClick={e => { e.stopPropagation(); handleCollapseToggle(); }}
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
          <Tooltip content={workspace.rootPath} placement="right" followCursor>
            <button
              type="button"
              className="bitfun-nav-panel__assistant-item-name-btn"
              onClick={e => { e.stopPropagation(); void handleCardNameClick(); }}
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
          </Tooltip>

          <div className="bitfun-nav-panel__assistant-item-menu" ref={menuRef} onClick={e => e.stopPropagation()}>
            <Tooltip content={t('nav.items.project')} placement="right" followCursor>
              <button
                type="button"
                className="bitfun-nav-panel__assistant-item-menu-trigger"
                onClick={() => { void handleOpenFiles(); }}
              >
                <Folder size="var(--bitfun-nav-row-action-icon-size)" />
              </button>
            </Tooltip>
            <div ref={menuAnchorRef}>
              <button
                type="button"
                className={`bitfun-nav-panel__assistant-item-menu-trigger${menuOpen ? ' is-open' : ''}`}
                onClick={() => setMenuOpen(prev => !prev)}
              >
                <MoreHorizontal size="var(--bitfun-nav-row-action-icon-size)" />
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
        className="bitfun-nav-panel__workspace-item-card"
        draggable={draggable}
        onDragStart={onDragStart}
        onDragEnd={onDragEnd}
        onClick={() => { void handleCardNameClick(); }}
        style={{ cursor: 'pointer' }}
      >
        <button
          type="button"
          className="bitfun-nav-panel__workspace-item-collapse-btn"
          onClick={e => { e.stopPropagation(); handleCollapseToggle(); }}
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
        <div className="bitfun-nav-panel__workspace-item-name-cluster">
          <div className="bitfun-nav-panel__workspace-item-name-stack">
            <div className="bitfun-nav-panel__workspace-item-name-row">
              <Tooltip content={workspace.rootPath} placement="right" followCursor>
                <button
                  type="button"
                  className="bitfun-nav-panel__workspace-item-name-btn"
                  onClick={e => { e.stopPropagation(); void handleCardNameClick(); }}
                >
                  <span className="bitfun-nav-panel__workspace-item-name-line">
                    <span className="bitfun-nav-panel__workspace-item-label">{workspaceDisplayName}</span>
                  </span>
                </button>
              </Tooltip>
              {searchIndexIndicator && (
                <>
                  <Tooltip
                    placement="right"
                    content={tFiles('search.index.indicator.hoverTooltip', {
                      status: [
                        searchIndexIndicator.title,
                        searchIndexIndicator.activeTaskLabel ?? searchIndexIndicator.phaseLabel,
                      ].join(' · '),
                    })}
                  >
                    <button
                      type="button"
                      className={`bitfun-nav-panel__workspace-index-indicator is-${searchIndexIndicator.tone}`}
                      aria-label={searchIndexIndicator.ariaLabel}
                      aria-expanded={searchIndexModalOpen}
                      onClick={e => {
                        e.stopPropagation();
                        setSearchIndexModalOpen(true);
                      }}
                    />
                  </Tooltip>
                  <Modal
                    isOpen={searchIndexModalOpen}
                    onClose={() => setSearchIndexModalOpen(false)}
                    title={tFiles('search.index.indicator.label')}
                    size="small"
                    contentInset
                    contentClassName="bitfun-nav-panel__workspace-index-modal-content"
                  >
                    <div className={`bitfun-nav-panel__workspace-index-tooltip is-${searchIndexIndicator.tone}`}>
                      <div className="bitfun-nav-panel__workspace-index-tooltip-header">
                        <div className="bitfun-nav-panel__workspace-index-tooltip-heading">
                          <span className={`bitfun-nav-panel__workspace-index-tooltip-dot is-${searchIndexIndicator.tone}`} aria-hidden="true" />
                          <div className="bitfun-nav-panel__workspace-index-tooltip-title-wrap">
                            <span className="bitfun-nav-panel__workspace-index-tooltip-title">
                              {searchIndexIndicator.title}
                            </span>
                            <span className="bitfun-nav-panel__workspace-index-tooltip-phase">
                              {searchIndexIndicator.activeTaskLabel ?? searchIndexIndicator.phaseLabel}
                            </span>
                          </div>
                        </div>
                        <span className={`bitfun-nav-panel__workspace-index-tooltip-badge is-${searchIndexIndicator.tone}`}>
                          {searchIndexIndicator.phaseLabel}
                        </span>
                      </div>
                      <div className="bitfun-nav-panel__workspace-index-tooltip-summary">
                        {searchIndexIndicator.activeTaskMessage ?? searchIndexIndicator.summary}
                      </div>
                      {searchIndexIndicator.progressLabel ? (
                        <div className="bitfun-nav-panel__workspace-index-tooltip-progress">
                          <div className="bitfun-nav-panel__workspace-index-tooltip-progress-head">
                            <span>{searchIndexIndicator.progressLabel}</span>
                            {searchIndexIndicator.progressPercentLabel ? (
                              <span className="bitfun-nav-panel__workspace-index-tooltip-progress-value">
                                {searchIndexIndicator.progressPercentLabel}
                              </span>
                            ) : null}
                          </div>
                          {typeof searchIndexIndicator.progressPercent === 'number' ? (
                            <div className="bitfun-nav-panel__workspace-index-tooltip-progress-bar" aria-hidden="true">
                              <span
                                className={`bitfun-nav-panel__workspace-index-tooltip-progress-fill is-${searchIndexIndicator.tone}`}
                                style={{ width: `${searchIndexIndicator.progressPercent}%` }}
                              />
                            </div>
                          ) : null}
                        </div>
                      ) : null}
                      {searchIndexIndicator.dirtyFilesLabel ? (
                        <div className="bitfun-nav-panel__workspace-index-tooltip-meta">
                          {searchIndexIndicator.dirtyFilesLabel}
                        </div>
                      ) : null}
                      {searchIndexIndicator.rebuildRecommended ? (
                        <div className="bitfun-nav-panel__workspace-index-tooltip-meta is-warning">
                          {tFiles('search.index.indicator.rebuildRecommended')}
                        </div>
                      ) : null}
                      {!searchIndexIndicator.probeHealthy ? (
                        <div className="bitfun-nav-panel__workspace-index-tooltip-meta is-warning">
                          {tFiles('search.index.indicator.probeDegraded')}
                        </div>
                      ) : null}
                      {searchIndexIndicator.errorText ? (
                        <div className="bitfun-nav-panel__workspace-index-tooltip-error">
                          {searchIndexIndicator.errorText}
                        </div>
                      ) : null}
                      <div className="bitfun-nav-panel__workspace-index-tooltip-actions">
                        <Button
                          size="small"
                          variant={searchIndexActionKind === 'build' ? 'accent' : 'secondary'}
                          onClick={() => {
                            void handleSearchIndexAction();
                          }}
                          disabled={
                            workspaceSearchIndex.loading
                            || workspaceSearchIndex.actionRunning
                            || workspaceSearchIndex.hasActiveTask
                          }
                        >
                          {workspaceSearchIndex.actionRunning || workspaceSearchIndex.hasActiveTask
                            ? tFiles('search.index.actions.running')
                            : searchIndexActionLabel}
                        </Button>
                      </div>
                    </div>
                  </Modal>
                </>
              )}
            </div>
            {isRemoteWorkspace(workspace) && (
              <span className="bitfun-nav-panel__workspace-item-subtitle">
                <span
                  className={`bitfun-nav-panel__workspace-item-status-dot is-${remoteConnStatus ?? 'connecting'}`}
                  aria-label={remoteConnStatus ?? 'connecting'}
                />
                <span>{workspace.connectionName}</span>
              </span>
            )}
          </div>
        </div>

        <div className="bitfun-nav-panel__workspace-item-actions" onClick={e => e.stopPropagation()}>
          <div className="bitfun-nav-panel__workspace-item-menu" ref={menuRef}>
            <Tooltip content={t('nav.items.project')} placement="right" followCursor>
              <button
                type="button"
                className="bitfun-nav-panel__workspace-item-menu-trigger"
                onClick={() => { void handleOpenFiles(); }}
              >
                <Folder size="var(--bitfun-nav-row-action-icon-size)" />
              </button>
            </Tooltip>
            <div ref={menuAnchorRef}>
              <button
                type="button"
                className={`bitfun-nav-panel__workspace-item-menu-trigger${menuOpen ? ' is-open' : ''}`}
                onClick={() => setMenuOpen(prev => !prev)}
              >
                <MoreHorizontal size="var(--bitfun-nav-row-action-icon-size)" />
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
                {acpClients.map(client => {
                  const label = client.name || client.id;
                  return (
                    <button
                      key={client.id}
                      type="button"
                      className="bitfun-nav-panel__workspace-item-menu-item"
                      onClick={() => { void handleCreateAcpSession(client); }}
                    >
                      <Bot size={13} />
                      <span className="bitfun-nav-panel__workspace-item-menu-label">
                        {t('nav.sessions.newExternalAgentSessionShort', { agentName: label })}
                      </span>
                    </button>
                  );
                })}
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

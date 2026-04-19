/**
 * SessionList — reusable session list for the new workspace layout.
 *
 * Used by the floating `SessionCapsule`, assistant profile pages, and
 * workspace-scoped lists inside scene navigation.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Pencil, Trash2, Check, X, Brush, Code2, ListTodo, Sparkles, MoreHorizontal, Loader2, LayoutGrid } from 'lucide-react';
import { IconButton, Input, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { flowChatStore } from '../../../flow_chat/store/FlowChatStore';
import { flowChatManager } from '../../../flow_chat/services/FlowChatManager';
import type { FlowChatState, Session } from '../../../flow_chat/types/flow-chat';
import { useOverlayStore } from '../../stores/overlayStore';
import { getWorkspaceDisplayName, useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import { useAgentCanvasStore } from '@/app/components/panels/content-canvas/stores';
import {
  openBtwSessionInAuxPane,
  openMainSession,
  selectActiveBtwSessionTab,
} from '@/flow_chat/services/openBtwSession';
import { resolveSessionRelationship } from '@/flow_chat/utils/sessionMetadata';
import {
  compareSessionsForDisplay,
  findOpenedWorkspaceForSession,
  sessionBelongsToWorkspaceNavRow,
} from '@/flow_chat/utils/sessionOrdering';
import {
  resolveActiveRunningLiveAppId,
  useRunningLiveAppItems,
  type RunningLiveAppItem,
} from '@/app/scenes/apps/live-app/liveAppTaskView';
import { renderLiveAppIcon } from '@/app/scenes/apps/live-app/liveAppIcons';
import { stateMachineManager } from '@/flow_chat/state-machine';
import { SessionExecutionState } from '@/flow_chat/state-machine/types';
import './SessionList.scss';

const log = createLogger('SessionList');
const AGENT_SCENE = 'session' as const;

type SessionMode = 'code' | 'cowork' | 'design' | 'claw';

const resolveSessionModeType = (session: Session): SessionMode => {
  const normalizedMode = session.mode?.toLowerCase();
  if (normalizedMode === 'cowork') return 'cowork';
  if (normalizedMode === 'design') return 'design';
  if (normalizedMode === 'claw') return 'claw';
  return 'code';
};

const getSessionTitle = (session: Session): string =>
  session.title?.trim() || `Task ${session.sessionId.slice(0, 6)}`;

export interface SessionListProps {
  workspaceId?: string;
  workspacePath?: string;
  remoteConnectionId?: string | null;
  remoteSshHost?: string | null;
  isActiveWorkspace?: boolean;
  showCreateActions?: boolean;
  assistantLabel?: string;
  showSessionModeIcon?: boolean;
  listAllSessions?: boolean;
  listFilterQuery?: string;
  maxSessions?: number;
}

const SessionList: React.FC<SessionListProps> = ({
  workspaceId,
  workspacePath,
  remoteConnectionId = null,
  remoteSshHost = null,
  isActiveWorkspace: _isActiveWorkspace = true,
  assistantLabel,
  showSessionModeIcon = true,
  listAllSessions = false,
  listFilterQuery,
  maxSessions,
}) => {
  const { t } = useI18n('common');
  const { setActiveWorkspace, currentWorkspace, openedWorkspacesList } = useWorkspaceContext();
  const activeOverlay = useOverlayStore(s => s.activeOverlay);
  const openOverlay = useOverlayStore(s => s.openOverlay);
  const activeTabId = activeOverlay ?? AGENT_SCENE;
  const activeLiveAppId = resolveActiveRunningLiveAppId(activeOverlay);
  const runningLiveApps = useRunningLiveAppItems();
  const activeBtwSessionTab = useAgentCanvasStore(state => selectActiveBtwSessionTab(state as any));
  const activeBtwSessionData = activeBtwSessionTab?.content.data as
    | { childSessionId: string; parentSessionId: string; workspacePath?: string }
    | undefined;
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() =>
    flowChatStore.getState()
  );
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState('');
  const [openMenuSessionId, setOpenMenuSessionId] = useState<string | null>(null);
  const [sessionMenuPosition, setSessionMenuPosition] = useState<{ top: number; left: number } | null>(null);
  const [runningSessionIds, setRunningSessionIds] = useState<Set<string>>(new Set());
  const editInputRef = useRef<HTMLInputElement>(null);
  const sessionMenuPopoverRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const updateRunningSessions = () => {
      const running = new Set<string>();
      for (const session of flowChatState.sessions.values()) {
        const machine = stateMachineManager.get(session.sessionId);
        if (
          machine &&
          (machine.getCurrentState() === SessionExecutionState.PROCESSING ||
            machine.getCurrentState() === SessionExecutionState.FINISHING)
        ) {
          running.add(session.sessionId);
        }
      }
      setRunningSessionIds(running);
    };

    updateRunningSessions();
    const unsubscribe = stateMachineManager.subscribeGlobal(() => {
      updateRunningSessions();
    });
    return () => unsubscribe();
  }, [flowChatState.sessions]);

  useEffect(() => {
    const unsubscribe = flowChatStore.subscribe(state => setFlowChatState(state));
    return () => unsubscribe();
  }, []);

  useEffect(() => {
    if (editingSessionId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingSessionId]);

  useEffect(() => {
    if (!openMenuSessionId) return;
    const handleOutside = (event: MouseEvent) => {
      if (!sessionMenuPopoverRef.current?.contains(event.target as Node)) {
        setOpenMenuSessionId(null);
        setSessionMenuPosition(null);
      }
    };
    document.addEventListener('mousedown', handleOutside);
    return () => document.removeEventListener('mousedown', handleOutside);
  }, [openMenuSessionId]);

  const sessions = useMemo(
    () =>
      Array.from(flowChatState.sessions.values())
        .filter((session: Session) => {
          if (session.mode === 'Dispatcher') return false;
          if (listAllSessions) return true;
          if (workspacePath) {
            return sessionBelongsToWorkspaceNavRow(session, workspacePath, remoteConnectionId, remoteSshHost);
          }
          return !session.workspacePath;
        })
        .sort(compareSessionsForDisplay)
        .slice(0, maxSessions ?? Number.POSITIVE_INFINITY),
    [flowChatState.sessions, workspacePath, remoteConnectionId, remoteSshHost, listAllSessions, maxSessions]
  );

  const { topLevelSessions, childrenByParent } = useMemo(() => {
    const childMap = new Map<string, Session[]>();
    const parents: Session[] = [];
    const knownIds = new Set(sessions.map(session => session.sessionId));

    for (const session of sessions) {
      const parentSessionId = resolveSessionRelationship(session).parentSessionId;
      if (parentSessionId && parentSessionId.trim() && knownIds.has(parentSessionId)) {
        const children = childMap.get(parentSessionId) || [];
        children.push(session);
        childMap.set(parentSessionId, children);
      } else {
        parents.push(session);
      }
    }

    for (const [parentSessionId, children] of childMap) {
      childMap.set(parentSessionId, [...children].sort(compareSessionsForDisplay));
    }

    return {
      topLevelSessions: [...parents].sort(compareSessionsForDisplay),
      childrenByParent: childMap,
    };
  }, [sessions]);

  const visibleItems = useMemo(() => {
    const items: Array<{ session: Session; level: 0 | 1 }> = [];
    for (const parentSession of topLevelSessions) {
      items.push({ session: parentSession, level: 0 });
      const childSessions = childrenByParent.get(parentSession.sessionId) || [];
      for (const childSession of childSessions) {
        items.push({ session: childSession, level: 1 });
      }
    }
    return items;
  }, [childrenByParent, topLevelSessions]);

  const filteredVisibleItems = useMemo(() => {
    const trimmedQuery = listFilterQuery?.trim();
    if (!trimmedQuery) return visibleItems;

    const normalizedQuery = trimmedQuery.toLowerCase();
    return visibleItems.filter(({ session }) => {
      if (getSessionTitle(session).toLowerCase().includes(normalizedQuery)) return true;
      if (session.sessionId.toLowerCase().includes(normalizedQuery)) return true;
      if (listAllSessions) {
        const workspace = findOpenedWorkspaceForSession(session, openedWorkspacesList);
        if (workspace?.name?.toLowerCase().includes(normalizedQuery)) return true;
        if (workspace && getWorkspaceDisplayName(workspace).toLowerCase().includes(normalizedQuery)) return true;
      }
      return false;
    });
  }, [visibleItems, listFilterQuery, listAllSessions, openedWorkspacesList]);

  const filteredRunningLiveApps = useMemo(() => {
    const trimmedQuery = listFilterQuery?.trim().toLowerCase();
    if (!trimmedQuery) return runningLiveApps;
    return runningLiveApps.filter(app =>
      app.title.toLowerCase().includes(trimmedQuery) ||
      app.id.toLowerCase().includes(trimmedQuery) ||
      app.description.toLowerCase().includes(trimmedQuery)
    );
  }, [listFilterQuery, runningLiveApps]);

  const activeSessionId = flowChatState.activeSessionId;

  const handleSwitch = useCallback(
    async (sessionId: string) => {
      if (editingSessionId) return;
      try {
        const session = flowChatStore.getState().sessions.get(sessionId);
        const relationship = resolveSessionRelationship(session);
        const parentSessionId = relationship.parentSessionId;
        const resolvedWorkspaceId =
          listAllSessions && session
            ? findOpenedWorkspaceForSession(session, openedWorkspacesList)?.id
            : workspaceId;
        const mustActivateWorkspace =
          Boolean(resolvedWorkspaceId) && resolvedWorkspaceId !== currentWorkspace?.id;
        const activateWorkspace = mustActivateWorkspace
          ? async (targetWorkspaceId: string) => {
              await setActiveWorkspace(targetWorkspaceId);
            }
          : undefined;

        if (relationship.canOpenInAuxPane && parentSessionId && session) {
          await openMainSession(parentSessionId, {
            workspaceId: resolvedWorkspaceId,
            activateWorkspace,
          });
          openBtwSessionInAuxPane({
            childSessionId: sessionId,
            parentSessionId,
            workspacePath: session.workspacePath,
          });
          return;
        }

        if (sessionId === activeSessionId) {
          await openMainSession(sessionId, {
            workspaceId: resolvedWorkspaceId,
            activateWorkspace,
          });
          return;
        }

        await openMainSession(sessionId, {
          workspaceId: resolvedWorkspaceId,
          activateWorkspace,
        });
      } catch (error) {
        log.error('Failed to switch session', error);
      }
    },
    [
      activeSessionId,
      currentWorkspace?.id,
      editingSessionId,
      listAllSessions,
      openedWorkspacesList,
      setActiveWorkspace,
      workspaceId,
    ]
  );

  const resolveDisplayTitle = useCallback(
    (session: Session): string => {
      const rawTitle = getSessionTitle(session);
      const matched = rawTitle.match(/^(?:新建会话|New Session)\s*(\d+)$/i);
      if (!matched) return rawTitle;

      const mode = resolveSessionModeType(session);
      const label =
        mode === 'cowork'
          ? t('nav.sessions.newCoworkSession')
          : mode === 'design'
            ? t('nav.sessions.newDesignSession')
          : mode === 'claw'
            ? t('nav.sessions.newClawSession')
            : t('nav.sessions.newCodeSession');
      return `${label} ${matched[1]}`;
    },
    [t]
  );

  const handleMenuOpen = useCallback(
    (event: React.MouseEvent, sessionId: string) => {
      event.stopPropagation();
      if (openMenuSessionId === sessionId) {
        setOpenMenuSessionId(null);
        setSessionMenuPosition(null);
        return;
      }
      const button = event.currentTarget as HTMLElement;
      const rect = button.getBoundingClientRect();
      const viewportPadding = 8;
      const estimatedWidth = 160;
      const maxLeft = window.innerWidth - estimatedWidth - viewportPadding;
      setSessionMenuPosition({
        top: Math.max(viewportPadding, rect.bottom + 4),
        left: Math.max(viewportPadding, Math.min(rect.left, maxLeft)),
      });
      setOpenMenuSessionId(sessionId);
    },
    [openMenuSessionId]
  );

  const handleDelete = useCallback(
    async (event: React.MouseEvent, sessionId: string) => {
      event.stopPropagation();
      try {
        await flowChatManager.deleteChatSession(sessionId);
      } catch (error) {
        log.error('Failed to delete session', error);
      }
    },
    []
  );

  const handleStartEdit = useCallback(
    (event: React.MouseEvent, session: Session) => {
      event.stopPropagation();
      setEditingSessionId(session.sessionId);
      setEditingTitle(resolveDisplayTitle(session));
    },
    [resolveDisplayTitle]
  );

  const handleConfirmEdit = useCallback(async () => {
    if (!editingSessionId) return;
    const trimmedTitle = editingTitle.trim();
    if (trimmedTitle) {
      try {
        await flowChatManager.renameChatSessionTitle(editingSessionId, trimmedTitle);
      } catch (error) {
        log.error('Failed to update session title', error);
      }
    }
    setEditingSessionId(null);
    setEditingTitle('');
  }, [editingSessionId, editingTitle]);

  const handleCancelEdit = useCallback(() => {
    setEditingSessionId(null);
    setEditingTitle('');
  }, []);

  const handleEditKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      if (event.key === 'Enter') {
        event.preventDefault();
        handleConfirmEdit();
      } else if (event.key === 'Escape') {
        event.preventDefault();
        handleCancelEdit();
      }
    },
    [handleConfirmEdit, handleCancelEdit]
  );

  if (topLevelSessions.length === 0 && runningLiveApps.length === 0) {
    return null;
  }

  if (filteredVisibleItems.length === 0 && filteredRunningLiveApps.length === 0 && listFilterQuery?.trim()) {
    return (
      <div className="bitfun-nav-panel__inline-list bitfun-nav-panel__inline-list--filter-empty">
        <div className="bitfun-nav-panel__filter-empty">{t('nav.sessionCapsule.filterNoMatch')}</div>
      </div>
    );
  }

  return (
    <div className="bitfun-nav-panel__inline-list">
      {filteredRunningLiveApps.length > 0 ? (
        <>
          <div className="bitfun-nav-panel__inline-group-label">
            {t('nav.sessionCapsule.runningLiveAppsGroupLabel')}
          </div>
          {filteredRunningLiveApps.map((app: RunningLiveAppItem) => {
            const isRowActive = activeTabId === app.overlayId || activeLiveAppId === app.id;
            const row = (
              <div
                key={app.id}
                className={[
                  'bitfun-nav-panel__inline-item',
                  'is-live-app',
                  isRowActive && 'is-active',
                ].filter(Boolean).join(' ')}
                onClick={() => openOverlay(app.overlayId)}
              >
                <span className="bitfun-nav-panel__inline-item-icon is-live-app">
                  {renderLiveAppIcon(app.icon, 14)}
                </span>
                <span className="bitfun-nav-panel__inline-item-main">
                  <span className="bitfun-nav-panel__inline-item-label">{app.title}</span>
                  <span className="bitfun-nav-panel__inline-item-live-badge">
                    <LayoutGrid size={10} />
                    {t('nav.sessionCapsule.liveAppBadge')}
                  </span>
                </span>
              </div>
            );
            return (
              <Tooltip key={app.id} content={app.description || app.title} placement="right" followCursor>
                {row}
              </Tooltip>
            );
          })}
        </>
      ) : null}

      {filteredRunningLiveApps.length > 0 && filteredVisibleItems.length > 0 ? (
        <div className="bitfun-nav-panel__inline-group-label is-secondary">
          {t('nav.search.groupSessions')}
        </div>
      ) : null}

      {filteredVisibleItems.map(({ session, level }) => {
        const isEditing = editingSessionId === session.sessionId;
        const relationship = resolveSessionRelationship(session);
        const isBtwChild = level === 1 && relationship.isBtw;
        const sessionModeKey = resolveSessionModeType(session);
        const sessionTitle = resolveDisplayTitle(session);
        const parentSessionId = relationship.parentSessionId;
        const parentSession = parentSessionId ? flowChatState.sessions.get(parentSessionId) : undefined;
        const parentTitle = parentSession ? resolveDisplayTitle(parentSession) : '';
        const parentTurnIndex = relationship.origin?.parentTurnIndex;
        const rowWorkspace = listAllSessions
          ? findOpenedWorkspaceForSession(session, openedWorkspacesList)
          : undefined;
        const contextLabel = listAllSessions
          ? (rowWorkspace ? getWorkspaceDisplayName(rowWorkspace) : '')
          : (assistantLabel?.trim() ?? '');
        const showContextInTooltip = contextLabel.length > 0;
        const showRichTooltip = showContextInTooltip || isBtwChild;
        const tooltipContent = showRichTooltip ? (
          <div className="bitfun-nav-panel__inline-item-tooltip">
            <div className="bitfun-nav-panel__inline-item-tooltip-title">{sessionTitle}</div>
            {showContextInTooltip ? (
              <div className="bitfun-nav-panel__inline-item-tooltip-meta">
                {listAllSessions
                  ? t('nav.sessions.sessionContext', { name: contextLabel })
                  : t('nav.sessions.assistantOwner', { name: contextLabel })}
              </div>
            ) : null}
            {isBtwChild ? (
              <div className="bitfun-nav-panel__inline-item-tooltip-meta">
                {`来自 ${parentTitle || '父会话'}${parentTurnIndex ? ` · 第 ${parentTurnIndex} 轮` : ''}`}
              </div>
            ) : null}
          </div>
        ) : (
          sessionTitle
        );
        const SessionIcon =
          sessionModeKey === 'cowork'
            ? ListTodo
            : sessionModeKey === 'design'
              ? Brush
            : sessionModeKey === 'claw'
              ? Sparkles
              : Code2;
        const isRunning = runningSessionIds.has(session.sessionId);
        const isRowActive = activeBtwSessionData?.childSessionId
          ? session.sessionId === activeBtwSessionData.childSessionId
          : activeTabId === AGENT_SCENE && session.sessionId === activeSessionId;
        const row = (
          <div
            className={[
              'bitfun-nav-panel__inline-item',
              level === 1 && 'is-child',
              isBtwChild && 'is-btw-child',
              isRowActive && 'is-active',
              isEditing && 'is-editing',
              openMenuSessionId === session.sessionId && 'is-menu-open',
            ]
              .filter(Boolean)
              .join(' ')}
            onClick={() => handleSwitch(session.sessionId)}
          >
            {showSessionModeIcon && !isBtwChild ? (
              isRunning ? (
                <Loader2
                  size={14}
                  className={[
                    'bitfun-nav-panel__inline-item-icon',
                    'is-running',
                  ].join(' ')}
                />
              ) : (
                <SessionIcon
                  size={14}
                  className={[
                    'bitfun-nav-panel__inline-item-icon',
                    sessionModeKey === 'cowork'
                      ? 'is-cowork'
                      : sessionModeKey === 'design'
                        ? 'is-design'
                      : sessionModeKey === 'claw'
                        ? 'is-claw'
                        : 'is-code',
                  ].join(' ')}
                />
              )
            ) : null}

            {isEditing ? (
              <div className="bitfun-nav-panel__inline-item-edit" onClick={event => event.stopPropagation()}>
                <Input
                  ref={editInputRef}
                  className="bitfun-nav-panel__inline-item-edit-field"
                  variant="default"
                  inputSize="small"
                  value={editingTitle}
                  onChange={event => setEditingTitle(event.target.value)}
                  onKeyDown={handleEditKeyDown}
                  onBlur={handleConfirmEdit}
                />
                <IconButton
                  variant="success"
                  size="xs"
                  className="bitfun-nav-panel__inline-item-edit-btn confirm"
                  onClick={event => {
                    event.stopPropagation();
                    handleConfirmEdit();
                  }}
                  tooltip={t('nav.sessions.confirmEdit')}
                  tooltipPlacement="top"
                >
                  <Check size={11} />
                </IconButton>
                <IconButton
                  variant="default"
                  size="xs"
                  className="bitfun-nav-panel__inline-item-edit-btn cancel"
                  onMouseDown={event => {
                    event.preventDefault();
                    event.stopPropagation();
                    handleCancelEdit();
                  }}
                  tooltip={t('nav.sessions.cancelEdit')}
                  tooltipPlacement="top"
                >
                  <X size={11} />
                </IconButton>
              </div>
            ) : (
              <>
                <span className="bitfun-nav-panel__inline-item-main">
                  <span className="bitfun-nav-panel__inline-item-label">{sessionTitle}</span>
                  {isBtwChild ? (
                    <span className="bitfun-nav-panel__inline-item-btw-badge">btw</span>
                  ) : null}
                </span>
                <div className="bitfun-nav-panel__inline-item-actions">
                  <button
                    type="button"
                    className={`bitfun-nav-panel__inline-item-action-btn${openMenuSessionId === session.sessionId ? ' is-open' : ''}`}
                    onClick={event => handleMenuOpen(event, session.sessionId)}
                  >
                    <MoreHorizontal size={12} />
                  </button>
                </div>
                {openMenuSessionId === session.sessionId && sessionMenuPosition && createPortal(
                  <div
                    ref={sessionMenuPopoverRef}
                    className="bitfun-nav-panel__inline-item-menu-popover"
                    role="menu"
                    data-bitfun-ignore-session-capsule-outside
                    style={{ top: `${sessionMenuPosition.top}px`, left: `${sessionMenuPosition.left}px` }}
                  >
                    <button
                      type="button"
                      className="bitfun-nav-panel__inline-item-menu-item"
                      onClick={event => {
                        setOpenMenuSessionId(null);
                        handleStartEdit(event, session);
                      }}
                    >
                      <Pencil size={13} />
                      <span>{t('nav.sessions.rename')}</span>
                    </button>
                    <button
                      type="button"
                      className="bitfun-nav-panel__inline-item-menu-item is-danger"
                      onClick={event => {
                        setOpenMenuSessionId(null);
                        void handleDelete(event, session.sessionId);
                      }}
                    >
                      <Trash2 size={13} />
                      <span>{t('nav.sessions.delete')}</span>
                    </button>
                  </div>,
                  document.body
                )}
              </>
            )}
          </div>
        );
        return isEditing || openMenuSessionId !== null ? row : (
          <Tooltip key={session.sessionId} content={tooltipContent} placement="right" followCursor>
            {row}
          </Tooltip>
        );
      })}
    </div>
  );
};

export default SessionList;

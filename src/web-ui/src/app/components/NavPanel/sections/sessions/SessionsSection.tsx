/**
 * SessionsSection — inline accordion content for the "Sessions" nav item.
 *
 * Rendered inside NavPanel when the Sessions item is expanded.
 * Owns all data fetching / mutation for chat sessions.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Pencil, Trash2, Check, X, Bot, Code2, ClipboardList, Panda, MoreHorizontal, Loader2 } from 'lucide-react';
import { IconButton, Input, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { flowChatStore } from '../../../../../flow_chat/store/FlowChatStore';
import { flowChatManager } from '../../../../../flow_chat/services/FlowChatManager';
import type { FlowChatState, Session } from '../../../../../flow_chat/types/flow-chat';
import { useSceneStore } from '../../../../stores/sceneStore';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
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
  sessionBelongsToWorkspaceNavRow,
} from '@/flow_chat/utils/sessionOrdering';
import { stateMachineManager } from '@/flow_chat/state-machine';
import { SessionExecutionState } from '@/flow_chat/state-machine/types';
import { i18nService } from '@/infrastructure/i18n';
import { resolveSessionTitle } from '@/flow_chat/utils/sessionTitle';
import { isSessionNavRowActive } from './sessionNavSelection';
import {
  deriveSessionReviewActivity,
  isReviewActivityBlocking,
} from '@/flow_chat/utils/sessionReviewActivity';
import './SessionsSection.scss';

/** Top-level parent sessions shown at each expand step (children still nest under visible parents). */
const SESSIONS_LEVEL_0 = 5;
const SESSIONS_LEVEL_1 = 10;
const log = createLogger('SessionsSection');

type SessionMode = 'code' | 'cowork' | 'claw';

const escapeRegExp = (value: string): string =>
  value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const resolveSessionModeType = (session: Session): SessionMode => {
  const normalizedMode = session.mode?.toLowerCase();
  if (normalizedMode === 'cowork') return 'cowork';
  if (normalizedMode === 'claw') return 'claw';
  return 'code';
};

const getTitle = (session: Session): string =>
  resolveSessionTitle(session, (key, options) => i18nService.t(key, options));

const getChildSessionBadge = (kind: Session['sessionKind']): string => {
  const normalizedKind = kind === 'review' || kind === 'deep_review' ? kind : 'btw';
  const fallback = normalizedKind === 'deep_review'
    ? 'Deep'
    : normalizedKind === 'review'
      ? 'Review'
      : 'btw';
  return i18nService.t(`flow-chat:childSession.kinds.${normalizedKind}.short`, {
    defaultValue: fallback,
  });
};

const getReviewActivityBadge = (kind: 'review' | 'deep_review'): string =>
  i18nService.t(
    kind === 'deep_review'
      ? 'common:nav.sessions.deepReviewRunning'
      : 'common:nav.sessions.reviewRunning',
    {
      defaultValue: kind === 'deep_review' ? 'Deep reviewing' : 'Reviewing',
    },
  );

interface SessionsSectionProps {
  workspaceId?: string;
  workspacePath?: string;
  /** Remote SSH: same `workspacePath` on different hosts must filter by this (see Session.remoteConnectionId). */
  remoteConnectionId?: string | null;
  /** Remote SSH: disambiguates same path on different hosts; when set with matching session host, connectionId may differ. */
  remoteSshHost?: string | null;
  isActiveWorkspace?: boolean;
  showCreateActions?: boolean;
  /** When set (e.g. assistant workspace), session row tooltip includes this assistant name. */
  assistantLabel?: string;
  /** When false, hide the leading mode / running icon on each row (e.g. assistant detail page). */
  showSessionModeIcon?: boolean;
}

const SessionsSection: React.FC<SessionsSectionProps> = ({
  workspaceId,
  workspacePath,
  remoteConnectionId = null,
  remoteSshHost = null,
  isActiveWorkspace: _isActiveWorkspace = true,
  assistantLabel,
  showSessionModeIcon = true,
}) => {
  const { t } = useI18n('common');
  const { setActiveWorkspace, currentWorkspace } = useWorkspaceContext();
  const activeTabId = useSceneStore(s => s.activeTabId);
  const activeBtwSessionTab = useAgentCanvasStore(state => selectActiveBtwSessionTab(state as any));
  const activeBtwSessionData = activeBtwSessionTab?.content.data as
    | { childSessionId: string; parentSessionId: string; workspacePath?: string }
    | undefined;
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() =>
    flowChatStore.getState()
  );
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState('');
  const [expandLevel, setExpandLevel] = useState<0 | 1 | 2>(0);
  const [openMenuSessionId, setOpenMenuSessionId] = useState<string | null>(null);
  const [sessionMenuPosition, setSessionMenuPosition] = useState<{ top: number; left: number } | null>(null);
  const [runningSessionIds, setRunningSessionIds] = useState<Set<string>>(new Set());
  const editInputRef = useRef<HTMLInputElement>(null);
  const sessionMenuPopoverRef = useRef<HTMLDivElement>(null);

  // Subscribe to state machine changes for running status
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
    const unsub = flowChatStore.subscribe(s => setFlowChatState(s));
    return () => unsub();
  }, []);

  useEffect(() => {
    if (editingSessionId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingSessionId]);

  useEffect(() => {
    setExpandLevel(0);
  }, [workspaceId, workspacePath, remoteConnectionId, remoteSshHost]);

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

  // Clear unread completion mark after the switched session renders
  useEffect(() => {
    const handleSessionSwitched = (e: Event) => {
      const { sessionId } = (e as CustomEvent).detail;
      if (!sessionId) return;
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          flowChatStore.clearSessionUnreadCompletion(sessionId);
          flowChatStore.clearSessionNeedsAttention(sessionId);
        });
      });
    };

    window.addEventListener('bitfun:session-switched', handleSessionSwitched);
    return () => window.removeEventListener('bitfun:session-switched', handleSessionSwitched);
  }, []);

  const sessions = useMemo(
    () =>
      Array.from(flowChatState.sessions.values())
        .filter((s: Session) => {
          if (workspacePath) {
            return sessionBelongsToWorkspaceNavRow(s, workspacePath, remoteConnectionId, remoteSshHost);
          }
          return !s.workspacePath;
        })
        .sort(compareSessionsForDisplay),
    [flowChatState.sessions, workspacePath, remoteConnectionId, remoteSshHost]
  );

  const { topLevelSessions, childrenByParent } = useMemo(() => {
    const childMap = new Map<string, Session[]>();
    const parents: Session[] = [];

    const knownIds = new Set(sessions.map(s => s.sessionId));

    for (const s of sessions) {
      const pid = resolveSessionRelationship(s).parentSessionId;
      if (pid && typeof pid === 'string' && pid.trim() && knownIds.has(pid)) {
        const list = childMap.get(pid) || [];
        list.push(s);
        childMap.set(pid, list);
      } else {
        parents.push(s);
      }
    }

    for (const [pid, list] of childMap) {
      childMap.set(pid, [...list].sort(compareSessionsForDisplay));
    }

    return {
      topLevelSessions: [...parents].sort(compareSessionsForDisplay),
      childrenByParent: childMap,
    };
  }, [sessions]);

  const sessionDisplayLimit = useMemo(() => {
    const total = topLevelSessions.length;
    if (expandLevel === 2 || total <= SESSIONS_LEVEL_0) return total;
    if (expandLevel === 1) return Math.min(total, SESSIONS_LEVEL_1);
    return SESSIONS_LEVEL_0;
  }, [topLevelSessions.length, expandLevel]);

  const visibleItems = useMemo(() => {
    const visibleParents = topLevelSessions.slice(0, sessionDisplayLimit);
    const out: Array<{ session: Session; level: 0 | 1 }> = [];
    for (const p of visibleParents) {
      out.push({ session: p, level: 0 });
      const children = childrenByParent.get(p.sessionId) || [];
      for (const c of children) out.push({ session: c, level: 1 });
    }
    return out;
  }, [childrenByParent, sessionDisplayLimit, topLevelSessions]);

  const activeSessionId = flowChatState.activeSessionId;

  const handleSwitch = useCallback(
    async (sessionId: string) => {
      if (editingSessionId) return;
      try {
        const session = flowChatStore.getState().sessions.get(sessionId);
        const relationship = resolveSessionRelationship(session);
        const parentSessionId = relationship.parentSessionId;
        const mustActivateWorkspace =
          Boolean(workspaceId) && workspaceId !== currentWorkspace?.id;
        const activateWorkspace = mustActivateWorkspace
          ? async (targetWorkspaceId: string) => {
              await setActiveWorkspace(targetWorkspaceId);
            }
          : undefined;

        if (relationship.canOpenInAuxPane && parentSessionId && session) {
          await openMainSession(parentSessionId, {
            workspaceId,
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
            workspaceId,
            activateWorkspace,
          });
          return;
        }

        await openMainSession(sessionId, {
          workspaceId,
          activateWorkspace,
        });
        window.dispatchEvent(
          new CustomEvent('flowchat:switch-session', { detail: { sessionId } })
        );
      } catch (err) {
        log.error('Failed to switch session', err);
      }
    },
    [
      activeSessionId,
      editingSessionId,
      setActiveWorkspace,
      workspaceId,
      currentWorkspace?.id,
    ]
  );

  const resolveSessionTitle = useCallback(
    (session: Session): string => {
      const rawTitle = getTitle(session);
      const newSessionPrefixes = Array.from(
        new Set([
          t('nav.sessions.newSession'),
          i18nService.t('nav.sessions.newSession', { lng: 'en-US' }),
          i18nService.t('nav.sessions.newSession', { lng: 'zh-CN' }),
          i18nService.t('nav.sessions.newSession', { lng: 'zh-TW' }),
        ].filter((value): value is string => Boolean(value)))
      );
      const matched = rawTitle.match(
        new RegExp(`^(?:${newSessionPrefixes.map(escapeRegExp).join('|')})\\s*(\\d+)$`, 'i')
      );
      if (!matched) return rawTitle;

      const mode = resolveSessionModeType(session);
      const label =
        mode === 'cowork'
          ? t('nav.sessions.newCoworkSession')
          : mode === 'claw'
            ? t('nav.sessions.newClawSession')
            : t('nav.sessions.newCodeSession');
      return `${label} ${matched[1]}`;
    },
    [t]
  );

  const handleMenuOpen = useCallback(
    (e: React.MouseEvent, sessionId: string) => {
      e.stopPropagation();
      if (openMenuSessionId === sessionId) {
        setOpenMenuSessionId(null);
        setSessionMenuPosition(null);
        return;
      }
      const btn = e.currentTarget as HTMLElement;
      const rect = btn.getBoundingClientRect();
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
    async (e: React.MouseEvent, sessionId: string) => {
      e.stopPropagation();
      try {
        await flowChatManager.deleteChatSession(sessionId);
      } catch (err) {
        log.error('Failed to delete session', err);
      }
    },
    []
  );

  const handleStartEdit = useCallback(
    (e: React.MouseEvent, session: Session) => {
      e.stopPropagation();
      setEditingSessionId(session.sessionId);
      setEditingTitle(resolveSessionTitle(session));
    },
    [resolveSessionTitle]
  );

  const handleConfirmEdit = useCallback(async () => {
    if (!editingSessionId) return;
    const trimmed = editingTitle.trim();
    if (trimmed) {
      try {
        await flowChatManager.renameChatSessionTitle(editingSessionId, trimmed);
      } catch (err) {
        log.error('Failed to update session title', err);
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
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        handleConfirmEdit();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        handleCancelEdit();
      }
    },
    [handleConfirmEdit, handleCancelEdit]
  );

  if (topLevelSessions.length === 0) {
    return null;
  }

  return (
    <div className="bitfun-nav-panel__inline-list">
      {visibleItems.map(({ session, level }) => {
          const isEditing = editingSessionId === session.sessionId;
          const relationship = resolveSessionRelationship(session);
          const isChildSession = level === 1 && relationship.displayAsChild;
          const childSessionBadge = getChildSessionBadge(relationship.kind);
          const parentReviewActivity = deriveSessionReviewActivity(
            flowChatState,
            session.sessionId,
            id => stateMachineManager.getCurrentState(id),
          );
          const showParentReviewActivity = !isChildSession && isReviewActivityBlocking(parentReviewActivity);
          const showChildReviewActivity =
            isChildSession && relationship.isReview && runningSessionIds.has(session.sessionId);
          const reviewActivityKind =
            showParentReviewActivity
              ? parentReviewActivity!.kind
              : showChildReviewActivity && (relationship.kind === 'review' || relationship.kind === 'deep_review')
                ? relationship.kind
                : null;
          const sessionModeKey = resolveSessionModeType(session);
          const sessionTitle = resolveSessionTitle(session);
          const parentSessionId = relationship.parentSessionId;
          const parentSession = parentSessionId ? flowChatState.sessions.get(parentSessionId) : undefined;
          const parentTitle = parentSession ? resolveSessionTitle(parentSession) : '';
          const parentTurnIndex = relationship.origin?.parentTurnIndex;
          const trimmedAssistant = assistantLabel?.trim() ?? '';
          const showAssistantInTooltip = trimmedAssistant.length > 0;
          const showRichTooltip = showAssistantInTooltip || isChildSession;
          const tooltipContent = showRichTooltip ? (
            <div className="bitfun-nav-panel__inline-item-tooltip">
              <div className="bitfun-nav-panel__inline-item-tooltip-title">{sessionTitle}</div>
              {showAssistantInTooltip ? (
                <div className="bitfun-nav-panel__inline-item-tooltip-meta">
                  {t('nav.sessions.assistantOwner', { name: trimmedAssistant })}
                </div>
              ) : null}
              {isChildSession ? (
                <div className="bitfun-nav-panel__inline-item-tooltip-meta">
                  {parentTurnIndex
                    ? t('nav.sessions.childSourceWithTurn', {
                        parentTitle: parentTitle || t('nav.sessions.parentSession'),
                        turnIndex: parentTurnIndex,
                      })
                    : t('nav.sessions.childSourceWithoutTurn', {
                        parentTitle: parentTitle || t('nav.sessions.parentSession'),
                      })}
                </div>
              ) : null}
            </div>
          ) : (
            sessionTitle
          );
          const SessionIcon =
            sessionModeKey === 'cowork'
              ? ClipboardList
              : sessionModeKey === 'claw'
                ? showAssistantInTooltip
                  ? Panda
                  : Bot
                : Code2;
          const isRunning = runningSessionIds.has(session.sessionId);
          const isRowActive = isSessionNavRowActive({
            rowSessionId: session.sessionId,
            activeTabId,
            activeSessionId,
            activeChildSessionId: activeBtwSessionData?.childSessionId,
            activeChildParentSessionId: activeBtwSessionData?.parentSessionId,
          });
          // Determine the notification state for this session row.
          // Priority: needsUserAttention > hasUnreadCompletion.
          const attentionKind = !isRunning && !isRowActive
            ? (session.needsUserAttention || session.hasUnreadCompletion || undefined)
            : undefined;
          const isHighPriority = !!session.needsUserAttention;
          const row = (
            <div
              className={[
                'bitfun-nav-panel__inline-item',
                level === 1 && 'is-child',
                isChildSession && 'is-btw-child',
                isRowActive && 'is-active',
                isEditing && 'is-editing',
              ]
                .filter(Boolean)
                .join(' ')}
              onClick={() => handleSwitch(session.sessionId)}
            >
              {showSessionModeIcon ? (
                <span className="bitfun-nav-panel__inline-item-icon-slot">
                  {isRunning ? (
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
                          : sessionModeKey === 'claw'
                            ? 'is-claw'
                            : 'is-code',
                      ].join(' ')}
                    />
                  )}
                  {attentionKind ? (
                    <span
                      className={[
                        'bitfun-nav-panel__inline-item-unread-dot',
                        attentionKind === 'error' && 'is-error',
                        attentionKind === 'ask_user' && 'is-ask-user',
                        attentionKind === 'tool_confirm' && 'is-tool-confirm',
                        isHighPriority && 'is-high-priority',
                      ].filter(Boolean).join(' ')}
                      aria-label={
                        attentionKind === 'error'
                          ? t('nav.sessions.unreadError')
                          : attentionKind === 'ask_user'
                            ? t('nav.sessions.needsUserInput')
                            : attentionKind === 'tool_confirm'
                              ? t('nav.sessions.needsToolConfirm')
                              : t('nav.sessions.unreadCompleted')
                      }
                    />
                  ) : null}
                </span>
              ) : null}

              {isEditing ? (
                <div className="bitfun-nav-panel__inline-item-edit" onClick={e => e.stopPropagation()}>
                  <Input
                    ref={editInputRef}
                    className="bitfun-nav-panel__inline-item-edit-field"
                    variant="default"
                    inputSize="small"
                    value={editingTitle}
                    onChange={e => setEditingTitle(e.target.value)}
                    onKeyDown={handleEditKeyDown}
                    onBlur={handleConfirmEdit}
                  />
                  <IconButton
                    variant="success"
                    size="xs"
                    className="bitfun-nav-panel__inline-item-edit-btn confirm"
                    onClick={e => { e.stopPropagation(); handleConfirmEdit(); }}
                    tooltip={t('nav.sessions.confirmEdit')}
                    tooltipPlacement="top"
                  >
                    <Check size={11} />
                  </IconButton>
                  <IconButton
                    variant="default"
                    size="xs"
                    className="bitfun-nav-panel__inline-item-edit-btn cancel"
                    onMouseDown={e => { e.preventDefault(); e.stopPropagation(); handleCancelEdit(); }}
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
                    {isChildSession ? (
                      <span className="bitfun-nav-panel__inline-item-btw-badge">{childSessionBadge}</span>
                    ) : null}
                    {attentionKind === 'ask_user' || attentionKind === 'tool_confirm' ? (
                      <span className="bitfun-nav-panel__inline-item-attention-badge">
                        {attentionKind === 'ask_user'
                          ? t('nav.sessions.badgeNeedsInput')
                          : t('nav.sessions.badgeNeedsConfirm')}
                      </span>
                    ) : null}
                    {reviewActivityKind ? (
                      <span className="bitfun-nav-panel__inline-item-review-badge">
                        <Loader2 size={9} aria-hidden />
                        {getReviewActivityBadge(reviewActivityKind)}
                      </span>
                    ) : null}
                  </span>
                  <div className="bitfun-nav-panel__inline-item-actions">
                    <button
                      type="button"
                      className={`bitfun-nav-panel__inline-item-action-btn${openMenuSessionId === session.sessionId ? ' is-open' : ''}`}
                      onClick={e => handleMenuOpen(e, session.sessionId)}
                    >
                      <MoreHorizontal size={12} />
                    </button>
                  </div>
                  {openMenuSessionId === session.sessionId && sessionMenuPosition && createPortal(
                    <div
                      ref={sessionMenuPopoverRef}
                      className="bitfun-nav-panel__inline-item-menu-popover"
                      role="menu"
                      style={{ top: `${sessionMenuPosition.top}px`, left: `${sessionMenuPosition.left}px` }}
                    >
                      <button
                        type="button"
                        className="bitfun-nav-panel__inline-item-menu-item"
                        onClick={e => { setOpenMenuSessionId(null); handleStartEdit(e, session); }}
                      >
                        <Pencil size={13} />
                        <span>{t('nav.sessions.rename')}</span>
                      </button>
                      <button
                        type="button"
                        className="bitfun-nav-panel__inline-item-menu-item is-danger"
                        onClick={e => { setOpenMenuSessionId(null); void handleDelete(e, session.sessionId); }}
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

      {topLevelSessions.length > SESSIONS_LEVEL_0 && (
        <button
          type="button"
          className="bitfun-nav-panel__inline-toggle"
          onClick={() => {
            const total = topLevelSessions.length;
            if (expandLevel === 0) {
              setExpandLevel(1);
              return;
            }
            if (expandLevel === 1 && total > SESSIONS_LEVEL_1) {
              setExpandLevel(2);
              return;
            }
            setExpandLevel(0);
          }}
        >
          {expandLevel === 0 ? (
            <>
              <span className="bitfun-nav-panel__inline-toggle-dots">···</span>
              <span>
                {t('nav.sessions.showMore', {
                  count: topLevelSessions.length - SESSIONS_LEVEL_0,
                })}
              </span>
            </>
          ) : expandLevel === 1 && topLevelSessions.length > SESSIONS_LEVEL_1 ? (
            <>
              <span className="bitfun-nav-panel__inline-toggle-dots">···</span>
              <span>
                {t('nav.sessions.showAll', {
                  count: topLevelSessions.length - SESSIONS_LEVEL_1,
                })}
              </span>
            </>
          ) : (
            <span>{t('nav.sessions.showLess')}</span>
          )}
        </button>
      )}
    </div>
  );
};

export default SessionsSection;

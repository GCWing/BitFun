/**
 * SessionsSection — inline accordion content for the "Sessions" nav item.
 *
 * Rendered inside NavPanel when the Sessions item is expanded.
 * Owns all data fetching / mutation for chat sessions.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Pencil, Trash2, Check, X, Bot, Code2, Users } from 'lucide-react';
import { IconButton, Input, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { flowChatStore } from '../../../../../flow_chat/store/FlowChatStore';
import { flowChatManager } from '../../../../../flow_chat/services/FlowChatManager';
import type { FlowChatState, Session } from '../../../../../flow_chat/types/flow-chat';
import { useSceneStore } from '../../../../stores/sceneStore';
import { useApp } from '../../../../hooks/useApp';
import type { SceneTabId } from '../../../SceneBar/types';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { createLogger } from '@/shared/utils/logger';
import './SessionsSection.scss';

const MAX_VISIBLE_SESSIONS = 8;
const INACTIVE_WORKSPACE_COLLAPSED_SESSIONS = 3;
const INACTIVE_WORKSPACE_EXPANDED_SESSIONS = 7;
const log = createLogger('SessionsSection');
const AGENT_SCENE: SceneTabId = 'session';

type SessionMode = 'code' | 'cowork' | 'claw';

const resolveSessionModeType = (session: Session): SessionMode => {
  const normalizedMode = session.mode?.toLowerCase();
  if (normalizedMode === 'cowork') return 'cowork';
  if (normalizedMode === 'claw') return 'claw';
  return 'code';
};

const getTitle = (session: Session): string =>
  session.title?.trim() || `Session ${session.sessionId.slice(0, 6)}`;

interface SessionsSectionProps {
  workspaceId?: string;
  workspacePath?: string;
  isActiveWorkspace?: boolean;
  showCreateActions?: boolean;
}

const SessionsSection: React.FC<SessionsSectionProps> = ({
  workspaceId,
  workspacePath,
  isActiveWorkspace = true,
}) => {
  const { t } = useI18n('common');
  const { switchLeftPanelTab } = useApp();
  const { setActiveWorkspace } = useWorkspaceContext();
  const openScene = useSceneStore(s => s.openScene);
  const activeTabId = useSceneStore(s => s.activeTabId);
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() =>
    flowChatStore.getState()
  );
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState('');
  const [showAll, setShowAll] = useState(false);
  const editInputRef = useRef<HTMLInputElement>(null);

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
    setShowAll(false);
  }, [workspaceId, workspacePath, isActiveWorkspace]);

  const sessions = useMemo(
    () =>
      Array.from(flowChatState.sessions.values())
        .filter((s: Session) => {
          if (workspacePath) {
            return s.workspacePath === workspacePath;
          }
          return !s.workspacePath;
        })
        .sort(
          (a: Session, b: Session) => b.lastActiveAt - a.lastActiveAt
        ),
    [flowChatState.sessions, workspacePath]
  );

  const { topLevelSessions, childrenByParent } = useMemo(() => {
    const childMap = new Map<string, Session[]>();
    const parents: Session[] = [];

    const knownIds = new Set(sessions.map(s => s.sessionId));

    for (const s of sessions) {
      const pid = s.parentSessionId;
      if (pid && typeof pid === 'string' && pid.trim() && knownIds.has(pid)) {
        const list = childMap.get(pid) || [];
        list.push(s);
        childMap.set(pid, list);
      } else {
        parents.push(s);
      }
    }

    // Stable ordering: children follow parent, sorted by activity.
    for (const [pid, list] of childMap) {
      childMap.set(pid, [...list].sort((a, b) => b.lastActiveAt - a.lastActiveAt));
    }

    return {
      topLevelSessions: [...parents].sort((a, b) => b.lastActiveAt - a.lastActiveAt),
      childrenByParent: childMap,
    };
  }, [sessions]);

  const sessionDisplayLimit = useMemo(() => {
    if (isActiveWorkspace) {
      return showAll || topLevelSessions.length <= MAX_VISIBLE_SESSIONS
        ? topLevelSessions.length
        : MAX_VISIBLE_SESSIONS;
    }

    return showAll
      ? Math.min(topLevelSessions.length, INACTIVE_WORKSPACE_EXPANDED_SESSIONS)
      : Math.min(topLevelSessions.length, INACTIVE_WORKSPACE_COLLAPSED_SESSIONS);
  }, [isActiveWorkspace, topLevelSessions.length, showAll]);

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

  const toggleThreshold = isActiveWorkspace
    ? MAX_VISIBLE_SESSIONS
    : INACTIVE_WORKSPACE_COLLAPSED_SESSIONS;
  const hiddenCount = Math.max(0, topLevelSessions.length - toggleThreshold);

  const activeSessionId = flowChatState.activeSessionId;

  const handleSwitch = useCallback(
    async (sessionId: string) => {
      if (editingSessionId) return;
      openScene('session');
      switchLeftPanelTab('sessions');
      if (sessionId === activeSessionId) return;
      try {
        if (workspaceId && !isActiveWorkspace) {
          await setActiveWorkspace(workspaceId);
        }
        await flowChatManager.switchChatSession(sessionId);
        window.dispatchEvent(
          new CustomEvent('flowchat:switch-session', { detail: { sessionId } })
        );
      } catch (err) {
        log.error('Failed to switch session', err);
      }
    },
    [activeSessionId, editingSessionId, isActiveWorkspace, openScene, setActiveWorkspace, switchLeftPanelTab, workspaceId]
  );

  const resolveSessionTitle = useCallback(
    (session: Session): string => {
      const rawTitle = getTitle(session);
      const matched = rawTitle.match(/^(?:新建会话|New Session)\s*(\d+)$/i);
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
        await flowChatStore.updateSessionTitle(editingSessionId, trimmed, 'generated');
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

  return (
    <div className="bitfun-nav-panel__inline-list">
      {topLevelSessions.length === 0 ? (
        <div className="bitfun-nav-panel__inline-empty">{t('nav.sessions.noSessions')}</div>
      ) : (
        visibleItems.map(({ session, level }) => {
          const isEditing = editingSessionId === session.sessionId;
          const sessionModeKey = resolveSessionModeType(session);
          const sessionTitle = resolveSessionTitle(session);
          const SessionIcon =
            sessionModeKey === 'cowork'
              ? Users
              : sessionModeKey === 'claw'
                ? Bot
                : Code2;
          const row = (
            <div
              className={[
                'bitfun-nav-panel__inline-item',
                level === 1 && 'is-child',
                activeTabId === AGENT_SCENE && session.sessionId === activeSessionId && 'is-active',
                isEditing && 'is-editing',
              ]
                .filter(Boolean)
                .join(' ')}
              onClick={() => handleSwitch(session.sessionId)}
            >
              <SessionIcon
                size={12}
                className={[
                  'bitfun-nav-panel__inline-item-icon',
                  sessionModeKey === 'cowork'
                    ? 'is-cowork'
                    : sessionModeKey === 'claw'
                      ? 'is-claw'
                      : 'is-code',
                ].join(' ')}
              />

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
                  <span className="bitfun-nav-panel__inline-item-label">{sessionTitle}</span>
                  <div className="bitfun-nav-panel__inline-item-actions">
                    <IconButton
                      variant="default"
                      size="xs"
                      className="bitfun-nav-panel__inline-item-action-btn"
                      onClick={e => handleStartEdit(e, session)}
                      tooltip={t('nav.sessions.rename')}
                      tooltipPlacement="top"
                    >
                      <Pencil size={11} />
                    </IconButton>
                    <IconButton
                      variant="danger"
                      size="xs"
                      className="bitfun-nav-panel__inline-item-action-btn delete"
                      onClick={e => handleDelete(e, session.sessionId)}
                      tooltip={t('nav.sessions.delete')}
                      tooltipPlacement="top"
                    >
                      <Trash2 size={11} />
                    </IconButton>
                  </div>
                </>
              )}
            </div>
          );
          return isEditing ? row : (
            <Tooltip key={session.sessionId} content={sessionTitle} placement="right" followCursor>
              {row}
            </Tooltip>
          );
        })
      )}

      {topLevelSessions.length > toggleThreshold && (
        <button
          type="button"
          className="bitfun-nav-panel__inline-toggle"
          onClick={() => setShowAll(prev => !prev)}
        >
          {showAll ? (
            <span>{t('nav.sessions.showLess')}</span>
          ) : (
            <>
              <span className="bitfun-nav-panel__inline-toggle-dots">···</span>
              <span>{t('nav.sessions.showMore', { count: hiddenCount })}</span>
            </>
          )}
        </button>
      )}
    </div>
  );
};

export default SessionsSection;

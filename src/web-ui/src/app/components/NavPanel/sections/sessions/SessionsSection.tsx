/**
 * SessionsSection — inline accordion content for the "Sessions" nav item.
 *
 * Rendered inside NavPanel when the Sessions item is expanded.
 * Owns all data fetching / mutation for chat sessions.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Plus, MessageSquareText, Pencil, Trash2, Check, X, Code2, Users } from 'lucide-react';
import { IconButton, Input, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { flowChatStore } from '../../../../../flow_chat/store/FlowChatStore';
import { flowChatManager } from '../../../../../flow_chat/services/FlowChatManager';
import type { FlowChatState, Session } from '../../../../../flow_chat/types/flow-chat';
import { useSceneStore } from '../../../../stores/sceneStore';
import { useSessionModeStore } from '../../../../stores/sessionModeStore';
import type { SessionMode } from '../../../../stores/sessionModeStore';
import { useApp } from '../../../../hooks/useApp';
import type { SceneTabId } from '../../../SceneBar/types';
import { createLogger } from '@/shared/utils/logger';
import './SessionsSection.scss';

const MAX_VISIBLE_SESSIONS = 8;
const log = createLogger('SessionsSection');
const AGENT_SCENE: SceneTabId = 'session';

const SESSION_MODES: { key: SessionMode; Icon: typeof Code2; labelKey: string }[] = [
  { key: 'code',   Icon: Code2, labelKey: 'nav.sessions.modeCode' },
  { key: 'cowork', Icon: Users, labelKey: 'nav.sessions.modeCowork' },
];

const getTitle = (session: Session): string =>
  session.title?.trim() || `Session ${session.sessionId.slice(0, 6)}`;

const SessionsSection: React.FC = () => {
  const { t } = useI18n('common');
  const { switchLeftPanelTab } = useApp();
  const openScene = useSceneStore(s => s.openScene);
  const sessionMode = useSessionModeStore(s => s.mode);
  const setSessionMode = useSessionModeStore(s => s.setMode);
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

  const sessions = useMemo(
    () =>
      Array.from(flowChatState.sessions.values()).sort(
        (a: Session, b: Session) => b.lastActiveAt - a.lastActiveAt
      ),
    [flowChatState.sessions]
  );

  const visibleSessions = useMemo(
    () => (showAll || sessions.length <= MAX_VISIBLE_SESSIONS ? sessions : sessions.slice(0, MAX_VISIBLE_SESSIONS)),
    [sessions, showAll]
  );

  const hiddenCount = sessions.length - MAX_VISIBLE_SESSIONS;

  const activeSessionId = flowChatState.activeSessionId;

  const handleSwitch = useCallback(
    async (sessionId: string) => {
      if (editingSessionId) return;
      openScene('session');
      switchLeftPanelTab('sessions');
      if (sessionId === activeSessionId) return;
      try {
        await flowChatManager.switchChatSession(sessionId);
        window.dispatchEvent(
          new CustomEvent('flowchat:switch-session', { detail: { sessionId } })
        );
      } catch (err) {
        log.error('Failed to switch session', err);
      }
    },
    [activeSessionId, openScene, switchLeftPanelTab, editingSessionId]
  );

  const handleCreate = useCallback(async () => {
    if (sessionMode === 'cowork') {
      log.info('Cowork session mode is not yet available');
      return;
    }
    openScene('session');
    switchLeftPanelTab('sessions');
    try {
      await flowChatManager.createChatSession({ modelName: 'claude-sonnet-4.5' });
    } catch (err) {
      log.error('Failed to create session', err);
    }
  }, [openScene, switchLeftPanelTab, sessionMode]);

  const handleModeSwitch = useCallback((mode: SessionMode) => {
    setSessionMode(mode);
  }, [setSessionMode]);

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
      setEditingTitle(getTitle(session));
    },
    []
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
      <div className="bitfun-nav-panel__inline-action-row">
        <Tooltip content={t('nav.sessions.newSession')} placement="right" followCursor>
          <button
            type="button"
            className={`bitfun-nav-panel__inline-action${sessionMode === 'cowork' ? ' is-placeholder' : ''}`}
            onClick={handleCreate}
          >
            <Plus size={12} />
            <span>
              {sessionMode === 'code'
                ? t('nav.sessions.newCodeSession')
                : t('nav.sessions.newCoworkSession')}
            </span>
          </button>
        </Tooltip>
        <div className="bitfun-nav-panel__mode-switcher">
          {SESSION_MODES.map(({ key, Icon, labelKey }) => (
            <Tooltip key={key} content={t(labelKey)} placement="top" followCursor>
              <span
                className={`bitfun-nav-panel__mode-chip${sessionMode === key ? ' is-active' : ''}${key === 'cowork' ? ' is-placeholder' : ''}`}
                role="button"
                tabIndex={-1}
                aria-label={t(labelKey)}
                aria-pressed={sessionMode === key}
                onClick={() => handleModeSwitch(key)}
              >
                <Icon size={sessionMode === key ? 11 : 9} />
              </span>
            </Tooltip>
          ))}
        </div>
      </div>

      {sessions.length === 0 ? (
        <div className="bitfun-nav-panel__inline-empty">{t('nav.sessions.noSessions')}</div>
      ) : (
        visibleSessions.map(session => {
          const isEditing = editingSessionId === session.sessionId;
          const sessionTitle = getTitle(session);
          const row = (
            <div
              key={session.sessionId}
              className={[
                'bitfun-nav-panel__inline-item',
                activeTabId === AGENT_SCENE && session.sessionId === activeSessionId && 'is-active',
                isEditing && 'is-editing',
              ]
                .filter(Boolean)
                .join(' ')}
              onClick={() => handleSwitch(session.sessionId)}
            >
              <MessageSquareText size={12} className="bitfun-nav-panel__inline-item-icon" />

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

      {sessions.length > MAX_VISIBLE_SESSIONS && (
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

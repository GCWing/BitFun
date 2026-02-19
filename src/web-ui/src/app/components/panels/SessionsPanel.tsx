/**
 * Session list panel component.
 * Displays all chat sessions; switching and management only.
 * Capabilities (Skills / Subagents / MCP) have been moved to CapabilitiesPanel.
 */

import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Pencil, Check, Loader2, MessageSquareText } from 'lucide-react';
import { flowChatStore } from '../../../flow_chat/store/FlowChatStore';
import { flowChatManager } from '../../../flow_chat/services/FlowChatManager';
import type { FlowChatState, Session } from '../../../flow_chat/types/flow-chat';
import { stateMachineManager } from '../../../flow_chat/state-machine/SessionStateMachineManager';
import { SessionExecutionState } from '../../../flow_chat/state-machine/types';
import { Search, Button, IconButton, Tooltip } from '@/component-library';
import { PanelHeader } from './base';
import { createLogger } from '@/shared/utils/logger';
import './SessionsPanel.scss';

const log = createLogger('SessionsPanel');

const ONE_HOUR_MS = 60 * 60 * 1000;

const SessionsPanel: React.FC = () => {
  const { t, i18n } = useTranslation('panels/sessions');

  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() =>
    flowChatStore.getState()
  );
  const [searchQuery, setSearchQuery] = useState('');
  const [isRecentCollapsed, setIsRecentCollapsed] = useState(false);
  const [isOldCollapsed, setIsOldCollapsed] = useState(false);
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState('');
  const editInputRef = useRef<HTMLInputElement>(null);
  const [processingSessionIds, setProcessingSessionIds] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    const unsubscribe = flowChatStore.subscribe(state => setFlowChatState(state));
    return () => unsubscribe();
  }, []);

  useEffect(() => {
    const unsubscribe = stateMachineManager.subscribeGlobal((sessionId, machine) => {
      const isProcessing = machine.currentState === SessionExecutionState.PROCESSING;
      setProcessingSessionIds(prev => {
        const next = new Set(prev);
        if (isProcessing) { next.add(sessionId); } else { next.delete(sessionId); }
        if (next.size !== prev.size || [...next].some(id => !prev.has(id))) return next;
        return prev;
      });
    });
    return () => unsubscribe();
  }, []);

  const allSessions = useMemo(() =>
    Array.from(flowChatState.sessions.values()).sort(
      (a: Session, b: Session) => b.createdAt - a.createdAt
    ),
    [flowChatState.sessions]
  );

  const sessions = useMemo(() => {
    if (!searchQuery.trim()) return allSessions;
    const query = searchQuery.toLowerCase();
    return allSessions.filter(session => {
      if (session.title?.toLowerCase().includes(query)) return true;
      return session.dialogTurns.some(turn => {
        const userContent = turn.userMessage?.content?.toLowerCase() || '';
        if (userContent.includes(query)) return true;
        return turn.modelRounds.some(round =>
          round.items.some(item => item.type === 'text' && item.content.toLowerCase().includes(query))
        );
      });
    });
  }, [allSessions, searchQuery]);

  const { recentSessions, oldSessions } = useMemo(() => {
    const now = Date.now();
    const recent: Session[] = [];
    const old: Session[] = [];
    sessions.forEach(session => {
      if (now - session.lastActiveAt < ONE_HOUR_MS) { recent.push(session); } else { old.push(session); }
    });
    return { recentSessions: recent, oldSessions: old };
  }, [sessions]);

  const activeSessionId = flowChatState.activeSessionId;

  const handleSessionClick = useCallback(async (sessionId: string) => {
    if (sessionId !== activeSessionId) {
      try {
        await flowChatManager.switchChatSession(sessionId);
        window.dispatchEvent(new CustomEvent('flowchat:switch-session', { detail: { sessionId } }));
      } catch (error) {
        log.error('Failed to switch session', error);
      }
    }
  }, [activeSessionId]);

  const handleCreateSession = useCallback(async () => {
    try {
      await flowChatManager.createChatSession({ modelName: 'claude-sonnet-4.5', agentType: 'general-purpose' });
    } catch (error) {
      log.error('Failed to create session', error);
    }
  }, []);

  const handleStartEdit = useCallback((sessionId: string, currentTitle: string, e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    setEditingSessionId(sessionId);
    setEditingTitle(currentTitle || '');
    setTimeout(() => { editInputRef.current?.focus(); editInputRef.current?.select(); }, 0);
  }, []);

  const handleSaveEdit = useCallback(async (sessionId: string) => {
    const trimmedTitle = editingTitle.trim();
    if (!trimmedTitle) { setEditingSessionId(null); setEditingTitle(''); return; }
    try {
      await flowChatStore.updateSessionTitle(sessionId, trimmedTitle, 'generated');
    } catch (error) {
      log.error('Failed to update session title', error);
    } finally {
      setEditingSessionId(null);
      setEditingTitle('');
    }
  }, [editingTitle]);

  const handleCancelEdit = useCallback(() => { setEditingSessionId(null); setEditingTitle(''); }, []);

  const handleEditKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>, sessionId: string) => {
    if (e.key === 'Enter') { e.preventDefault(); handleSaveEdit(sessionId); }
    else if (e.key === 'Escape') { e.preventDefault(); handleCancelEdit(); }
  }, [handleSaveEdit, handleCancelEdit]);

  const handleEditBlur = useCallback((sessionId: string) => {
    setTimeout(() => { if (editingSessionId === sessionId) handleSaveEdit(sessionId); }, 150);
  }, [editingSessionId, handleSaveEdit]);

  const handleDeleteSession = useCallback((sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    if (sessions.length <= 1) { log.warn('Cannot delete last session'); return; }
    flowChatManager.deleteChatSession(sessionId).catch(error => log.error('Failed to delete session', error));
  }, [sessions.length]);

  const formatTime = useCallback((timestamp: number) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    if (diff < 60 * 1000) return t('time.justNow');
    if (diff < 60 * 60 * 1000) return t('time.minutesAgo', { count: Math.floor(diff / 60000) });
    if (diff < 24 * 60 * 60 * 1000) return t('time.hoursAgo', { count: Math.floor(diff / 3600000) });
    if (diff < 7 * 24 * 60 * 60 * 1000) return t('time.daysAgo', { count: Math.floor(diff / 86400000) });
    return date.toLocaleDateString(i18n.language, { month: 'short', day: 'numeric' });
  }, [t, i18n.language]);

  const getSessionPreview = useCallback((session: Session) => {
    const firstTurn = session.dialogTurns.find(turn => turn.userMessage?.content);
    if (firstTurn?.userMessage?.content) {
      const text = firstTurn.userMessage.content;
      return text.length > 50 ? text.substring(0, 50) + '...' : text;
    }
    return t('session.newConversation');
  }, [t]);

  const renderSessionItem = (session: Session, compact = false) => {
    const isActive = session.sessionId === activeSessionId;
    const isEditing = editingSessionId === session.sessionId;
    const isProcessing = processingSessionIds.has(session.sessionId);
    const displayTitle = session.title || t('session.defaultTitle', { id: session.sessionId.substring(0, 8) });

    return (
      <div
        key={session.sessionId}
        className={[
          'bitfun-sessions-panel__item',
          compact ? 'bitfun-sessions-panel__item--compact' : '',
          isActive ? 'bitfun-sessions-panel__item--active' : '',
          isProcessing ? 'bitfun-sessions-panel__item--processing' : '',
        ].join(' ')}
        onClick={() => !isEditing && handleSessionClick(session.sessionId)}
      >
        <div className="bitfun-sessions-panel__item-header">
          {isEditing ? (
            <div className="bitfun-sessions-panel__item-edit">
              <input
                ref={editInputRef}
                type="text"
                className="bitfun-sessions-panel__item-edit-input"
                value={editingTitle}
                onChange={e => setEditingTitle(e.target.value)}
                onKeyDown={e => handleEditKeyDown(e, session.sessionId)}
                onBlur={() => handleEditBlur(session.sessionId)}
                onClick={e => e.stopPropagation()}
                placeholder={t('input.titlePlaceholder')}
              />
              <IconButton variant="success" size="xs"
                onClick={e => { e.stopPropagation(); handleSaveEdit(session.sessionId); }}
                tooltip={t('actions.save')}
              >
                <Check size={14} />
              </IconButton>
            </div>
          ) : (
            <>
              <div className="bitfun-sessions-panel__item-title-wrapper">
                {isProcessing && (
                  <Tooltip content={t('status.processing')}>
                    <Loader2 size={14} className="bitfun-sessions-panel__item-processing-icon" />
                  </Tooltip>
                )}
                <Tooltip content={t('actions.doubleClickToEdit')}>
                  <div
                    className="bitfun-sessions-panel__item-title"
                    onDoubleClick={e => handleStartEdit(session.sessionId, displayTitle, e)}
                  >
                    {displayTitle}
                  </div>
                </Tooltip>
              </div>
              <div className="bitfun-sessions-panel__item-meta">
                <span className="bitfun-sessions-panel__item-time">{formatTime(session.lastActiveAt)}</span>
                <Tooltip content={t('actions.editTitle')}>
                  <button className="bitfun-sessions-panel__item-edit-btn"
                    onClick={e => handleStartEdit(session.sessionId, displayTitle, e)}>
                    <Pencil size={14} />
                  </button>
                </Tooltip>
                <Tooltip content={t('actions.deleteSession')}>
                  <button className="bitfun-sessions-panel__item-delete"
                    onClick={e => handleDeleteSession(session.sessionId, e)}>
                    <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                      <path d="M3 4H13M5 4V3C5 2.44772 5.44772 2 6 2H10C10.5523 2 11 2.44772 11 3V4M6.5 7.5V11.5M9.5 7.5V11.5M4 4H12V13C12 13.5523 11.5523 14 11 14H5C4.44772 14 4 13.5523 4 13V4Z"
                        stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                    </svg>
                  </button>
                </Tooltip>
              </div>
            </>
          )}
        </div>
        {!isEditing && !compact && (
          <div className="bitfun-sessions-panel__item-preview">{getSessionPreview(session)}</div>
        )}
      </div>
    );
  };

  return (
    <div className="bitfun-sessions-panel">
      <PanelHeader title={t('title')} />

      <div className="bitfun-sessions-panel__search">
        <Search
          placeholder={t('search.placeholder')}
          value={searchQuery}
          onChange={setSearchQuery}
          onClear={() => setSearchQuery('')}
          clearable
          size="small"
        />
      </div>

      <div className="bitfun-sessions-panel__create-section">
        <Button variant="secondary" size="small" onClick={handleCreateSession}
          className="bitfun-sessions-panel__create-button">
          <Plus size={16} />
          <span>{t('actions.createSession')}</span>
        </Button>
      </div>

      <div className="bitfun-sessions-panel__list">
        {sessions.length === 0 ? (
          <div className="bitfun-sessions-panel__empty">
            <div className="bitfun-sessions-panel__empty-icon">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                {searchQuery ? (
                  <>
                    <circle cx="11" cy="11" r="8" />
                    <path d="m21 21-4.35-4.35" />
                  </>
                ) : (
                  <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
                )}
              </svg>
            </div>
            <p className="bitfun-sessions-panel__empty-text">
              {searchQuery ? t('empty.noSearchResults', { query: searchQuery }) : t('empty.noSessions')}
            </p>
            {!searchQuery && (
              <button className="bitfun-sessions-panel__empty-btn" onClick={handleCreateSession}>
                {t('actions.createFirstSession')}
              </button>
            )}
          </div>
        ) : (
          <>
            {recentSessions.length > 0 && (
              <div className="bitfun-sessions-panel__group">
                <div className="bitfun-sessions-panel__group-header"
                  onClick={() => setIsRecentCollapsed(!isRecentCollapsed)}>
                  <span className="bitfun-sessions-panel__group-title">{t('groups.recent')}</span>
                  <span className="bitfun-sessions-panel__group-count">{recentSessions.length}</span>
                </div>
                {!isRecentCollapsed && (
                  <div className="bitfun-sessions-panel__group-content">
                    {recentSessions.map(s => renderSessionItem(s, false))}
                  </div>
                )}
              </div>
            )}
            {oldSessions.length > 0 && (
              <div className="bitfun-sessions-panel__group">
                <div className="bitfun-sessions-panel__group-header"
                  onClick={() => setIsOldCollapsed(!isOldCollapsed)}>
                  <span className="bitfun-sessions-panel__group-title">{t('groups.earlier')}</span>
                  <span className="bitfun-sessions-panel__group-count">{oldSessions.length}</span>
                </div>
                {!isOldCollapsed && (
                  <div className="bitfun-sessions-panel__group-content">
                    {oldSessions.map(s => renderSessionItem(s, true))}
                  </div>
                )}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
};

export default SessionsPanel;

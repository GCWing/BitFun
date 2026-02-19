/**
 * Session list panel component
 * Displays all chat sessions, supports switching and managing sessions
 */

import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Pencil, Check, Loader2, Puzzle, Bot, Plug, AlertTriangle, MessageSquareText, RefreshCw, Copy } from 'lucide-react';
import { flowChatStore } from '../../../flow_chat/store/FlowChatStore';
import { flowChatManager } from '../../../flow_chat/services/FlowChatManager';
import type { FlowChatState, Session } from '../../../flow_chat/types/flow-chat';
import { stateMachineManager } from '../../../flow_chat/state-machine/SessionStateMachineManager';
import { SessionExecutionState } from '../../../flow_chat/state-machine/types';
import { Search, Button, IconButton, Tooltip, Switch, Tabs, TabPane, Card, CardBody } from '@/component-library';
import { PanelHeader } from './base';
import { createLogger } from '@/shared/utils/logger';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { SubagentAPI, type SubagentInfo } from '@/infrastructure/api/service-api/SubagentAPI';
import { MCPAPI, type MCPServerInfo } from '@/infrastructure/api/service-api/MCPAPI';
import type { SkillInfo } from '@/infrastructure/config/types';
import { useNotification } from '@/shared/notification-system';
import './SessionsPanel.scss';

const log = createLogger('SessionsPanel');

const ONE_HOUR_MS = 60 * 60 * 1000;
const MCP_HEALTHY_STATUSES = new Set(['connected', 'healthy']);

type CapabilityPanelType = 'skills' | 'subagents' | 'mcp' | null;
type SessionPanelViewMode = 'sessions' | 'capabilities';

const SessionsPanel: React.FC = () => {
  const { t, i18n } = useTranslation('panels/sessions');
  const { error: notifyError, success: notifySuccess } = useNotification();
  
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
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [subagents, setSubagents] = useState<SubagentInfo[]>([]);
  const [mcpServers, setMcpServers] = useState<MCPServerInfo[]>([]);
  const [activeCapabilityPanel, setActiveCapabilityPanel] = useState<CapabilityPanelType>('skills');
  const [viewMode, setViewMode] = useState<SessionPanelViewMode>('sessions');
  const [isCapRefreshing, setIsCapRefreshing] = useState(false);
  const [expandedCapIds, setExpandedCapIds] = useState<Set<string>>(() => new Set());

  const toggleCapExpanded = useCallback((id: string) => {
    setExpandedCapIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) { next.delete(id); } else { next.add(id); }
      return next;
    });
  }, []);

  const [copiedPath, setCopiedPath] = useState<string | null>(null);

  const handleCopyPath = useCallback(async (path: string) => {
    try {
      await navigator.clipboard.writeText(path);
      setCopiedPath(path);
      notifySuccess(t('capabilities.pathCopied'));
      setTimeout(() => setCopiedPath(null), 2000);
    } catch {
      notifyError(t('capabilities.pathCopyFailed'));
    }
  }, [notifySuccess, notifyError, t]);

  useEffect(() => {
    const unsubscribe = flowChatStore.subscribe((state) => {
      setFlowChatState(state);
    });

    return () => {
      unsubscribe();
    };
  }, []);

  const loadCapabilities = useCallback(async (silent = false) => {
    try {
      const [skillList, subagentList, serverList] = await Promise.all([
        configAPI.getSkillConfigs(),
        SubagentAPI.listSubagents(),
        MCPAPI.getServers(),
      ]);
      setSkills(skillList);
      setSubagents(subagentList);
      setMcpServers(serverList);
    } catch (error) {
      log.error('Failed to load capabilities', error);
      if (!silent) {
        notifyError(t('capabilities.loadFailed'));
      }
    }
  }, [notifyError, t]);

  useEffect(() => {
    loadCapabilities();
  }, [loadCapabilities]);

  useEffect(() => {
    const unsubscribe = stateMachineManager.subscribeGlobal((sessionId, machine) => {
      const isProcessing = machine.currentState === SessionExecutionState.PROCESSING;
      
      setProcessingSessionIds(prev => {
        const next = new Set(prev);
        if (isProcessing) {
          next.add(sessionId);
        } else {
          next.delete(sessionId);
        }
        if (next.size !== prev.size || [...next].some(id => !prev.has(id))) {
          return next;
        }
        return prev;
      });
    });

    return () => {
      unsubscribe();
    };
  }, []);

  const allSessions = useMemo(() => 
    Array.from(flowChatState.sessions.values()).sort(
      (a: Session, b: Session) => b.createdAt - a.createdAt
    ),
    [flowChatState.sessions]
  );

  const sessions = useMemo(() => {
    if (!searchQuery.trim()) {
      return allSessions;
    }

    const query = searchQuery.toLowerCase();
    return allSessions.filter((session) => {
      if (session.title?.toLowerCase().includes(query)) {
        return true;
      }

      return session.dialogTurns.some((turn) => {
        const userContent = turn.userMessage?.content?.toLowerCase() || '';
        if (userContent.includes(query)) {
          return true;
        }

        return turn.modelRounds.some((round) => {
          return round.items.some((item) => {
            if (item.type === 'text') {
              return item.content.toLowerCase().includes(query);
            }
            return false;
          });
        });
      });
    });
  }, [allSessions, searchQuery]);

  const { recentSessions, oldSessions } = useMemo(() => {
    const now = Date.now();
    const recent: Session[] = [];
    const old: Session[] = [];
    
    sessions.forEach((session) => {
      if (now - session.lastActiveAt < ONE_HOUR_MS) {
        recent.push(session);
      } else {
        old.push(session);
      }
    });
    
    return { recentSessions: recent, oldSessions: old };
  }, [sessions]);

  const activeSessionId = flowChatState.activeSessionId;

  const enabledSkills = useMemo(() => skills.filter((skill) => skill.enabled), [skills]);
  const enabledSubagents = useMemo(() => subagents.filter((agent) => agent.enabled), [subagents]);
  const enabledMcpServers = useMemo(() => mcpServers.filter((server) => server.enabled), [mcpServers]);
  const unhealthyMcpServers = useMemo(
    () =>
      enabledMcpServers.filter((server) => !MCP_HEALTHY_STATUSES.has((server.status || '').toLowerCase())),
    [enabledMcpServers]
  );
  const hasMcpIssue = unhealthyMcpServers.length > 0;

  const handleSessionClick = useCallback(async (sessionId: string) => {
    if (sessionId !== activeSessionId) {
      try {
        await flowChatManager.switchChatSession(sessionId);
        
        const event = new CustomEvent('flowchat:switch-session', {
          detail: { sessionId }
        });
        window.dispatchEvent(event);
      } catch (error) {
        log.error('Failed to switch session', error);
      }
    }
  }, [activeSessionId]);

  const handleToggleSkill = useCallback(async (skill: SkillInfo) => {
    const nextEnabled = !skill.enabled;
    try {
      await configAPI.setSkillEnabled(skill.name, nextEnabled);
      await loadCapabilities(true);
    } catch (error) {
      notifyError(t('capabilities.toggleFailed'));
    }
  }, [loadCapabilities, notifyError, t]);

  const handleToggleSubagent = useCallback(async (subagent: SubagentInfo) => {
    const nextEnabled = !subagent.enabled;
    try {
      const isCustom = subagent.subagentSource === 'user' || subagent.subagentSource === 'project';
      if (isCustom) {
        await SubagentAPI.updateSubagentConfig({
          subagentId: subagent.id,
          enabled: nextEnabled,
        });
      } else {
        await configAPI.setSubagentConfig(subagent.id, nextEnabled);
      }
      await loadCapabilities(true);
    } catch (error) {
      notifyError(t('capabilities.toggleFailed'));
    }
  }, [loadCapabilities, notifyError, t]);

  const handleReconnectMcp = useCallback(async (server: MCPServerInfo) => {
    try {
      if ((server.status || '').toLowerCase() === 'stopped') {
        await MCPAPI.startServer(server.id);
      } else {
        await MCPAPI.restartServer(server.id);
      }
      await loadCapabilities(true);
      notifySuccess(t('capabilities.mcpReconnectSuccess', { name: server.name }));
    } catch (error) {
      notifyError(t('capabilities.mcpReconnectFailed', { name: server.name }));
    }
  }, [loadCapabilities, notifyError, notifySuccess, t]);

  const handleDeleteSession = useCallback((sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    
    if (sessions.length <= 1) {
      log.warn('Cannot delete last session');
      return;
    }

    flowChatManager.deleteChatSession(sessionId)
      .catch(error => {
        log.error('Failed to delete session', error);
      });
  }, [sessions.length]);

  const handleCreateSession = useCallback(async () => {
    try {
      await flowChatManager.createChatSession({
        modelName: 'claude-sonnet-4.5',
        agentType: 'general-purpose'
      });
    } catch (error) {
      log.error('Failed to create session', error);
    }
  }, []);

  const handleStartEdit = useCallback((sessionId: string, currentTitle: string, e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    setEditingSessionId(sessionId);
    setEditingTitle(currentTitle || '');
    setTimeout(() => {
      editInputRef.current?.focus();
      editInputRef.current?.select();
    }, 0);
  }, []);

  const handleSaveEdit = useCallback(async (sessionId: string) => {
    const trimmedTitle = editingTitle.trim();
    if (!trimmedTitle) {
      setEditingSessionId(null);
      setEditingTitle('');
      return;
    }

    try {
      await flowChatStore.updateSessionTitle(sessionId, trimmedTitle, 'generated');
      log.debug('Session title updated', { sessionId, title: trimmedTitle });
    } catch (error) {
      log.error('Failed to update session title', error);
    } finally {
      setEditingSessionId(null);
      setEditingTitle('');
    }
  }, [editingTitle]);

  const handleCancelEdit = useCallback(() => {
    setEditingSessionId(null);
    setEditingTitle('');
  }, []);

  const handleEditKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>, sessionId: string) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleSaveEdit(sessionId);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      handleCancelEdit();
    }
  }, [handleSaveEdit, handleCancelEdit]);

  const handleEditBlur = useCallback((sessionId: string) => {
    setTimeout(() => {
      if (editingSessionId === sessionId) {
        handleSaveEdit(sessionId);
      }
    }, 150);
  }, [editingSessionId, handleSaveEdit]);

  const formatTime = useCallback((timestamp: number) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    
    if (diff < 60 * 1000) {
      return t('time.justNow');
    }
    
    if (diff < 60 * 60 * 1000) {
      const minutes = Math.floor(diff / (60 * 1000));
      return t('time.minutesAgo', { count: minutes });
    }
    
    if (diff < 24 * 60 * 60 * 1000) {
      const hours = Math.floor(diff / (60 * 60 * 1000));
      return t('time.hoursAgo', { count: hours });
    }
    
    if (diff < 7 * 24 * 60 * 60 * 1000) {
      const days = Math.floor(diff / (24 * 60 * 60 * 1000));
      return t('time.daysAgo', { count: days });
    }
    
    return date.toLocaleDateString(i18n.language, { 
      month: 'short', 
      day: 'numeric' 
    });
  }, [t, i18n.language]);

  const getSessionPreview = useCallback((session: Session) => {
    const firstDialogTurn = session.dialogTurns.find(
      (turn) => turn.userMessage?.content
    );
    
    if (firstDialogTurn?.userMessage?.content) {
      const text = firstDialogTurn.userMessage.content;
      return text.length > 50 ? text.substring(0, 50) + '...' : text;
    }
    
    return t('session.newConversation');
  }, [t]);

  const handleCapabilityChipClick = useCallback((panel: CapabilityPanelType) => {
    setActiveCapabilityPanel(panel);
  }, []);

  const handleRefreshCapabilities = useCallback(async () => {
    setIsCapRefreshing(true);
    try {
      await loadCapabilities(true);
    } finally {
      setIsCapRefreshing(false);
    }
  }, [loadCapabilities]);

  const capIndex = activeCapabilityPanel === 'skills' ? 0 : activeCapabilityPanel === 'subagents' ? 1 : 2;

  return (
    <div className="bitfun-sessions-panel">
      <PanelHeader title={t('title')} />

      <Tabs
        activeKey={viewMode}
        onChange={(key) => setViewMode(key as SessionPanelViewMode)}
        type="line"
        size="small"
        className="bitfun-sessions-panel__tabs"
      >
        <TabPane
          tabKey="sessions"
          label={t('views.sessions')}
          icon={<MessageSquareText size={14} />}
        >
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
            <Button
              variant="secondary"
              size="small"
              onClick={handleCreateSession}
              className="bitfun-sessions-panel__create-button"
            >
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
                      <circle cx="11" cy="11" r="8" />
                    ) : (
                      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
                    )}
                    {searchQuery && <path d="m21 21-4.35-4.35" />}
                  </svg>
                </div>
                <p className="bitfun-sessions-panel__empty-text">
                  {searchQuery ? t('empty.noSearchResults', { query: searchQuery }) : t('empty.noSessions')}
                </p>
                {!searchQuery && (
                  <button
                    className="bitfun-sessions-panel__empty-btn"
                    onClick={handleCreateSession}
                  >
                    {t('actions.createFirstSession')}
                  </button>
                )}
              </div>
            ) : (
              <>
                {recentSessions.length > 0 && (
                  <div className="bitfun-sessions-panel__group">
                    <div
                      className="bitfun-sessions-panel__group-header"
                      onClick={() => setIsRecentCollapsed(!isRecentCollapsed)}
                    >
                      <span className="bitfun-sessions-panel__group-title">{t('groups.recent')}</span>
                      <span className="bitfun-sessions-panel__group-count">{recentSessions.length}</span>
                    </div>
                    {!isRecentCollapsed && (
                      <div className="bitfun-sessions-panel__group-content">
                        {recentSessions.map((session: Session) => {
                          const isActive = session.sessionId === activeSessionId;
                          const preview = getSessionPreview(session);
                          const isEditing = editingSessionId === session.sessionId;
                          const isProcessing = processingSessionIds.has(session.sessionId);
                          const displayTitle = session.title || t('session.defaultTitle', { id: session.sessionId.substring(0, 8) });

                          return (
                            <div
                              key={session.sessionId}
                              className={`bitfun-sessions-panel__item ${isActive ? 'bitfun-sessions-panel__item--active' : ''} ${isProcessing ? 'bitfun-sessions-panel__item--processing' : ''}`}
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
                                      onChange={(e) => setEditingTitle(e.target.value)}
                                      onKeyDown={(e) => handleEditKeyDown(e, session.sessionId)}
                                      onBlur={() => handleEditBlur(session.sessionId)}
                                      onClick={(e) => e.stopPropagation()}
                                      placeholder={t('input.titlePlaceholder')}
                                    />
                                    <IconButton
                                      variant="success"
                                      size="xs"
                                      onClick={(e) => {
                                        e.stopPropagation();
                                        handleSaveEdit(session.sessionId);
                                      }}
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
                                          onDoubleClick={(e) => handleStartEdit(session.sessionId, displayTitle, e)}
                                        >
                                          {displayTitle}
                                        </div>
                                      </Tooltip>
                                    </div>
                                    <div className="bitfun-sessions-panel__item-meta">
                                      <span className="bitfun-sessions-panel__item-time">
                                        {formatTime(session.lastActiveAt)}
                                      </span>
                                      <Tooltip content={t('actions.editTitle')}>
                                        <button
                                          className="bitfun-sessions-panel__item-edit-btn"
                                          onClick={(e) => handleStartEdit(session.sessionId, displayTitle, e)}
                                        >
                                          <Pencil size={14} />
                                        </button>
                                      </Tooltip>
                                      <Tooltip content={t('actions.deleteSession')}>
                                        <button
                                          className="bitfun-sessions-panel__item-delete"
                                          onClick={(e) => handleDeleteSession(session.sessionId, e)}
                                        >
                                          <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                                            <path
                                              d="M3 4H13M5 4V3C5 2.44772 5.44772 2 6 2H10C10.5523 2 11 2.44772 11 3V4M6.5 7.5V11.5M9.5 7.5V11.5M4 4H12V13C12 13.5523 11.5523 14 11 14H5C4.44772 14 4 13.5523 4 13V4Z"
                                              stroke="currentColor"
                                              strokeWidth="1.5"
                                              strokeLinecap="round"
                                            />
                                          </svg>
                                        </button>
                                      </Tooltip>
                                    </div>
                                  </>
                                )}
                              </div>
                              {!isEditing && (
                                <div className="bitfun-sessions-panel__item-preview">
                                  {preview}
                                </div>
                              )}
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                )}

                {oldSessions.length > 0 && (
                  <div className="bitfun-sessions-panel__group">
                    <div
                      className="bitfun-sessions-panel__group-header"
                      onClick={() => setIsOldCollapsed(!isOldCollapsed)}
                    >
                      <span className="bitfun-sessions-panel__group-title">{t('groups.earlier')}</span>
                      <span className="bitfun-sessions-panel__group-count">{oldSessions.length}</span>
                    </div>
                    {!isOldCollapsed && (
                      <div className="bitfun-sessions-panel__group-content">
                        {oldSessions.map((session: Session) => {
                          const isActive = session.sessionId === activeSessionId;
                          const isEditing = editingSessionId === session.sessionId;
                          const isProcessing = processingSessionIds.has(session.sessionId);
                          const displayTitle = session.title || t('session.defaultTitle', { id: session.sessionId.substring(0, 8) });

                          return (
                            <div
                              key={session.sessionId}
                              className={`bitfun-sessions-panel__item bitfun-sessions-panel__item--compact ${isActive ? 'bitfun-sessions-panel__item--active' : ''} ${isProcessing ? 'bitfun-sessions-panel__item--processing' : ''}`}
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
                                      onChange={(e) => setEditingTitle(e.target.value)}
                                      onKeyDown={(e) => handleEditKeyDown(e, session.sessionId)}
                                      onBlur={() => handleEditBlur(session.sessionId)}
                                      onClick={(e) => e.stopPropagation()}
                                      placeholder={t('input.titlePlaceholder')}
                                    />
                                    <IconButton
                                      variant="success"
                                      size="xs"
                                      onClick={(e) => {
                                        e.stopPropagation();
                                        handleSaveEdit(session.sessionId);
                                      }}
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
                                          onDoubleClick={(e) => handleStartEdit(session.sessionId, displayTitle, e)}
                                        >
                                          {displayTitle}
                                        </div>
                                      </Tooltip>
                                    </div>
                                    <div className="bitfun-sessions-panel__item-meta">
                                      <span className="bitfun-sessions-panel__item-time">
                                        {formatTime(session.lastActiveAt)}
                                      </span>
                                      <Tooltip content={t('actions.editTitle')}>
                                        <button
                                          className="bitfun-sessions-panel__item-edit-btn"
                                          onClick={(e) => handleStartEdit(session.sessionId, displayTitle, e)}
                                        >
                                          <Pencil size={14} />
                                        </button>
                                      </Tooltip>
                                      <Tooltip content={t('actions.deleteSession')}>
                                        <button
                                          className="bitfun-sessions-panel__item-delete"
                                          onClick={(e) => handleDeleteSession(session.sessionId, e)}
                                        >
                                          <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                                            <path
                                              d="M3 4H13M5 4V3C5 2.44772 5.44772 2 6 2H10C10.5523 2 11 2.44772 11 3V4M6.5 7.5V11.5M9.5 7.5V11.5M4 4H12V13C12 13.5523 11.5523 14 11 14H5C4.44772 14 4 13.5523 4 13V4Z"
                                              stroke="currentColor"
                                              strokeWidth="1.5"
                                              strokeLinecap="round"
                                            />
                                          </svg>
                                        </button>
                                      </Tooltip>
                                    </div>
                                  </>
                                )}
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                )}
              </>
            )}
          </div>
        </TabPane>

        <TabPane
          tabKey="capabilities"
          label={t('views.capabilities')}
          icon={<Puzzle size={14} />}
        >
          <div className="bitfun-sessions-panel__capabilities-view">
            <div className="bitfun-sessions-panel__cap-segments">
              <div
                className="bitfun-sessions-panel__cap-segments-track"
                style={{ '--cap-index': capIndex } as React.CSSProperties}
              >
                <div className="bitfun-sessions-panel__cap-segments-slider" />
                <button
                  className={`bitfun-sessions-panel__cap-seg ${activeCapabilityPanel === 'skills' ? 'is-active' : ''}`}
                  onClick={() => handleCapabilityChipClick('skills')}
                >
                  <Puzzle size={12} />
                  <span className="bitfun-sessions-panel__cap-seg-label">{t('capabilities.skills')}</span>
                  <span className="bitfun-sessions-panel__cap-seg-count">{enabledSkills.length}/{skills.length}</span>
                </button>
                <button
                  className={`bitfun-sessions-panel__cap-seg ${activeCapabilityPanel === 'subagents' ? 'is-active' : ''}`}
                  onClick={() => handleCapabilityChipClick('subagents')}
                >
                  <Bot size={12} />
                  <span className="bitfun-sessions-panel__cap-seg-label">{t('capabilities.subagents')}</span>
                  <span className="bitfun-sessions-panel__cap-seg-count">{enabledSubagents.length}/{subagents.length}</span>
                </button>
                <button
                  className={`bitfun-sessions-panel__cap-seg ${activeCapabilityPanel === 'mcp' ? 'is-active' : ''} ${hasMcpIssue ? 'is-warning' : ''}`}
                  onClick={() => handleCapabilityChipClick('mcp')}
                >
                  <Plug size={12} />
                  <span className="bitfun-sessions-panel__cap-seg-label">{t('capabilities.mcp')}</span>
                  <span className="bitfun-sessions-panel__cap-seg-count">{enabledMcpServers.length}/{mcpServers.length}</span>
                  {hasMcpIssue && <span className="bitfun-sessions-panel__cap-seg-warn" />}
                </button>
              </div>
            </div>

            {hasMcpIssue && activeCapabilityPanel === 'mcp' && (
              <div className="bitfun-sessions-panel__cap-alert">
                <AlertTriangle size={12} />
                <span>{t('capabilities.mcpWarning', { count: unhealthyMcpServers.length })}</span>
              </div>
            )}

            <div className="bitfun-sessions-panel__cap-content">
              {activeCapabilityPanel === 'skills' && (
                skills.length === 0 ? (
                  <div className="bitfun-sessions-panel__cap-empty">
                    <span>{t('capabilities.emptySkills')}</span>
                  </div>
                ) : (
                  <div className="bitfun-sessions-panel__cap-cards-grid">
                    {skills.map((skill) => {
                      const isExpanded = expandedCapIds.has(`skill:${skill.name}`);
                      return (
                        <Card
                          key={skill.name}
                          variant="default"
                          padding="none"
                          className={`bitfun-sessions-panel__cap-card ${!skill.enabled ? 'is-disabled' : ''} ${isExpanded ? 'is-expanded' : ''}`}
                        >
                          <div
                            className="bitfun-sessions-panel__cap-card-header"
                            onClick={() => toggleCapExpanded(`skill:${skill.name}`)}
                          >
                            <div className="bitfun-sessions-panel__cap-card-icon bitfun-sessions-panel__cap-card-icon--skill">
                              <Puzzle size={13} />
                            </div>
                            <div className="bitfun-sessions-panel__cap-card-info">
                              <span className="bitfun-sessions-panel__cap-card-name">{skill.name}</span>
                              <span className="bitfun-sessions-panel__cap-badge bitfun-sessions-panel__cap-badge--purple">{skill.level}</span>
                            </div>
                            <div className="bitfun-sessions-panel__cap-card-actions" onClick={(e) => e.stopPropagation()}>
                              <Switch checked={skill.enabled} onChange={() => handleToggleSkill(skill)} size="small" />
                            </div>
                          </div>
                          {isExpanded && (
                            <CardBody className="bitfun-sessions-panel__cap-card-details">
                              {skill.description && (
                                <div className="bitfun-sessions-panel__cap-card-desc">{skill.description}</div>
                              )}
                              <button
                                className="bitfun-sessions-panel__cap-card-path"
                                onClick={() => handleCopyPath(skill.path)}
                                title={t('capabilities.clickToCopy')}
                              >
                                <span className="bitfun-sessions-panel__cap-card-path-label">{t('capabilities.path')}</span>
                                <span className="bitfun-sessions-panel__cap-card-path-value">{skill.path}</span>
                                <span className="bitfun-sessions-panel__cap-card-path-copy">
                                  {copiedPath === skill.path ? <Check size={11} /> : <Copy size={11} />}
                                </span>
                              </button>
                            </CardBody>
                          )}
                        </Card>
                      );
                    })}
                  </div>
                )
              )}

              {activeCapabilityPanel === 'subagents' && (
                subagents.length === 0 ? (
                  <div className="bitfun-sessions-panel__cap-empty">
                    <span>{t('capabilities.emptySubagents')}</span>
                  </div>
                ) : (
                  <div className="bitfun-sessions-panel__cap-cards-grid">
                    {subagents.map((agent) => {
                      const isExpanded = expandedCapIds.has(`agent:${agent.id}`);
                      return (
                        <Card
                          key={agent.id}
                          variant="default"
                          padding="none"
                          className={`bitfun-sessions-panel__cap-card ${!agent.enabled ? 'is-disabled' : ''} ${isExpanded ? 'is-expanded' : ''}`}
                        >
                          <div
                            className="bitfun-sessions-panel__cap-card-header"
                            onClick={() => toggleCapExpanded(`agent:${agent.id}`)}
                          >
                            <div className="bitfun-sessions-panel__cap-card-icon bitfun-sessions-panel__cap-card-icon--agent">
                              <Bot size={13} />
                            </div>
                            <div className="bitfun-sessions-panel__cap-card-info">
                              <span className="bitfun-sessions-panel__cap-card-name">{agent.name}</span>
                              {agent.model && <span className="bitfun-sessions-panel__cap-badge bitfun-sessions-panel__cap-badge--blue">{agent.model}</span>}
                              {agent.subagentSource && <span className="bitfun-sessions-panel__cap-badge bitfun-sessions-panel__cap-badge--gray">{agent.subagentSource}</span>}
                            </div>
                            <div className="bitfun-sessions-panel__cap-card-actions" onClick={(e) => e.stopPropagation()}>
                              <Switch checked={agent.enabled} onChange={() => handleToggleSubagent(agent)} size="small" />
                            </div>
                          </div>
                          {isExpanded && (
                            <CardBody className="bitfun-sessions-panel__cap-card-details">
                              {agent.description && (
                                <div className="bitfun-sessions-panel__cap-card-desc">{agent.description}</div>
                              )}
                              <div className="bitfun-sessions-panel__cap-card-meta-row">
                                <span className="bitfun-sessions-panel__cap-card-path-label">{t('capabilities.toolCount')}</span>
                                <span className="bitfun-sessions-panel__cap-card-path-value">{agent.toolCount}</span>
                              </div>
                            </CardBody>
                          )}
                        </Card>
                      );
                    })}
                  </div>
                )
              )}

              {activeCapabilityPanel === 'mcp' && (
                mcpServers.length === 0 ? (
                  <div className="bitfun-sessions-panel__cap-empty">
                    <span>{t('capabilities.emptyMcp')}</span>
                  </div>
                ) : (
                  <div className="bitfun-sessions-panel__cap-cards-grid">
                    {mcpServers.map((server) => {
                      const healthy = MCP_HEALTHY_STATUSES.has((server.status || '').toLowerCase());
                      const isExpanded = expandedCapIds.has(`mcp:${server.id}`);
                      return (
                        <Card
                          key={server.id}
                          variant="default"
                          padding="none"
                          className={`bitfun-sessions-panel__cap-card ${isExpanded ? 'is-expanded' : ''} ${!healthy ? 'is-unhealthy' : ''}`}
                        >
                          <div
                            className="bitfun-sessions-panel__cap-card-header"
                            onClick={() => toggleCapExpanded(`mcp:${server.id}`)}
                          >
                            <div className={`bitfun-sessions-panel__cap-card-icon bitfun-sessions-panel__cap-card-icon--mcp ${!healthy ? 'is-error' : ''}`}>
                              <Plug size={13} />
                            </div>
                            <div className="bitfun-sessions-panel__cap-card-info">
                              <span className="bitfun-sessions-panel__cap-card-name">{server.name}</span>
                              <span className={`bitfun-sessions-panel__cap-badge bitfun-sessions-panel__cap-badge--${healthy ? 'green' : 'yellow'}`}>{server.status}</span>
                            </div>
                            {!healthy && (
                              <div className="bitfun-sessions-panel__cap-card-actions" onClick={(e) => e.stopPropagation()}>
                                <button
                                  className="bitfun-sessions-panel__cap-row-reconnect"
                                  onClick={() => handleReconnectMcp(server)}
                                  title={t('capabilities.reconnect')}
                                >
                                  <RefreshCw size={11} />
                                </button>
                              </div>
                            )}
                          </div>
                          {isExpanded && (
                            <CardBody className="bitfun-sessions-panel__cap-card-details">
                              <div className="bitfun-sessions-panel__cap-card-meta-row">
                                <span className="bitfun-sessions-panel__cap-card-path-label">{t('capabilities.serverType')}</span>
                                <span className="bitfun-sessions-panel__cap-card-path-value">{server.serverType}</span>
                              </div>
                            </CardBody>
                          )}
                        </Card>
                      );
                    })}
                  </div>
                )
              )}
            </div>

            <div className="bitfun-sessions-panel__cap-footer">
              <button
                className={`bitfun-sessions-panel__cap-refresh ${isCapRefreshing ? 'is-spinning' : ''}`}
                onClick={handleRefreshCapabilities}
                title={t('capabilities.refresh')}
              >
                <RefreshCw size={11} />
                <span>{t('capabilities.refresh')}</span>
              </button>
            </div>
          </div>
        </TabPane>
      </Tabs>
    </div>
  );
};

export default SessionsPanel;


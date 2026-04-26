/**
 * Toolbar Mode component.
 * Single-window morph UI for compact toolbar view.
 *
 * Layout: two rows
 * - Row 1: + / session list only when expanded; collapsed: no left control. Right: ⋮ when expanded, expand when collapsed.
 * - Row 2: streaming content/input + controls
 */

import React, { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { 
  MessageSquare, 
  Square, 
  Check, 
  X, 
  Send,
  Maximize2,
  MoreVertical,
  PanelTopOpen,
  PanelTopClose,
  Plus
} from 'lucide-react';
import { useToolbarModeContext } from './ToolbarModeContext';
import { flowChatStore } from '../../store/FlowChatStore';
import { syncSessionToModernStore } from '../../services/storeSync';
import { FlowChatState } from '../../types/flow-chat';
import { compareSessionsForDisplay } from '../../utils/sessionOrdering';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';
import { resolveSessionTitle } from '../../utils/sessionTitle';

const log = createLogger('ToolbarMode');
import { ModernFlowChatContainer } from '../modern/ModernFlowChatContainer';
import { Tooltip } from '@/component-library';
import { useImeEnterGuard } from '../../hooks/useImeEnterGuard';
import './ToolbarMode.scss';

export const ToolbarMode: React.FC = () => {
  const { t } = useTranslation('flow-chat');
  const { 
    isToolbarMode,
    isExpanded,
    disableToolbarMode,
    toggleExpanded,
    toolbarState
  } = useToolbarModeContext();
  
  const [showInput, setShowInput] = useState(false);
  const [inputValue, setInputValue] = useState('');
  const { isImeEnter, handleCompositionStart, handleCompositionEnd } = useImeEnterGuard();
  const [showSessionPicker, setShowSessionPicker] = useState(false);
  const [showHeaderOverflowMenu, setShowHeaderOverflowMenu] = useState(false);
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => 
    flowChatStore.getState()
  );
  const sessionPickerRef = useRef<HTMLDivElement>(null);
  const headerOverflowRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const unsubscribe = flowChatStore.subscribe((state) => {
      setFlowChatState(state);
    });
    return () => unsubscribe();
  }, []);
  
  const sessionTitle = useMemo(() => {
    const activeSession = flowChatState.activeSessionId 
      ? flowChatState.sessions.get(flowChatState.activeSessionId)
      : undefined;
    return resolveSessionTitle(activeSession, (key, options) => i18nService.t(key, options));
  }, [flowChatState]);
  
  const sessions = useMemo(() => {
    return Array.from(flowChatState.sessions.values())
      .sort(compareSessionsForDisplay)
      .slice(0, 10); // Limit to 10.
  }, [flowChatState]);
  
  const lastMessageContent = useMemo(() => {
    const activeSession = flowChatState.activeSessionId 
      ? flowChatState.sessions.get(flowChatState.activeSessionId)
      : undefined;
    
    if (!activeSession || !activeSession.dialogTurns || activeSession.dialogTurns.length === 0) {
      return null;
    }
    
    const lastTurn = activeSession.dialogTurns[activeSession.dialogTurns.length - 1];
    
    // Prefer the last text item in the latest model round.
    if (lastTurn.modelRounds && lastTurn.modelRounds.length > 0) {
      const lastRound = lastTurn.modelRounds[lastTurn.modelRounds.length - 1];
      for (let i = lastRound.items.length - 1; i >= 0; i--) {
        const item = lastRound.items[i];
        if (item.type === 'text' && 'content' in item) {
          const content = (item as any).content as string;
          const lines = content.trim().split('\n');
          return lines[lines.length - 1].trim() || lines[lines.length - 2]?.trim() || content.slice(-100);
        }
      }
    }
    
    // Fallback to the user's latest message.
    return lastTurn.userMessage?.content?.slice(0, 100) || null;
  }, [flowChatState]);
  
  // Derive current streaming state from session data.
  const currentStreamState = useMemo(() => {
    const activeSession = flowChatState.activeSessionId 
      ? flowChatState.sessions.get(flowChatState.activeSessionId)
      : undefined;
    
    if (!activeSession || !activeSession.dialogTurns || activeSession.dialogTurns.length === 0) {
      return { isStreaming: false, toolName: null, content: null };
    }
    
    const lastTurn = activeSession.dialogTurns[activeSession.dialogTurns.length - 1];
    
    const isStreaming =
      lastTurn.status === 'processing' ||
      lastTurn.status === 'finishing' ||
      lastTurn.status === 'image_analyzing';
    
    if (!isStreaming || !lastTurn.modelRounds || lastTurn.modelRounds.length === 0) {
      return { isStreaming, toolName: null, content: null };
    }
    
    const lastRound = lastTurn.modelRounds[lastTurn.modelRounds.length - 1];
    
    let toolName: string | null = null;
    let content: string | null = null;
    
    for (let i = lastRound.items.length - 1; i >= 0; i--) {
      const item = lastRound.items[i];
      
      if (item.type === 'tool' && 'toolName' in item) {
        toolName = (item as any).toolName;
        if ('input' in item && typeof (item as any).input === 'object') {
          const input = (item as any).input;
          content = input.path || input.command || input.query || input.content?.slice(0, 50) || t('toolCards.toolbar.executing');
        } else {
          content = t('toolCards.toolbar.executing');
        }
        break;
      }
      
      if (item.type === 'text' && 'content' in item && !toolName) {
        const textContent = (item as any).content as string;
        const lines = textContent.trim().split('\n');
        content = lines[lines.length - 1].trim() || lines[lines.length - 2]?.trim() || textContent.slice(-100);
      }
    }
    
    return { isStreaming, toolName, content };
  }, [flowChatState, t]);
  
  useEffect(() => {
    if (!isExpanded) {
      setShowSessionPicker(false);
      setShowHeaderOverflowMenu(false);
    }
  }, [isExpanded]);
  
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (sessionPickerRef.current?.contains(target)) {
        return;
      }
      if (target.closest?.('.bitfun-toolbar-mode__session-menu-trigger')) {
        return;
      }
      setShowSessionPicker(false);
    };
    
    if (showSessionPicker) {
      const timer = setTimeout(() => {
        document.addEventListener('mousedown', handleClickOutside);
      }, 0);
      return () => {
        clearTimeout(timer);
        document.removeEventListener('mousedown', handleClickOutside);
      };
    }
  }, [showSessionPicker]);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (headerOverflowRef.current?.contains(target)) {
        return;
      }
      if (target.closest?.('.bitfun-toolbar-mode__overflow-trigger')) {
        return;
      }
      setShowHeaderOverflowMenu(false);
    };

    if (showHeaderOverflowMenu) {
      const timer = setTimeout(() => {
        document.addEventListener('mousedown', handleClickOutside);
      }, 0);
      return () => {
        clearTimeout(timer);
        document.removeEventListener('mousedown', handleClickOutside);
      };
    }
  }, [showHeaderOverflowMenu]);
  
  const handleStartDrag = useCallback(async (e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    // Avoid dragging when interacting with UI controls.
    if (target.closest?.(
      'button, input, .bitfun-toolbar-mode__session-picker, .bitfun-toolbar-mode__session-dropdown, .bitfun-toolbar-mode__overflow-menu, .bitfun-toolbar-mode__stream-content, .bitfun-toolbar-mode__session-item, .bitfun-toolbar-mode__flowchat-container'
    )) {
      return;
    }
    try {
      const win = getCurrentWindow();
      await win.startDragging();
    } catch (error) {
      log.error('Failed to start dragging', error);
    }
  }, []);
  
  const handleExpand = useCallback(async () => {
    await disableToolbarMode();
  }, [disableToolbarMode]);
  
  const handleSwitchSession = useCallback((e: React.MouseEvent, sessionId: string) => {
    e.stopPropagation();
    e.preventDefault();
    flowChatStore.switchSession(sessionId);
    syncSessionToModernStore(sessionId);
    setShowSessionPicker(false);
  }, []);
  
  const handleCancel = useCallback(() => {
    window.dispatchEvent(new CustomEvent('toolbar-cancel-task'));
  }, []);
  
  const handleConfirm = useCallback(() => {
    if (toolbarState.pendingToolId) {
      window.dispatchEvent(new CustomEvent('toolbar-tool-confirm', { 
        detail: { toolId: toolbarState.pendingToolId } 
      }));
    }
  }, [toolbarState.pendingToolId]);
  
  const handleReject = useCallback(() => {
    if (toolbarState.pendingToolId) {
      window.dispatchEvent(new CustomEvent('toolbar-tool-reject', { 
        detail: { toolId: toolbarState.pendingToolId } 
      }));
    }
  }, [toolbarState.pendingToolId]);
  
  const dispatchToolbarCreateSession = useCallback((mode: 'code' | 'cowork') => {
    window.dispatchEvent(new CustomEvent('toolbar-create-session', { detail: { mode } }));
    setShowSessionPicker(false);
  }, []);

  const toggleSessionMenu = useCallback(() => {
    setShowHeaderOverflowMenu(false);
    setShowSessionPicker(v => !v);
  }, []);

  const toggleHeaderOverflowMenu = useCallback(() => {
    if (!isExpanded) return;
    setShowSessionPicker(false);
    setShowHeaderOverflowMenu(v => !v);
  }, [isExpanded]);
  
  const handleSendMessage = useCallback(() => {
    const message = inputValue.trim();
    if (message) {
      window.dispatchEvent(new CustomEvent('toolbar-send-message', { 
        detail: { message, sessionId: flowChatState.activeSessionId } 
      }));
      setInputValue('');
      setShowInput(false);
    }
  }, [inputValue, flowChatState.activeSessionId]);
  
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      if (isImeEnter(e)) return;
      e.preventDefault();
      handleSendMessage();
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      if (showInput) {
        setShowInput(false);
      } else if (showHeaderOverflowMenu) {
        setShowHeaderOverflowMenu(false);
      } else if (showSessionPicker) {
        setShowSessionPicker(false);
      } else {
        handleExpand();
      }
    }
  }, [handleSendMessage, showInput, showSessionPicker, showHeaderOverflowMenu, handleExpand, isImeEnter]);

  const sessionMenuContent = useMemo(
    () => (
      <div className="bitfun-toolbar-mode__session-menu">
        <div className="bitfun-toolbar-mode__session-menu-actions">
          <button
            type="button"
            className="bitfun-toolbar-mode__session-item bitfun-toolbar-mode__session-item--new"
            onMouseDown={(e) => {
              e.preventDefault();
              e.stopPropagation();
              dispatchToolbarCreateSession('code');
            }}
          >
            <span className="bitfun-toolbar-mode__session-item-icon" aria-hidden>
              <Plus size={13} strokeWidth={2.25} />
            </span>
            <span className="bitfun-toolbar-mode__session-item-label">
              {t('toolCards.toolbar.newCodeSessionItem')}
            </span>
          </button>
          <button
            type="button"
            className="bitfun-toolbar-mode__session-item bitfun-toolbar-mode__session-item--new"
            onMouseDown={(e) => {
              e.preventDefault();
              e.stopPropagation();
              dispatchToolbarCreateSession('cowork');
            }}
          >
            <span className="bitfun-toolbar-mode__session-item-icon" aria-hidden>
              <Plus size={13} strokeWidth={2.25} />
            </span>
            <span className="bitfun-toolbar-mode__session-item-label">
              {t('toolCards.toolbar.newCoworkSessionItem')}
            </span>
          </button>
          <div className="bitfun-toolbar-mode__session-list-divider" role="separator" />
        </div>
        <div
          className="bitfun-toolbar-mode__session-menu-scroll"
          role="listbox"
          aria-label={t('session.switchSession')}
        >
          {sessions.map((session) => (
            <button
              key={session.sessionId}
              type="button"
              className={`bitfun-toolbar-mode__session-item ${
                session.sessionId === flowChatState.activeSessionId ? 'bitfun-toolbar-mode__session-item--active' : ''
              }`}
              onMouseDown={(e) => handleSwitchSession(e, session.sessionId)}
            >
              {resolveSessionTitle(session, (key, options) => i18nService.t(key, options))}
            </button>
          ))}
        </div>
      </div>
    ),
    [sessions, flowChatState.activeSessionId, dispatchToolbarCreateSession, handleSwitchSession, t]
  );
  
  if (!isToolbarMode) {
    return null;
  }
  
  const containerClassName = [
    'bitfun-toolbar-mode',
    isExpanded && 'bitfun-toolbar-mode--expanded',
    currentStreamState.isStreaming && 'bitfun-toolbar-mode--processing',
    toolbarState.hasError && 'bitfun-toolbar-mode--error',
    toolbarState.hasPendingConfirmation && 'bitfun-toolbar-mode--confirm'
  ].filter(Boolean).join(' ');
  
  return (
    <div className={containerClassName} onMouseDown={handleStartDrag}>
      <div className="bitfun-toolbar-mode__header">
        <div className="bitfun-toolbar-mode__header-left">
          {isExpanded ? (
            <div className="bitfun-toolbar-mode__session-menu-root">
              <Tooltip content={t('toolCards.toolbar.openSessionMenu')}>
                <button
                  type="button"
                  className={[
                    'bitfun-toolbar-mode__create-btn',
                    'bitfun-toolbar-mode__session-menu-trigger',
                    showSessionPicker ? 'bitfun-toolbar-mode__session-menu-trigger--open' : '',
                  ].filter(Boolean).join(' ')}
                  onClick={toggleSessionMenu}
                  aria-expanded={showSessionPicker}
                  aria-haspopup="listbox"
                >
                  <Plus size={14} />
                </button>
              </Tooltip>
              {showSessionPicker && (
                <div 
                  className="bitfun-toolbar-mode__session-dropdown" 
                  ref={sessionPickerRef}
                  onMouseDown={(e) => e.stopPropagation()}
                >
                  {sessionMenuContent}
                </div>
              )}
            </div>
          ) : null}
        </div>

        <div className="bitfun-toolbar-mode__title-wrapper">
          <div className="bitfun-toolbar-mode__title-display" title={sessionTitle}>
            <span className="bitfun-toolbar-mode__title-text">{sessionTitle}</span>
          </div>
        </div>

        <div className="bitfun-toolbar-mode__header-right">
          <div className="bitfun-toolbar-mode__header-drag-area" aria-hidden="true" />
          <div className="bitfun-toolbar-mode__header-overflow">
            {isExpanded ? (
              <>
                <Tooltip content={t('toolCards.toolbar.moreMenu')}>
                  <button
                    type="button"
                    className="toolbar-btn toolbar-btn--overflow bitfun-toolbar-mode__overflow-trigger"
                    onClick={toggleHeaderOverflowMenu}
                    aria-expanded={showHeaderOverflowMenu}
                    aria-haspopup="menu"
                  >
                    <MoreVertical size={14} />
                  </button>
                </Tooltip>
                {showHeaderOverflowMenu && (
                  <div
                    ref={headerOverflowRef}
                    className="bitfun-toolbar-mode__overflow-menu"
                    role="menu"
                    onMouseDown={(e) => e.stopPropagation()}
                  >
                    <button
                      type="button"
                      className="bitfun-toolbar-mode__overflow-menu-item"
                      role="menuitem"
                      onClick={() => {
                        void toggleExpanded();
                        setShowHeaderOverflowMenu(false);
                      }}
                    >
                      <PanelTopClose size={14} />
                      <span>{t('toolCards.toolbar.collapseChat')}</span>
                    </button>
                    <button
                      type="button"
                      className="bitfun-toolbar-mode__overflow-menu-item"
                      role="menuitem"
                      onClick={() => {
                        void handleExpand();
                        setShowHeaderOverflowMenu(false);
                      }}
                    >
                      <Maximize2 size={14} />
                      <span>{t('session.restoreMain')}</span>
                    </button>
                  </div>
                )}
              </>
            ) : (
              <div className="bitfun-toolbar-mode__header-collapsed-actions">
                <Tooltip content={t('toolCards.toolbar.expandChat')}>
                  <button
                    type="button"
                    className="toolbar-btn toolbar-btn--overflow"
                    onClick={() => void toggleExpanded()}
                    aria-label={t('toolCards.toolbar.expandChat')}
                  >
                    <PanelTopOpen size={14} />
                  </button>
                </Tooltip>
                <Tooltip content={t('session.restoreMain')}>
                  <button
                    type="button"
                    className="toolbar-btn toolbar-btn--expand"
                    onClick={() => void handleExpand()}
                    aria-label={t('session.restoreMain')}
                  >
                    <Maximize2 size={14} />
                  </button>
                </Tooltip>
              </div>
            )}
          </div>
        </div>
      </div>
      
      {isExpanded ? (
        <>
          <div className="bitfun-toolbar-mode__flowchat-container">
            <ModernFlowChatContainer />
          </div>
          <div className="bitfun-toolbar-mode__expanded-input">
            <input
              type="text"
              className="bitfun-toolbar-mode__input-field bitfun-toolbar-mode__input-field--expanded"
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              onKeyDown={handleKeyDown}
              onCompositionStart={handleCompositionStart}
              onCompositionEnd={handleCompositionEnd}
              placeholder={currentStreamState.isStreaming ? t('toolCards.toolbar.aiProcessing') : t('toolCards.toolbar.inputMessage')}
              disabled={currentStreamState.isStreaming}
            />
            {currentStreamState.isStreaming ? (
              <Tooltip content={t('input.stop')}>
                <button 
                  className="toolbar-btn toolbar-btn--cancel"
                  onClick={handleCancel}
                >
                  <Square size={14} />
                </button>
              </Tooltip>
            ) : (
              <Tooltip content={t('input.send')}>
                <button 
                  className="toolbar-btn toolbar-btn--send"
                  onClick={handleSendMessage}
                  disabled={!inputValue.trim()}
                >
                  <Send size={16} />
                </button>
              </Tooltip>
            )}
          </div>
        </>
      ) : (
        <div className="bitfun-toolbar-mode__content-row">
          {showInput ? (
            <>
              <input
                type="text"
                className="bitfun-toolbar-mode__input-field"
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
                onKeyDown={handleKeyDown}
                onCompositionStart={handleCompositionStart}
                onCompositionEnd={handleCompositionEnd}
                placeholder={t('input.placeholder')}
                autoFocus
              />
              <Tooltip content={t('input.send')}>
                <button 
                  className="toolbar-btn toolbar-btn--send"
                  onClick={handleSendMessage}
                  disabled={!inputValue.trim()}
                >
                  <Send size={16} />
                </button>
              </Tooltip>
              <Tooltip content={t('planner.cancel')}>
                <button 
                  className="toolbar-btn"
                  onClick={() => setShowInput(false)}
                >
                  <X size={16} />
                </button>
              </Tooltip>
            </>
          ) : (
            <>
              <div className="bitfun-toolbar-mode__stream-content" onClick={toggleExpanded}>
                {currentStreamState.toolName ? (
                  <div className="bitfun-toolbar-mode__tool">
                    <span className="bitfun-toolbar-mode__tool-name">{currentStreamState.toolName}</span>
                    <span className="bitfun-toolbar-mode__tool-summary">{currentStreamState.content || t('toolCards.toolbar.executing')}</span>
                  </div>
                ) : toolbarState.todoProgress && toolbarState.todoProgress.total > 0 ? (
                  <div className="bitfun-toolbar-mode__todo">
                    <span className="bitfun-toolbar-mode__todo-progress">
                      {toolbarState.todoProgress.completed}/{toolbarState.todoProgress.total}
                    </span>
                    <span className="bitfun-toolbar-mode__todo-current">
                      {toolbarState.todoProgress.current || currentStreamState.content}
                    </span>
                  </div>
                ) : (
                  <span className={`bitfun-toolbar-mode__text ${currentStreamState.isStreaming ? 'bitfun-toolbar-mode__text--streaming' : ''}`}>
                    {currentStreamState.content || (currentStreamState.isStreaming ? t('toolCards.toolbar.processing') : (lastMessageContent || t('toolCards.toolbar.startNewChat')))}
                  </span>
                )}
              </div>
              
              <div className="bitfun-toolbar-mode__controls">
                {toolbarState.hasPendingConfirmation && (
                  <>
                    <Tooltip content={t('toolCards.common.confirm')}>
                      <button className="toolbar-btn toolbar-btn--confirm" onClick={handleConfirm}>
                        <Check size={16} />
                      </button>
                    </Tooltip>
                    <Tooltip content={t('toolCards.common.cancel')}>
                      <button className="toolbar-btn toolbar-btn--reject" onClick={handleReject}>
                        <X size={16} />
                      </button>
                    </Tooltip>
                  </>
                )}
                
                {currentStreamState.isStreaming && !toolbarState.hasPendingConfirmation && (
                  <Tooltip content={t('planner.cancel')}>
                    <button className="toolbar-btn toolbar-btn--cancel-compact" onClick={handleCancel}>
                      <Square size={12} />
                    </button>
                  </Tooltip>
                )}
                
                <Tooltip content={t('input.placeholder')}>
                  <button 
                    className="toolbar-btn toolbar-btn--input" 
                    onClick={() => setShowInput(true)}
                  >
                    <MessageSquare size={16} />
                  </button>
                </Tooltip>
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
};

export interface ToolbarModeProps {
  visible?: boolean;
  onExpandToFull?: () => void;
  className?: string;
}

export default ToolbarMode;

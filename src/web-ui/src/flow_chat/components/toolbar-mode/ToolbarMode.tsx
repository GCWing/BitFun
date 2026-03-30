/**
 * Toolbar Mode component.
 * Single-window morph UI for compact toolbar view.
 *
 * Layout: two rows
 * - Row 1: status icons + session title (click to switch)
 * - Row 2: streaming content/input + controls
 */

import React, { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { PhysicalSize, PhysicalPosition } from '@tauri-apps/api/dpi';
import { 
  MessageSquare, 
  Square, 
  Check, 
  X, 
  Send,
  Maximize2,
  ChevronDown,
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

const log = createLogger('ToolbarMode');
import { ModernFlowChatContainer } from '../modern/ModernFlowChatContainer';
import { Tooltip } from '@/component-library';
import './ToolbarMode.scss';

// Window size config (physical pixels, accounts for Windows DPI scaling).
const TOOLBAR_WIDTH = 600;
const TOOLBAR_HEIGHT_NORMAL = 120;  // Two-row height (32px + ~88px).
const TOOLBAR_HEIGHT_EXPANDED = 320; // Height when session list is expanded.

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
  const [showSessionPicker, setShowSessionPicker] = useState(false);
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => 
    flowChatStore.getState()
  );
  const sessionPickerRef = useRef<HTMLDivElement>(null);

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
    return activeSession?.title || t('session.new');
  }, [flowChatState, t]);
  
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
  
  // Window position is initialized in ToolbarModeContext.tsx to avoid conflicts.
  
  // Track the previous picker state to avoid redundant resize calls.
  const prevShowSessionPickerRef = useRef(showSessionPicker);
  
  useEffect(() => {
    if (prevShowSessionPickerRef.current === showSessionPicker) {
      return;
    }
    prevShowSessionPickerRef.current = showSessionPicker;
    
    const adjustWindowSize = async () => {
      if (isExpanded) return;
      if (!isToolbarMode) return;
      
      try {
        const win = getCurrentWindow();
        const currentPosition = await win.outerPosition();
        const currentSize = await win.outerSize();
        
        if (showSessionPicker) {
          const heightDiff = TOOLBAR_HEIGHT_EXPANDED - currentSize.height;
          const newY = currentPosition.y - heightDiff;
          
          await win.setSize(new PhysicalSize(TOOLBAR_WIDTH, TOOLBAR_HEIGHT_EXPANDED));
          await win.setPosition(new PhysicalPosition(currentPosition.x, Math.max(0, newY)));
        } else {
          const heightDiff = currentSize.height - TOOLBAR_HEIGHT_NORMAL;
          const newY = currentPosition.y + heightDiff;
          
          await win.setSize(new PhysicalSize(TOOLBAR_WIDTH, TOOLBAR_HEIGHT_NORMAL));
          await win.setPosition(new PhysicalPosition(currentPosition.x, newY));
        }
      } catch (error) {
        log.error('Failed to adjust window size', { isToolbarMode, isExpanded, error });
      }
    };
    
    adjustWindowSize();
  }, [isToolbarMode, isExpanded, showSessionPicker]);
  
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (sessionPickerRef.current?.contains(target)) {
        return;
      }
      if (target.closest?.('.bitfun-toolbar-mode__title-btn')) {
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
  
  const handleStartDrag = useCallback(async (e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    // Avoid dragging when interacting with UI controls.
    if (target.closest?.('button, input, .bitfun-toolbar-mode__session-picker, .bitfun-toolbar-mode__stream-content, .bitfun-toolbar-mode__session-item, .bitfun-toolbar-mode__flowchat-container')) {
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
  
  const handleCreateSession = useCallback(() => {
    window.dispatchEvent(new CustomEvent('toolbar-create-session'));
  }, []);
  
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
      e.preventDefault();
      handleSendMessage();
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      if (showInput) {
        setShowInput(false);
      } else if (showSessionPicker) {
        setShowSessionPicker(false);
      } else {
        handleExpand();
      }
    }
  }, [handleSendMessage, showInput, showSessionPicker, handleExpand]);
  
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
      {showSessionPicker && !isExpanded && (
        <div 
          className="bitfun-toolbar-mode__session-picker" 
          ref={sessionPickerRef}
          onMouseDown={(e) => e.stopPropagation()}
        >
          {sessions.map((session) => (
            <button
              key={session.sessionId}
              className={`bitfun-toolbar-mode__session-item ${
                session.sessionId === flowChatState.activeSessionId ? 'bitfun-toolbar-mode__session-item--active' : ''
              }`}
              onMouseDown={(e) => handleSwitchSession(e, session.sessionId)}
            >
              {session.title || t('session.new')}
            </button>
          ))}
        </div>
      )}
      
      <div className="bitfun-toolbar-mode__header">
        <Tooltip content={t('session.newCode')}>
          <button
            className="bitfun-toolbar-mode__create-btn"
            onClick={handleCreateSession}
          >
            <Plus size={14} />
          </button>
        </Tooltip>

        <div className="bitfun-toolbar-mode__title-wrapper">
          <Tooltip content={t('session.switchSession')}>
            <button
              className="bitfun-toolbar-mode__title-btn"
              onClick={() => setShowSessionPicker(!showSessionPicker)}
            >
              <span className="bitfun-toolbar-mode__title-text">{sessionTitle}</span>
              <ChevronDown size={12} className={`bitfun-toolbar-mode__title-chevron ${showSessionPicker ? 'bitfun-toolbar-mode__title-chevron--open' : ''}`} />
            </button>
          </Tooltip>
          
          {showSessionPicker && isExpanded && (
            <div 
              className="bitfun-toolbar-mode__session-dropdown" 
              ref={sessionPickerRef}
              onMouseDown={(e) => e.stopPropagation()}
            >
              {sessions.map((session) => (
                <button
                  key={session.sessionId}
                  className={`bitfun-toolbar-mode__session-item ${
                    session.sessionId === flowChatState.activeSessionId ? 'bitfun-toolbar-mode__session-item--active' : ''
                  }`}
                  onMouseDown={(e) => handleSwitchSession(e, session.sessionId)}
                >
                  {session.title || t('session.new')}
                </button>
              ))}
            </div>
          )}
        </div>
        
        <button
          className="toolbar-btn toolbar-btn--toggle"
          onClick={toggleExpanded}
          title={isExpanded ? t('toolCards.toolbar.collapseChat') : t('toolCards.toolbar.expandChat')}
        >
          {isExpanded ? <PanelTopClose size={14} /> : <PanelTopOpen size={14} />}
        </button>
        
        <Tooltip content={t('session.restoreMain')}>
          <button 
            className="toolbar-btn toolbar-btn--expand"
            onClick={handleExpand}
          >
            <Maximize2 size={14} />
          </button>
        </Tooltip>
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

/**
 * Toolbar Mode component.
 * Single-window morph UI for compact toolbar view.
 *
 * Two window states:
 * - Collapsed: a compact status strip (latest activity + confirm/cancel controls).
 * - Expanded: the main window session surface verbatim — ChatPane renders the
 *   same FlowChat conversation and ChatInput composer the session scene uses, so
 *   this mode never drifts behind the normal chat UI and needs no parallel
 *   conversation/composer implementation of its own.
 */

import React, { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { Menu, MenuItem } from '@bitfun/ui';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  Square,
  Check,
  X,
  Maximize2,
  MoreVertical,
  PanelTopOpen,
  PanelTopClose
} from 'lucide-react';
import { useToolbarModeContext } from './ToolbarModeContext';
import { type FlowToolItem } from '../../types/flow-chat';
import { projectEffectiveToolItem } from '../../utils/toolInvocationIdentity';
import { createLogger } from '@/shared/utils/logger';
import { isMacOSDesktopRuntime } from '@/infrastructure/runtime';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { SessionMenu, useFlowChatSessions } from '../session-menu';

const log = createLogger('ToolbarMode');
import ChatPane from '@/app/scenes/session/ChatPane';
import { Tooltip } from '@bitfun/ui';
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

  const [showHeaderOverflowMenu, setShowHeaderOverflowMenu] = useState(false);
  const headerOverflowTriggerRef = useRef<HTMLButtonElement>(null);
  const headerOverflowRef = useRef<HTMLDivElement>(null);
  const headerOverflowLayout = useAnchoredPopoverPosition({
    open: showHeaderOverflowMenu,
    anchorRef: headerOverflowTriggerRef,
    popoverRef: headerOverflowRef,
    preferredPlacement: 'bottom',
    alignment: 'end',
    gap: 4,
  });

  const isMacOS = useMemo(() => isMacOSDesktopRuntime(), []);
  const { workspacePath } = useCurrentWorkspace();
  const { activeSession, sessionTitle } = useFlowChatSessions();

  const lastMessageContent = useMemo(() => {
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
  }, [activeSession]);

  // Derive current streaming state from session data.
  const currentStreamState = useMemo(() => {
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
        const effectiveItem = projectEffectiveToolItem(item as FlowToolItem);
        toolName = effectiveItem.toolName;
        if (effectiveItem.toolCall?.input && typeof effectiveItem.toolCall.input === 'object') {
          const input = effectiveItem.toolCall.input;
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
  }, [activeSession, t]);

  useEffect(() => {
    if (!isExpanded) {
      setShowHeaderOverflowMenu(false);
    }
  }, [isExpanded]);

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
      'button, input, .bitfun-session-menu, .bitfun-toolbar-mode__overflow-menu, .bitfun-toolbar-mode__stream-content, .bitfun-toolbar-mode__session-surface'
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

  const toggleHeaderOverflowMenu = useCallback(() => {
    if (!isExpanded) return;
    setShowHeaderOverflowMenu(v => !v);
  }, [isExpanded]);

  const handleSessionMenuOpenChange = useCallback((open: boolean) => {
    if (open) setShowHeaderOverflowMenu(false);
  }, []);

  if (!isToolbarMode) {
    return null;
  }

  const containerClassName = [
    'bitfun-toolbar-mode',
    isExpanded && 'bitfun-toolbar-mode--expanded',
    currentStreamState.isStreaming && 'bitfun-toolbar-mode--processing',
    toolbarState.hasError && 'bitfun-toolbar-mode--error',
    toolbarState.hasPendingConfirmation && 'bitfun-toolbar-mode--confirm',
    isMacOS && 'bitfun-toolbar-mode--macos',
  ].filter(Boolean).join(' ');

  return (
    <div data-bf-component="toolbar-mode" data-bf-part="root" data-bf-state={[
      isExpanded && 'expanded',
      currentStreamState.isStreaming && 'processing',
      toolbarState.hasError && 'error',
      toolbarState.hasPendingConfirmation && 'confirm',
    ].filter(Boolean).join(' ') || undefined} className={containerClassName} onMouseDown={handleStartDrag}>
      <div className="bitfun-toolbar-mode__header" data-bf-component="toolbar-mode" data-bf-part="header">
        <div className="bitfun-toolbar-mode__header-left" data-bf-component="toolbar-mode" data-bf-part="headerLeft">
          {isExpanded ? <SessionMenu onOpenChange={handleSessionMenuOpenChange} /> : null}
        </div>

        <div className="bitfun-toolbar-mode__title-wrapper" data-bf-component="toolbar-mode" data-bf-part="title">
          <div className="bitfun-toolbar-mode__title-display" title={sessionTitle}>
            <span className="bitfun-toolbar-mode__title-text">{sessionTitle}</span>
          </div>
        </div>

        <div className="bitfun-toolbar-mode__header-right" data-bf-component="toolbar-mode" data-bf-part="headerActions">
          <div className="bitfun-toolbar-mode__header-drag-area" aria-hidden="true" />
          <div className="bitfun-toolbar-mode__header-overflow">
            {isExpanded ? (
              <>
                <Tooltip content={t('toolCards.toolbar.moreMenu')}>
                  <button
                    ref={headerOverflowTriggerRef}
                    type="button"
                    className="toolbar-btn toolbar-btn--overflow bitfun-toolbar-mode__overflow-trigger"
                    data-bf-component="toolbar-mode"
                    data-bf-part="overflowTrigger"
                    data-bf-state={showHeaderOverflowMenu ? 'open' : undefined}
                    onClick={toggleHeaderOverflowMenu}
                    aria-expanded={showHeaderOverflowMenu}
                    aria-haspopup="menu"
                  >
                    <MoreVertical size={14} />
                  </button>
                </Tooltip>
                {showHeaderOverflowMenu && createPortal(
                  <Menu
                    ref={headerOverflowRef}
                    className="bitfun-toolbar-mode__overflow-menu"
                    data-bf-component="toolbar-mode"
                    data-bf-part="overflowMenu"
                    data-bf-state="open"
                    data-bf-placement={headerOverflowLayout?.placement ?? 'bottom'}
                    style={{
                      top: `${headerOverflowLayout?.top ?? 0}px`,
                      left: `${headerOverflowLayout?.left ?? 0}px`,
                      visibility: headerOverflowLayout ? 'visible' : 'hidden',
                    }}
                    onMouseDown={(e) => e.stopPropagation()}
                  >
                    <MenuItem
                      type="button"
                      leading={<PanelTopClose size={14} />}
                      data-bf-component="toolbar-mode"
                      data-bf-part="overflowItem"
                      onClick={() => {
                        void toggleExpanded();
                        setShowHeaderOverflowMenu(false);
                      }}
                    >
                      <span>{t('toolCards.toolbar.collapseChat')}</span>
                    </MenuItem>
                    <MenuItem
                      type="button"
                      leading={<Maximize2 size={14} />}
                      data-bf-component="toolbar-mode"
                      data-bf-part="overflowItem"
                      onClick={() => {
                        void handleExpand();
                        setShowHeaderOverflowMenu(false);
                      }}
                    >
                      <span>{t('session.restoreMain')}</span>
                    </MenuItem>
                  </Menu>,
                  getAppearanceOverlayHost(),
                )}
              </>
            ) : (
              <div className="bitfun-toolbar-mode__header-collapsed-actions" data-bf-component="toolbar-mode" data-bf-part="collapsedActions">
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
        /* Main window session surface, reused as-is: same conversation view and
           same full composer, so there is nothing extra to maintain here. */
        <div className="bitfun-toolbar-mode__session-surface" data-bf-component="toolbar-mode" data-bf-part="sessionSurface">
          <ChatPane
            width={0}
            isFullscreen={false}
            isSceneActive
            workspacePath={workspacePath}
            showChatInput
          />
        </div>
      ) : (
        <div className="bitfun-toolbar-mode__content-row" data-bf-component="toolbar-mode" data-bf-part="content">
          <div className="bitfun-toolbar-mode__stream-content" onClick={toggleExpanded} data-bf-component="toolbar-mode" data-bf-part="stream" data-bf-content-kind={currentStreamState.toolName ? 'tool' : toolbarState.todoProgress && toolbarState.todoProgress.total > 0 ? 'todo' : 'text'} data-bf-state={currentStreamState.isStreaming ? 'streaming' : undefined}>
            {currentStreamState.toolName ? (
              <div className="bitfun-toolbar-mode__tool" data-bf-component="toolbar-mode" data-bf-part="tool">
                <span className="bitfun-toolbar-mode__tool-name" data-bf-component="toolbar-mode" data-bf-part="toolName">{currentStreamState.toolName}</span>
                <span className="bitfun-toolbar-mode__tool-summary" data-bf-component="toolbar-mode" data-bf-part="toolSummary">{currentStreamState.content || t('toolCards.toolbar.executing')}</span>
              </div>
            ) : toolbarState.todoProgress && toolbarState.todoProgress.total > 0 ? (
              <div className="bitfun-toolbar-mode__todo" data-bf-component="toolbar-mode" data-bf-part="todo">
                <span className="bitfun-toolbar-mode__todo-progress" data-bf-component="toolbar-mode" data-bf-part="todoProgress">
                  {toolbarState.todoProgress.completed}/{toolbarState.todoProgress.total}
                </span>
                <span className="bitfun-toolbar-mode__todo-current" data-bf-component="toolbar-mode" data-bf-part="todoCurrent">
                  {toolbarState.todoProgress.current || currentStreamState.content}
                </span>
              </div>
            ) : (
              <span className={`bitfun-toolbar-mode__text ${currentStreamState.isStreaming ? 'bitfun-toolbar-mode__text--streaming' : ''}`} data-bf-component="toolbar-mode" data-bf-part="streamText">
                {currentStreamState.content || (currentStreamState.isStreaming ? t('toolCards.toolbar.processing') : (lastMessageContent || t('toolCards.toolbar.startNewChat')))}
              </span>
            )}
          </div>

          <div className="bitfun-toolbar-mode__controls" data-bf-component="toolbar-mode" data-bf-part="controls">
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
          </div>
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

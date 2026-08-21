/**
 * FlowChat header.
 * Shows the currently viewed turn and user message.
 * Height matches side panel headers (40px).
 */

import React, { useEffect, useLayoutEffect, useMemo, useRef, useState, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { ChevronDown, ChevronUp, GitPullRequest, Keyboard, MoreHorizontal, Maximize2, Minimize2, Move, Search, Square, SquareTerminal, Terminal, X } from 'lucide-react';
import { Tooltip, IconButton, Input } from '@/component-library';
import { useTranslation } from 'react-i18next';
import { useApp, useChatFullWidth } from '@/app/hooks/useApp';
import { SessionFilesBadge } from './SessionFilesBadge';
import { SessionTreePopover, type SessionTreeSelection } from './SessionTreePopover';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance';
import { computeFixedPopoverPosition } from '@/shared/utils/fixedPopoverViewport';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { createReviewPlatformTab } from '@/shared/utils/tabUtils';
import './FlowChatHeader.scss';

export interface FlowChatHeaderCommandSummary {
  execSessionKey: string;
  execSessionId: number;
  title: string;
  command: string;
  status: 'running' | 'exited' | 'interrupted' | 'killed' | 'pruned' | 'failed';
  remote?: boolean;
  tty?: boolean;
  exitCode?: number;
  elapsedMs?: number;
  isStopping?: boolean;
}

export interface FlowChatHeaderProps {
  /** Current turn index. */
  currentTurn: number;
  /** Total turns. */
  totalTurns: number;
  /** Current user message. */
  currentUserMessage: string;
  /** Whether the header is visible. */
  visible: boolean;
  /** Session ID. */
  sessionId?: string;
  /** Jump to the currently displayed turn. */
  onJumpToCurrentTurn?: () => void;
  /** Current search query string. */
  searchQuery?: string;
  /** Called when the user types in the search box. */
  onSearchChange?: (query: string) => void;
  /** Total number of search matches. */
  searchMatchCount?: number;
  /** 1-based index of the currently focused match. */
  searchCurrentMatch?: number;
  /** Navigate to the next match. */
  onSearchNext?: () => void;
  /** Navigate to the previous match. */
  onSearchPrev?: () => void;
  /** Called when the user closes the search bar. */
  onSearchClose?: () => void;
  /** Increments each time the parent requests to open the search bar. */
  searchOpenRequest?: number;
  /** Open a Session from the active Agent tree. */
  onOpenSessionTreeSession?: (selection: SessionTreeSelection) => void;
  /** Whether the active Agent tree contains running descendants. */
  hasActiveSessionTreeDescendants?: boolean;
  /** Cancel one running session from the active Agent tree without cancelling descendants. */
  onCancelSessionTreeSession?: (selection: SessionTreeSelection) => Promise<boolean>;
  /** Long-running background commands launched by the active parent session. */
  backgroundCommands?: FlowChatHeaderCommandSummary[];
  /** Open a read-only output panel for a background command. */
  onOpenBackgroundCommandOutput?: (command: FlowChatHeaderCommandSummary) => void;
  /** Request user-provided stdin for an interactive background command. */
  onRequestBackgroundCommandInput?: (command: FlowChatHeaderCommandSummary) => void;
  /** Stop a running background command. */
  onStopBackgroundCommand?: (command: FlowChatHeaderCommandSummary) => void;
  /** Stop all running background commands. */
  onStopAllBackgroundCommands?: () => void;
  /**
   * Host-owned content rendered inside the left action group, after
   * SessionFilesBadge (R-GC-24: group chat menu reuses the original header
   * left-actions slot instead of a custom top bar).
   */
  leftActionsContent?: React.ReactNode;
}
export const FlowChatHeader: React.FC<FlowChatHeaderProps> = ({
  currentTurn,
  totalTurns,
  currentUserMessage,
  visible,
  sessionId,
  onJumpToCurrentTurn,
  searchQuery = '',
  onSearchChange,
  searchMatchCount = 0,
  searchCurrentMatch = 0,
  onSearchNext,
  onSearchPrev,
  onSearchClose,
  searchOpenRequest = 0,
  onOpenSessionTreeSession,
  hasActiveSessionTreeDescendants = false,
  onCancelSessionTreeSession,
  backgroundCommands = [],
  onOpenBackgroundCommandOutput,
  onRequestBackgroundCommandInput,
  onStopBackgroundCommand,
  onStopAllBackgroundCommands,
  leftActionsContent,
}) => {
  const { t } = useTranslation('flow-chat');
  const { toggleChatFullWidth } = useApp();
  const isChatFullWidth = useChatFullWidth();
  const { currentWorkspace } = useWorkspaceContext();
  const [isBackgroundCommandPanelOpen, setIsBackgroundCommandPanelOpen] = useState(false);
  const [isBackgroundCommandSectionMenuOpen, setIsBackgroundCommandSectionMenuOpen] = useState(false);
  const [openBackgroundCommandMenuId, setOpenBackgroundCommandMenuId] = useState<string | null>(null);
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const headerRef = useRef<HTMLDivElement | null>(null);
  const leftActionsRef = useRef<HTMLDivElement | null>(null);
  const rightActionsRef = useRef<HTMLDivElement | null>(null);
  const backgroundCommandRootRef = useRef<HTMLDivElement | null>(null);
  const backgroundCommandTriggerRef = useRef<HTMLButtonElement | null>(null);
  const backgroundCommandPanelRef = useRef<HTMLDivElement | null>(null);
  const backgroundCommandMenuAnchorRef = useRef<HTMLButtonElement | null>(null);
  const backgroundCommandMenuRef = useRef<HTMLDivElement | null>(null);
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const [backgroundCommandMenuPosition, setBackgroundCommandMenuPosition] = useState<{
    top: number;
    left: number;
  } | null>(null);

  // Truncate long messages.
  const truncatedMessage = currentUserMessage.length > 50
    ? currentUserMessage.slice(0, 50) + '...'
    : currentUserMessage;
  const turnBadgeLabel = t('flowChatHeader.turnBadge', {
    current: currentTurn
  });
  const hasBackgroundCommands = backgroundCommands.length > 0;
  const backgroundCommandCount = backgroundCommands.length;
  const displayBackgroundCommands = useMemo(() => (
    backgroundCommands.map((command) => ({
      ...command,
      title: command.title.trim() || t('flowChatHeader.backgroundCommandUntitled'),
    }))
  ), [backgroundCommands, t]);
  const hasNoResults = searchQuery.trim().length > 0 && searchMatchCount === 0;
  const hasOpenBackgroundCommandMenu =
    isBackgroundCommandSectionMenuOpen ||
    openBackgroundCommandMenuId !== null;
  const backgroundCommandPanelLayout = useAnchoredPopoverPosition({
    open: isBackgroundCommandPanelOpen,
    anchorRef: backgroundCommandTriggerRef,
    popoverRef: backgroundCommandPanelRef,
    preferredPlacement: 'bottom',
    alignment: 'end',
    gap: 8,
    layoutRevision: backgroundCommandCount,
  });

  const updateBackgroundCommandMenuPosition = useCallback(() => {
    const anchor = backgroundCommandMenuAnchorRef.current;
    if (!anchor) return;

    const menu = backgroundCommandMenuRef.current;
    const { top, left } = computeFixedPopoverPosition(
      anchor.getBoundingClientRect(),
      menu?.offsetWidth ?? 200,
      menu?.offsetHeight ?? 96,
      4,
      8,
    );
    setBackgroundCommandMenuPosition({ top, left });
  }, []);

  const prepareBackgroundCommandMenu = useCallback((anchor: HTMLButtonElement) => {
    backgroundCommandMenuAnchorRef.current = anchor;
    updateBackgroundCommandMenuPosition();
  }, [updateBackgroundCommandMenuPosition]);

  useEffect(() => {
    if (!isBackgroundCommandPanelOpen) return;

    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (
        !backgroundCommandRootRef.current?.contains(target) &&
        !backgroundCommandPanelRef.current?.contains(target) &&
        !backgroundCommandMenuRef.current?.contains(target)
      ) {
        setIsBackgroundCommandPanelOpen(false);
        setIsBackgroundCommandSectionMenuOpen(false);
        setOpenBackgroundCommandMenuId(null);
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsBackgroundCommandPanelOpen(false);
        setIsBackgroundCommandSectionMenuOpen(false);
        setOpenBackgroundCommandMenuId(null);
      }
    };

    document.addEventListener('mousedown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isBackgroundCommandPanelOpen]);

  useLayoutEffect(() => {
    if (!hasOpenBackgroundCommandMenu) {
      setBackgroundCommandMenuPosition(null);
      return;
    }

    updateBackgroundCommandMenuPosition();
    window.addEventListener('resize', updateBackgroundCommandMenuPosition);
    window.addEventListener('scroll', updateBackgroundCommandMenuPosition, true);

    return () => {
      window.removeEventListener('resize', updateBackgroundCommandMenuPosition);
      window.removeEventListener('scroll', updateBackgroundCommandMenuPosition, true);
    };
  }, [
    hasOpenBackgroundCommandMenu,
    isBackgroundCommandSectionMenuOpen,
    openBackgroundCommandMenuId,
    updateBackgroundCommandMenuPosition,
  ]);

  const prevSearchOpenRequestRef = useRef(0);
  useEffect(() => {
    if (searchOpenRequest > 0 && searchOpenRequest !== prevSearchOpenRequestRef.current) {
      prevSearchOpenRequestRef.current = searchOpenRequest;
      setIsSearchOpen(true);
    }
  }, [searchOpenRequest]);

  useEffect(() => {
    if (hasBackgroundCommands) return;

    setIsBackgroundCommandSectionMenuOpen(false);
    setOpenBackgroundCommandMenuId(null);
    setBackgroundCommandMenuPosition(null);
  }, [hasBackgroundCommands]);

  useEffect(() => {
    if (!isSearchOpen) return;

    const frameId = requestAnimationFrame(() => {
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    });

    return () => {
      cancelAnimationFrame(frameId);
    };
  }, [isSearchOpen]);

  useLayoutEffect(() => {
    const header = headerRef.current;
    const leftActions = leftActionsRef.current;
    const rightActions = rightActionsRef.current;
    if (!header || !leftActions || !rightActions) return;

    const updateSideWidth = () => {
      const sideWidth = Math.ceil(Math.max(
        leftActions.getBoundingClientRect().width,
        rightActions.getBoundingClientRect().width,
      ));
      header.style.setProperty('--bf-appearance-token-flowchat-header-side-width', `${sideWidth}px`);
    };

    updateSideWidth();

    if (typeof ResizeObserver === 'undefined') {
      window.addEventListener('resize', updateSideWidth);
      return () => window.removeEventListener('resize', updateSideWidth);
    }

    const observer = new ResizeObserver(updateSideWidth);
    observer.observe(leftActions);
    observer.observe(rightActions);

    return () => observer.disconnect();
  }, [isSearchOpen, totalTurns, visible, sessionId]);

  const handleOpenSearch = useCallback(() => {
    setIsSearchOpen(true);
  }, []);

  const handleCloseSearch = useCallback(() => {
    setIsSearchOpen(false);
    onSearchClose?.();
  }, [onSearchClose]);

  const handleSearchKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Escape') {
        handleCloseSearch();
        e.preventDefault();
        return;
      }

      if (e.key === 'Enter') {
        if (e.shiftKey) {
          onSearchPrev?.();
        } else {
          onSearchNext?.();
        }
        e.preventDefault();
      }
    },
    [handleCloseSearch, onSearchNext, onSearchPrev],
  );

  const handleToggleBackgroundCommandPanel = () => {
    setIsBackgroundCommandSectionMenuOpen(false);
    setOpenBackgroundCommandMenuId(null);
    setIsBackgroundCommandPanelOpen(prev => !prev);
  };

  const handleOpenPullRequests = useCallback(() => {
    createReviewPlatformTab(currentWorkspace?.rootPath);
  }, [currentWorkspace?.rootPath]);

  const handleCommandSectionMenuToggle = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    if (isBackgroundCommandSectionMenuOpen) {
      setBackgroundCommandMenuPosition(null);
    } else {
      prepareBackgroundCommandMenu(event.currentTarget);
    }
    setOpenBackgroundCommandMenuId(null);
    setIsBackgroundCommandSectionMenuOpen(open => !open);
  };

  const handleCommandSelect = (command: FlowChatHeaderCommandSummary) => {
    onOpenBackgroundCommandOutput?.(command);
    setIsBackgroundCommandPanelOpen(false);
  };

  const handleCommandMenuToggle = (
    event: React.MouseEvent<HTMLButtonElement>,
    command: FlowChatHeaderCommandSummary,
  ) => {
    event.preventDefault();
    event.stopPropagation();
    if (openBackgroundCommandMenuId === command.execSessionKey) {
      setBackgroundCommandMenuPosition(null);
    } else {
      prepareBackgroundCommandMenu(event.currentTarget);
    }
    setIsBackgroundCommandSectionMenuOpen(false);
    setOpenBackgroundCommandMenuId(previous => previous === command.execSessionKey ? null : command.execSessionKey);
  };

  const handleCommandInputRequest = (
    event: React.MouseEvent<HTMLButtonElement>,
    command: FlowChatHeaderCommandSummary,
  ) => {
    event.preventDefault();
    event.stopPropagation();
    onRequestBackgroundCommandInput?.(command);
    setOpenBackgroundCommandMenuId(null);
    setIsBackgroundCommandPanelOpen(false);
  };

  const handleCommandStop = (
    event: React.MouseEvent<HTMLButtonElement>,
    command: FlowChatHeaderCommandSummary,
  ) => {
    event.preventDefault();
    event.stopPropagation();
    onStopBackgroundCommand?.(command);
    setOpenBackgroundCommandMenuId(null);
  };

  const handleCommandStopAll = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    onStopAllBackgroundCommands?.();
    setIsBackgroundCommandSectionMenuOpen(false);
  };

  const renderBackgroundCommandActions = (command: FlowChatHeaderCommandSummary) => {
    const canSendBackgroundCommandInput =
      command.status === 'running' &&
      command.tty === true &&
      !!onRequestBackgroundCommandInput;
    const canStopBackgroundCommand =
      command.status === 'running' &&
      !!onStopBackgroundCommand;

    if (!canSendBackgroundCommandInput && !canStopBackgroundCommand) {
      return null;
    }

    return (
      <div
        className="flowchat-header__background-command-actions"
        data-bf-component="flow-chat-header"
        data-bf-part="backgroundActivity"
      >
        <IconButton
          className="flowchat-header__background-command-menu-button"
          variant="ghost"
          size="xs"
          onClick={(event) => handleCommandMenuToggle(event, command)}
          tooltip={t('flowChatHeader.backgroundCommandActions')}
          aria-label={t('flowChatHeader.backgroundCommandActions')}
          aria-haspopup="menu"
          aria-expanded={openBackgroundCommandMenuId === command.execSessionKey}
        >
          <MoreHorizontal size={13} aria-hidden="true" />
        </IconButton>
        {openBackgroundCommandMenuId === command.execSessionKey && backgroundCommandMenuPosition ? createPortal(
          <div
            ref={backgroundCommandMenuRef}
            className="flowchat-header__background-command-menu flowchat-header__background-command-menu--portal"
            data-bf-component="flow-chat-header"
            data-bf-part="commandMenu"
            role="menu"
            aria-label={t('flowChatHeader.backgroundCommandActions')}
            style={backgroundCommandMenuPosition}
            data-testid="flowchat-header-background-menu"
          >
            {canSendBackgroundCommandInput ? (
              <button
                type="button"
                role="menuitem"
                className="flowchat-header__background-command-menu-item"
                data-bf-component="flow-chat-header"
                data-bf-part="commandItem"
                onClick={(event) => handleCommandInputRequest(event, command)}
              >
                <Keyboard size={12} aria-hidden="true" />
                <span>{t('flowChatHeader.backgroundCommandSendInput')}</span>
              </button>
            ) : null}
            {canStopBackgroundCommand ? (
              <button
                type="button"
                role="menuitem"
                className="flowchat-header__background-command-menu-item flowchat-header__background-command-menu-item--danger"
                data-bf-component="flow-chat-header"
                data-bf-part="commandItem"
                onClick={(event) => handleCommandStop(event, command)}
                disabled={command.isStopping === true}
              >
                <Square size={12} aria-hidden="true" />
                <span>
                  {command.isStopping
                    ? t('flowChatHeader.backgroundCommandStopping')
                    : t('flowChatHeader.backgroundCommandStop')}
                </span>
              </button>
            ) : null}
          </div>,
          getAppearanceOverlayHost(),
        ) : null}
      </div>
    );
  };

  const backgroundCommandLabel = t('flowChatHeader.backgroundCommands', {
    count: backgroundCommandCount,
  });

  // Render whenever there is an active session, even before the first turn is
  // produced: the header hosts the full-width toggle and the L0 drag handle,
  // which must be reachable from the very first message (or an empty session).
  // Previously `totalTurns === 0` returned null, hiding those actions entirely
  // in empty/new sessions — the reported "changes not effective" symptom.
  // (Upstream unmounts on `!visible` to avoid list resize mid-session; we keep
  // the header mounted for the active session while withholding the centre
  // message via `hasTurnInfo` below — both concerns satisfied.)
  if (!visible && !sessionId) {
    return null;
  }
  const hasTurnInfo = totalTurns > 0;

  return (
    <div
      className="flowchat-header"
      ref={headerRef}
      data-bf-component="flow-chat-header"
      data-bf-part="root"
    >
      <div
        className="flowchat-header__actions flowchat-header__actions--left"
        ref={leftActionsRef}
        data-bf-component="flow-chat-header"
        data-bf-part="leftActions"
      >
        <SessionFilesBadge sessionId={sessionId} />
        {leftActionsContent}
      </div>

      {hasTurnInfo ? (
        <Tooltip content={currentUserMessage} placement="bottom">
          <div
            className="flowchat-header__message"
            data-bf-component="flow-chat-header"
            data-bf-part="message"
            role="button"
            tabIndex={0}
            onClick={onJumpToCurrentTurn}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onJumpToCurrentTurn?.();
              }
            }}
            aria-label={t('flowChatHeader.jumpToCurrentTurn', {
              turn: currentTurn
            })}
          >
            <span
              className="flowchat-header__turn-badge"
              aria-label={turnBadgeLabel}
              data-bf-component="flow-chat-header"
              data-bf-part="turnBadge"
            >
              <span>{turnBadgeLabel}</span>
            </span>
            <span className="flowchat-header__message-text">
              {truncatedMessage}
            </span>
          </div>
        </Tooltip>
      ) : null}

      <div
        className="flowchat-header__actions"
        ref={rightActionsRef}
        data-bf-component="flow-chat-header"
        data-bf-part="actions"
      >
        <SessionTreePopover
          sessionId={sessionId}
          fallbackWorkspacePath={currentWorkspace?.rootPath}
          onSelectSession={onOpenSessionTreeSession}
          hasActiveDescendants={hasActiveSessionTreeDescendants}
          onCancelSession={onCancelSessionTreeSession}
          t={t}
        />
        <div
          className="flowchat-header__background-command-nav"
          ref={backgroundCommandRootRef}
          data-bf-component="flow-chat-header"
          data-bf-part="backgroundActivity"
        >
          <IconButton
            ref={backgroundCommandTriggerRef}
            className={[
              'flowchat-header__background-command-nav-button',
              isBackgroundCommandPanelOpen && 'flowchat-header__background-command-nav-button--active',
              hasBackgroundCommands && 'flowchat-header__background-command-nav-button--has-commands',
            ].filter(Boolean).join(' ')}
            variant="ghost"
            size="xs"
            onClick={handleToggleBackgroundCommandPanel}
            tooltip={backgroundCommandLabel}
            aria-label={backgroundCommandLabel}
            aria-expanded={isBackgroundCommandPanelOpen}
            aria-haspopup="dialog"
            data-testid="flowchat-header-background-commands"
          >
            <span className="flowchat-header__background-command-nav-button-inner">
              <SquareTerminal size={14} />
              {hasBackgroundCommands ? (
                <span
                  className="flowchat-header__background-command-status-dot"
                  aria-hidden="true"
                />
              ) : null}
            </span>
          </IconButton>

          {isBackgroundCommandPanelOpen && createPortal(
            <div
              ref={backgroundCommandPanelRef}
              className="flowchat-header__background-command-panel"
              data-bf-component="flow-chat-header"
              data-bf-part="activityPanel"
              data-bf-placement={backgroundCommandPanelLayout?.placement ?? 'bottom'}
              role="dialog"
              aria-label={backgroundCommandLabel}
              style={{
                top: `${backgroundCommandPanelLayout?.top ?? 0}px`,
                left: `${backgroundCommandPanelLayout?.left ?? 0}px`,
                visibility: backgroundCommandPanelLayout ? 'visible' : 'hidden',
              }}
            >
              <div className="flowchat-header__background-command-panel-header">
                <span>{backgroundCommandLabel}</span>
                <div className="flowchat-header__background-command-panel-header-actions">
                  {hasBackgroundCommands && onStopAllBackgroundCommands ? (
                    <IconButton
                      className="flowchat-header__background-command-menu-button"
                      variant="ghost"
                      size="xs"
                      onClick={handleCommandSectionMenuToggle}
                      tooltip={t('flowChatHeader.backgroundCommandActions')}
                      aria-label={t('flowChatHeader.backgroundCommandActions')}
                      aria-haspopup="menu"
                      aria-expanded={isBackgroundCommandSectionMenuOpen}
                      disabled={displayBackgroundCommands.every(command => (
                        command.status !== 'running' || command.isStopping === true
                      ))}
                    >
                      <MoreHorizontal size={13} aria-hidden="true" />
                    </IconButton>
                  ) : null}
                  {isBackgroundCommandSectionMenuOpen && backgroundCommandMenuPosition ? createPortal(
                    <div
                      ref={backgroundCommandMenuRef}
                      className="flowchat-header__background-command-menu flowchat-header__background-command-menu--portal"
                      data-bf-component="flow-chat-header"
                      data-bf-part="commandMenu"
                      role="menu"
                      aria-label={t('flowChatHeader.backgroundCommandActions')}
                      style={backgroundCommandMenuPosition}
                      data-testid="flowchat-header-background-menu"
                    >
                      <button
                        type="button"
                        role="menuitem"
                        className="flowchat-header__background-command-menu-item flowchat-header__background-command-menu-item--danger"
                        data-bf-component="flow-chat-header"
                        data-bf-part="commandItem"
                        onClick={handleCommandStopAll}
                      >
                        <Square size={12} aria-hidden="true" />
                        <span>{t('flowChatHeader.backgroundCommandStopAll')}</span>
                      </button>
                    </div>,
                    getAppearanceOverlayHost(),
                  ) : null}
                </div>
              </div>
              <div
                className="flowchat-header__background-command-list"
                data-bf-component="flow-chat-header"
                data-bf-part="activitySection"
              >
                {hasBackgroundCommands ? displayBackgroundCommands.map((command) => (
                  <div
                    key={command.execSessionKey}
                    className="flowchat-header__background-command-list-item"
                  >
                    <button
                      type="button"
                      className="flowchat-header__background-command-list-item-button flowchat-header__background-command-open-button"
                      onClick={() => handleCommandSelect(command)}
                    >
                      <span className="flowchat-header__background-command-list-title">
                        <Terminal size={12} aria-hidden="true" />
                        <span>{command.title}</span>
                      </span>
                      <span className="flowchat-header__background-command-list-meta">
                        {[
                          t('flowChatHeader.backgroundCommandSession', { id: command.execSessionId }),
                          command.status === 'running'
                            ? t('flowChatHeader.backgroundCommandStatusRunning')
                            : t('flowChatHeader.backgroundCommandStatusFinished'),
                        ].filter(Boolean).join(' · ')}
                      </span>
                    </button>
                    {renderBackgroundCommandActions(command)}
                  </div>
                )) : (
                  <div className="flowchat-header__background-command-empty-state">
                    {t('flowChatHeader.backgroundCommandEmpty')}
                  </div>
                )}
              </div>
            </div>,
            getAppearanceOverlayHost(),
          )}
        </div>

        <IconButton
          className="flowchat-header__review-platform-btn"
          variant="ghost"
          size="xs"
          onClick={handleOpenPullRequests}
          tooltip={t('flowChatHeader.pullRequests')}
          aria-label={t('flowChatHeader.pullRequests')}
          data-testid="flowchat-header-pull-requests"
        >
          <GitPullRequest size={14} />
        </IconButton>
        {isSearchOpen ? (
          <div
            className="flowchat-header__search"
            role="search"
            data-testid="flowchat-header-search-bar"
            data-bf-component="flow-chat-header"
            data-bf-part="search"
          >
            <Input
              ref={searchInputRef}
              className="flowchat-header__search-field"
              variant="filled"
              inputSize="small"
              prefix={<Search size={12} className="flowchat-header__search-prefix-icon" aria-hidden="true" />}
              suffix={
                <span
                  className="flowchat-header__search-inline-controls"
                  data-bf-component="flow-chat-header"
                  data-bf-part="searchControls"
                >
                  <span className="flowchat-header__search-count" aria-live="polite">
                    {searchQuery.trim()
                      ? hasNoResults
                        ? t('flowChatHeader.searchNoResults')
                        : t('flowChatHeader.searchResult', {
                          current: searchCurrentMatch,
                          total: searchMatchCount
                        })
                      : null}
                  </span>
                  <span className="flowchat-header__search-nav">
                    <button
                      className="flowchat-header__search-nav-btn"
                      onClick={onSearchPrev}
                      disabled={searchMatchCount === 0}
                      title={t('flowChatHeader.searchPrevious')}
                      aria-label={t('flowChatHeader.searchPrevious')}
                      type="button"
                    >
                      <ChevronUp size={10} />
                    </button>
                    <button
                      className="flowchat-header__search-nav-btn"
                      onClick={onSearchNext}
                      disabled={searchMatchCount === 0}
                      title={t('flowChatHeader.searchNext')}
                      aria-label={t('flowChatHeader.searchNext')}
                      type="button"
                    >
                      <ChevronDown size={10} />
                    </button>
                  </span>
                </span>
              }
              type="text"
              value={searchQuery}
              onChange={e => onSearchChange?.(e.target.value)}
              onKeyDown={handleSearchKeyDown}
              placeholder={t('flowChatHeader.searchPlaceholder')}
              aria-label={t('flowChatHeader.searchPlaceholder')}
              error={hasNoResults}
            />
            <IconButton
              className="flowchat-header__search-close"
              variant="ghost"
              size="xs"
              onClick={handleCloseSearch}
              tooltip={t('flowChatHeader.searchClose')}
              aria-label={t('flowChatHeader.searchClose')}
            >
              <X size={14} />
            </IconButton>
          </div>
        ) : (
          <IconButton
            className="flowchat-header__search-btn"
            variant="ghost"
            size="xs"
            onClick={handleOpenSearch}
            tooltip={t('flowChatHeader.searchOpen')}
            aria-label={t('flowChatHeader.searchOpen')}
            data-testid="flowchat-header-search"
          >
            <Search size={14} />
          </IconButton>
        )}

        {/* Drag L0 conversation into the auxiliary canvas — same row as the
            header action group, non-floating (original icon style) */}
        {sessionId ? (
          <IconButton
            className="flowchat-header__drag-session-btn"
            variant="ghost"
            size="xs"
            draggable
            onDragStart={(e) => {
              e.dataTransfer.setData(
                'application/x-bitfun-chat-session',
                JSON.stringify({ sessionId, title: currentUserMessage || 'Chat' }),
              );
              e.dataTransfer.effectAllowed = 'copy';
            }}
            tooltip={t('flowChatHeader.dragToAuxiliary')}
            aria-label={t('flowChatHeader.dragToAuxiliary')}
            data-testid="flowchat-header-drag-session"
          >
            <Move size={14} />
          </IconButton>
        ) : null}

        {/* Full-width tiled chat toggle — same row as the header action group */}
        <IconButton
          className="flowchat-header__fullwidth-btn"
          variant="ghost"
          size="xs"
          onClick={() => toggleChatFullWidth()}
          tooltip={t(isChatFullWidth ? 'layout.fullWidth.exit' : 'layout.fullWidth.enter')}
          aria-label={t(isChatFullWidth ? 'layout.fullWidth.exit' : 'layout.fullWidth.enter')}
          data-testid="session-fullwidth-toggle"
        >
          {isChatFullWidth ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
        </IconButton>
      </div>
    </div>
  );
};

FlowChatHeader.displayName = 'FlowChatHeader';

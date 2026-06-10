/**
 * FlowChat header.
 * Shows the currently viewed turn and user message.
 * Height matches side panel headers (40px).
 */

import React, { useEffect, useMemo, useRef, useState, useCallback } from 'react';
import { Activity, Bot, ChevronDown, ChevronUp, GitPullRequest, Keyboard, List, MoreHorizontal, Search, Square, Terminal, X } from 'lucide-react';
import { Tooltip, IconButton, Input } from '@/component-library';
import { useTranslation } from 'react-i18next';
import { SessionFilesBadge } from './SessionFilesBadge';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { createReviewPlatformTab } from '@/shared/utils/tabUtils';
import './FlowChatHeader.scss';

export interface FlowChatHeaderTurnSummary {
  turnId: string;
  turnIndex: number;
  backendTurnIndex?: number;
  title: string;
}

export interface FlowChatHeaderSubagentSummary {
  sessionId: string;
  title: string;
  agentType?: string;
  status: 'processing' | 'finishing';
}

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
  /** Ordered turn summaries used by header navigation. */
  turns?: FlowChatHeaderTurnSummary[];
  /** Jump to a specific turn. */
  onJumpToTurn?: (turnId: string) => void;
  /** Jump to the currently displayed turn. */
  onJumpToCurrentTurn?: () => void;
  /** Jump to the previous turn. */
  onJumpToPreviousTurn?: () => void;
  /** Jump to the next turn. */
  onJumpToNextTurn?: () => void;
  /** Whether the previous-turn action can navigate within the loaded turn range. */
  canJumpToPreviousTurn?: boolean;
  /** Whether the next-turn action can navigate within the loaded turn range. */
  canJumpToNextTurn?: boolean;
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
  /** Running background subagents launched by the active parent session. */
  backgroundSubagents?: FlowChatHeaderSubagentSummary[];
  /** Long-running background commands launched by the active parent session. */
  backgroundCommands?: FlowChatHeaderCommandSummary[];
  /** Open a background subagent in the right-side panel. */
  onOpenBackgroundSubagent?: (sessionId: string) => void;
  /** Open a read-only output panel for a background command. */
  onOpenBackgroundCommandOutput?: (command: FlowChatHeaderCommandSummary) => void;
  /** Request user-provided stdin for an interactive background command. */
  onRequestBackgroundCommandInput?: (command: FlowChatHeaderCommandSummary) => void;
  /** Stop a running background command. */
  onStopBackgroundCommand?: (command: FlowChatHeaderCommandSummary) => void;
}
export const FlowChatHeader: React.FC<FlowChatHeaderProps> = ({
  currentTurn,
  totalTurns,
  currentUserMessage,
  visible,
  sessionId,
  turns = [],
  onJumpToTurn,
  onJumpToCurrentTurn,
  onJumpToPreviousTurn,
  onJumpToNextTurn,
  canJumpToPreviousTurn,
  canJumpToNextTurn,
  searchQuery = '',
  onSearchChange,
  searchMatchCount = 0,
  searchCurrentMatch = 0,
  onSearchNext,
  onSearchPrev,
  onSearchClose,
  searchOpenRequest = 0,
  backgroundSubagents = [],
  backgroundCommands = [],
  onOpenBackgroundSubagent,
  onOpenBackgroundCommandOutput,
  onRequestBackgroundCommandInput,
  onStopBackgroundCommand,
}) => {
  const { t } = useTranslation('flow-chat');
  const { currentWorkspace } = useWorkspaceContext();
  const [isTurnListOpen, setIsTurnListOpen] = useState(false);
  const [isBackgroundActivityPanelOpen, setIsBackgroundActivityPanelOpen] = useState(false);
  const [openBackgroundCommandMenuId, setOpenBackgroundCommandMenuId] = useState<string | null>(null);
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const turnListRef = useRef<HTMLDivElement | null>(null);
  const backgroundActivityPanelRef = useRef<HTMLDivElement | null>(null);
  const activeTurnItemRef = useRef<HTMLButtonElement | null>(null);
  const searchInputRef = useRef<HTMLInputElement | null>(null);

  // Truncate long messages.
  const truncatedMessage = currentUserMessage.length > 50
    ? currentUserMessage.slice(0, 50) + '...'
    : currentUserMessage;
  const turnListTooltip = t('flowChatHeader.turnList');
  const untitledTurnLabel = t('flowChatHeader.untitledTurn');
  const turnBadgeLabel = t('flowChatHeader.turnBadge', {
    current: currentTurn
  });
  const previousTurnDisabled = !(canJumpToPreviousTurn ?? currentTurn > 1);
  const nextTurnDisabled = !(canJumpToNextTurn ?? (currentTurn > 0 && currentTurn < totalTurns));
  const hasTurnNavigation = turns.length > 0 && !!onJumpToTurn;
  const hasBackgroundSubagents = backgroundSubagents.length > 0;
  const hasBackgroundCommands = backgroundCommands.length > 0;
  const hasBackgroundActivities = hasBackgroundSubagents || hasBackgroundCommands;
  const backgroundActivityCount = backgroundSubagents.length + backgroundCommands.length;
  const displayTurns = useMemo(() => (
    turns.map(turn => ({
      ...turn,
      title: turn.title.trim() || untitledTurnLabel,
    }))
  ), [turns, untitledTurnLabel]);
  const displayBackgroundSubagents = useMemo(() => (
    backgroundSubagents.map((subagent) => ({
      ...subagent,
      title: subagent.title.trim() || t('flowChatHeader.backgroundSubagentUntitled'),
    }))
  ), [backgroundSubagents, t]);
  const displayBackgroundCommands = useMemo(() => (
    backgroundCommands.map((command) => ({
      ...command,
      title: command.title.trim() || t('flowChatHeader.backgroundCommandUntitled'),
    }))
  ), [backgroundCommands, t]);
  const hasNoResults = searchQuery.trim().length > 0 && searchMatchCount === 0;

  useEffect(() => {
    if (!isTurnListOpen && !isBackgroundActivityPanelOpen) return;

    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (
        !turnListRef.current?.contains(target) &&
        !backgroundActivityPanelRef.current?.contains(target)
      ) {
        setIsTurnListOpen(false);
        setIsBackgroundActivityPanelOpen(false);
        setOpenBackgroundCommandMenuId(null);
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsTurnListOpen(false);
        setIsBackgroundActivityPanelOpen(false);
        setOpenBackgroundCommandMenuId(null);
      }
    };

    document.addEventListener('mousedown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isBackgroundActivityPanelOpen, isTurnListOpen]);

  const prevSearchOpenRequestRef = useRef(0);
  useEffect(() => {
    if (searchOpenRequest > 0 && searchOpenRequest !== prevSearchOpenRequestRef.current) {
      prevSearchOpenRequestRef.current = searchOpenRequest;
      setIsSearchOpen(true);
    }
  }, [searchOpenRequest]);

  useEffect(() => {
    setIsTurnListOpen(false);
  }, [currentTurn]);

  useEffect(() => {
    if (!hasBackgroundActivities) {
      setIsBackgroundActivityPanelOpen(false);
    }
  }, [hasBackgroundActivities]);

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

  useEffect(() => {
    if (!isTurnListOpen) return;

    const frameId = requestAnimationFrame(() => {
      activeTurnItemRef.current?.scrollIntoView({
        block: 'center',
        inline: 'nearest',
      });
    });

    return () => {
      cancelAnimationFrame(frameId);
    };
  }, [currentTurn, displayTurns.length, isTurnListOpen]);

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

  const handleToggleTurnList = () => {
    if (!hasTurnNavigation) return;
    setIsBackgroundActivityPanelOpen(false);
    setIsTurnListOpen(prev => !prev);
  };

  const handleToggleBackgroundActivityPanel = () => {
    if (!hasBackgroundActivities) return;
    setIsTurnListOpen(false);
    setOpenBackgroundCommandMenuId(null);
    setIsBackgroundActivityPanelOpen(prev => !prev);
  };

  const handleOpenPullRequests = useCallback(() => {
    createReviewPlatformTab(currentWorkspace?.rootPath);
  }, [currentWorkspace?.rootPath]);

  const handleTurnSelect = (turnId: string) => {
    if (!onJumpToTurn) return;
    onJumpToTurn(turnId);
    setIsTurnListOpen(false);
  };

  const handleSubagentSelect = (sessionId: string) => {
    onOpenBackgroundSubagent?.(sessionId);
    setIsBackgroundActivityPanelOpen(false);
  };

  const handleCommandSelect = (command: FlowChatHeaderCommandSummary) => {
    onOpenBackgroundCommandOutput?.(command);
    setIsBackgroundActivityPanelOpen(false);
  };

  const handleCommandMenuToggle = (
    event: React.MouseEvent<HTMLButtonElement>,
    command: FlowChatHeaderCommandSummary,
  ) => {
    event.preventDefault();
    event.stopPropagation();
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
    setIsBackgroundActivityPanelOpen(false);
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
      <div className="flowchat-header__background-command-actions">
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
        {openBackgroundCommandMenuId === command.execSessionKey ? (
          <div
            className="flowchat-header__background-command-menu"
            role="menu"
            aria-label={t('flowChatHeader.backgroundCommandActions')}
          >
            {canSendBackgroundCommandInput ? (
              <button
                type="button"
                role="menuitem"
                className="flowchat-header__background-command-menu-item"
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
          </div>
        ) : null}
      </div>
    );
  };

  const backgroundActivityLabel = t('flowChatHeader.backgroundActivities', {
    count: backgroundActivityCount,
  });

  if (!visible || totalTurns === 0) {
    return null;
  }

  return (
    <div className="flowchat-header">
      <div className="flowchat-header__actions flowchat-header__actions--left">
        <SessionFilesBadge sessionId={sessionId} />
      </div>

      <Tooltip content={currentUserMessage} placement="bottom">
        <div
          className="flowchat-header__message"
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
          <span className="flowchat-header__turn-badge" aria-label={turnBadgeLabel}>
            <span>{turnBadgeLabel}</span>
          </span>
          <span className="flowchat-header__message-text">
            {truncatedMessage}
          </span>
        </div>
      </Tooltip>

      <div className="flowchat-header__actions">
        <div className="flowchat-header__background-activity-nav" ref={backgroundActivityPanelRef}>
          <IconButton
            className={`flowchat-header__background-activity-nav-button${isBackgroundActivityPanelOpen ? ' flowchat-header__background-activity-nav-button--active' : ''}`}
            variant="ghost"
            size="xs"
            onClick={handleToggleBackgroundActivityPanel}
            tooltip={backgroundActivityLabel}
            disabled={!hasBackgroundActivities}
            aria-label={backgroundActivityLabel}
            aria-expanded={isBackgroundActivityPanelOpen}
            aria-haspopup="dialog"
            data-testid="flowchat-header-background-activities"
          >
            <span className="flowchat-header__background-activity-nav-button-inner">
              <Activity size={14} />
              {hasBackgroundActivities ? (
                <span
                  className="flowchat-header__background-activity-status-dot"
                  aria-hidden="true"
                />
              ) : null}
            </span>
          </IconButton>

          {isBackgroundActivityPanelOpen && hasBackgroundActivities && (
            <div
              className="flowchat-header__background-activity-panel"
              role="dialog"
              aria-label={backgroundActivityLabel}
            >
              <div className="flowchat-header__background-activity-panel-header">
                <span>{backgroundActivityLabel}</span>
                <span>{backgroundActivityCount}</span>
              </div>
              <div className="flowchat-header__background-activity-list">
                {hasBackgroundSubagents && (
                  <div className="flowchat-header__background-section">
                    <div className="flowchat-header__background-section-title">
                      {t('flowChatHeader.backgroundSubagentSection', { count: backgroundSubagents.length })}
                    </div>
                    {displayBackgroundSubagents.map((subagent) => (
                      <button
                        key={subagent.sessionId}
                        type="button"
                        className="flowchat-header__background-activity-list-item"
                        onClick={() => handleSubagentSelect(subagent.sessionId)}
                      >
                        <span className="flowchat-header__background-activity-list-title">
                          <Bot size={12} aria-hidden="true" />
                          <span>{subagent.title}</span>
                        </span>
                        <span className="flowchat-header__background-activity-list-meta">
                          {[
                            subagent.agentType,
                            subagent.status === 'finishing'
                              ? t('flowChatHeader.subagentStatusFinishing')
                              : t('flowChatHeader.subagentStatusProcessing'),
                          ].filter(Boolean).join(' · ')}
                        </span>
                      </button>
                    ))}
                  </div>
                )}
                {hasBackgroundCommands && (
                  <div className="flowchat-header__background-section">
                    <div className="flowchat-header__background-section-title">
                      {t('flowChatHeader.backgroundCommandSection', { count: backgroundCommands.length })}
                    </div>
                    {displayBackgroundCommands.map((command) => (
                      <div
                        key={command.execSessionKey}
                        className="flowchat-header__background-command-list-item"
                      >
                        <button
                          type="button"
                          className="flowchat-header__background-activity-list-item flowchat-header__background-command-open-button"
                          onClick={() => handleCommandSelect(command)}
                        >
                          <span className="flowchat-header__background-activity-list-title">
                            <Terminal size={12} aria-hidden="true" />
                            <span>{command.title}</span>
                          </span>
                          <span className="flowchat-header__background-activity-list-meta">
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
                    ))}
                  </div>
                )}
              </div>
            </div>
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
          <div className="flowchat-header__search" role="search" data-testid="flowchat-header-search-bar">
            <Input
              ref={searchInputRef}
              className="flowchat-header__search-field"
              variant="filled"
              inputSize="small"
              prefix={<Search size={12} className="flowchat-header__search-prefix-icon" aria-hidden="true" />}
              suffix={
                <span className="flowchat-header__search-inline-controls">
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
        <div className="flowchat-header__turn-nav" ref={turnListRef}>
          <IconButton
            className={`flowchat-header__turn-nav-button${isTurnListOpen ? ' flowchat-header__turn-nav-button--active' : ''}`}
            variant="ghost"
            size="xs"
            onClick={handleToggleTurnList}
            tooltip={turnListTooltip}
            disabled={!hasTurnNavigation}
            aria-label={turnListTooltip}
            aria-expanded={isTurnListOpen}
            aria-haspopup="dialog"
            data-testid="flowchat-header-turn-list"
          >
            <List size={14} />
          </IconButton>
          <IconButton
            className="flowchat-header__turn-nav-button"
            variant="ghost"
            size="xs"
            onClick={onJumpToPreviousTurn}
            tooltip={t('flowChatHeader.previousTurn')}
            disabled={previousTurnDisabled || !onJumpToPreviousTurn}
            aria-label={t('flowChatHeader.previousTurn')}
            data-testid="flowchat-header-turn-prev"
          >
            <ChevronUp size={14} />
          </IconButton>
          <IconButton
            className="flowchat-header__turn-nav-button"
            variant="ghost"
            size="xs"
            onClick={onJumpToNextTurn}
            tooltip={t('flowChatHeader.nextTurn')}
            disabled={nextTurnDisabled || !onJumpToNextTurn}
            aria-label={t('flowChatHeader.nextTurn')}
            data-testid="flowchat-header-turn-next"
          >
            <ChevronDown size={14} />
          </IconButton>

          {isTurnListOpen && hasTurnNavigation && (
            <div className="flowchat-header__turn-list-panel" role="dialog" aria-label={turnListTooltip}>
              <div className="flowchat-header__turn-list-header">
                <span>{turnListTooltip}</span>
                <span>{currentTurn}/{totalTurns}</span>
              </div>
              <div className="flowchat-header__turn-list">
                {displayTurns.map(turn => (
                  <button
                    key={turn.turnId}
                    type="button"
                    className={`flowchat-header__turn-list-item${turn.turnIndex === currentTurn ? ' flowchat-header__turn-list-item--active' : ''}`}
                    onClick={() => handleTurnSelect(turn.turnId)}
                    ref={turn.turnIndex === currentTurn ? activeTurnItemRef : undefined}
                  >
                    <span className="flowchat-header__turn-list-badge">
                      {t('flowChatHeader.turnBadge', {
                        current: turn.turnIndex
                      })}
                    </span>
                    <span className="flowchat-header__turn-list-title">{turn.title}</span>
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

FlowChatHeader.displayName = 'FlowChatHeader';


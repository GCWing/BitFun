/**
 * FlowChat header.
 * Shows the workspace path and agent type for the active session.
 * Height matches side panel headers (40px).
 */

import React, { useEffect, useMemo, useRef, useState, useCallback } from 'react';
import { ChevronDown, ChevronUp, CornerUpLeft, List, FolderOpen, Bot, Orbit, Search, X } from 'lucide-react';
import { Tooltip, IconButton } from '@/component-library';
import { useTranslation } from 'react-i18next';
import { globalEventBus } from '@/infrastructure/event-bus';
import { SessionFilesBadge } from './SessionFilesBadge';
import type { Session } from '../../types/flow-chat';
import { FLOWCHAT_FOCUS_ITEM_EVENT, type FlowChatFocusItemRequest } from '../../events/flowchatNavigation';
import './FlowChatHeader.scss';

export interface FlowChatHeaderTurnSummary {
  turnId: string;
  turnIndex: number;
  title: string;
}

export interface FlowChatHeaderProps {
  /** Current turn index. */
  currentTurn: number;
  /** Total turns. */
  totalTurns: number;
  /** Current user message (kept for turn list tooltip). */
  currentUserMessage: string;
  /** Whether the header is visible. */
  visible: boolean;
  /** Session ID. */
  sessionId?: string;
  /** Workspace path displayed in the header. */
  workspacePath?: string;
  /** Agent type / mode for the active session. */
  agentType?: string;
  /** Session mode string (e.g. Dispatcher) for label/icon rules. */
  sessionMode?: string;
  /** BTW child-session origin metadata. */
  btwOrigin?: Session['btwOrigin'] | null;
  /** BTW parent session title. */
  btwParentTitle?: string;
  /** Ordered turn summaries used by header navigation. */
  turns?: FlowChatHeaderTurnSummary[];
  /** Jump to a specific turn. */
  onJumpToTurn?: (turnId: string) => void;
  /** Jump to the previous turn. */
  onJumpToPreviousTurn?: () => void;
  /** Jump to the next turn. */
  onJumpToNextTurn?: () => void;
  /** When set with handler, show left icon to switch to Agentic OS (Dispatcher). */
  showBackToAgenticOs?: boolean;
  onOpenAgenticOs?: () => void;

  // ========== Search ==========
  /** Current search query string. */
  searchQuery?: string;
  /** Called when the user types in the search box. */
  onSearchChange?: (query: string) => void;
  /** Total number of search matches. */
  searchMatchCount?: number;
  /** 1-based index of the currently focused match (0 means no active match). */
  searchCurrentMatch?: number;
  /** Navigate to the next match. */
  onSearchNext?: () => void;
  /** Navigate to the previous match. */
  onSearchPrev?: () => void;
  /** Called when the user closes the search bar. */
  onSearchClose?: () => void;
  /** Increments each time the parent requests to open the search bar (e.g. Ctrl+F). */
  searchOpenRequest?: number;
}
export const FlowChatHeader: React.FC<FlowChatHeaderProps> = ({
  currentTurn,
  totalTurns,
  currentUserMessage: _currentUserMessage,
  visible,
  sessionId,
  workspacePath,
  agentType,
  sessionMode,
  btwOrigin,
  btwParentTitle = '',
  turns = [],
  onJumpToTurn,
  onJumpToPreviousTurn,
  onJumpToNextTurn,
  showBackToAgenticOs,
  onOpenAgenticOs,
  searchQuery = '',
  onSearchChange,
  searchMatchCount = 0,
  searchCurrentMatch = 0,
  onSearchNext,
  onSearchPrev,
  onSearchClose,
  searchOpenRequest = 0,
}) => {
  const { t } = useTranslation('flow-chat');
  const [isTurnListOpen, setIsTurnListOpen] = useState(false);
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const turnListRef = useRef<HTMLDivElement | null>(null);
  const activeTurnItemRef = useRef<HTMLButtonElement | null>(null);
  const searchInputRef = useRef<HTMLInputElement | null>(null);

  const workspaceName = useMemo(() => {
    if (!workspacePath) return '';
    return workspacePath.replace(/\\/g, '/').split('/').filter(Boolean).pop() ?? workspacePath;
  }, [workspacePath]);

  const isDispatcherSession =
    sessionMode === 'Dispatcher' || sessionMode?.toLowerCase() === 'dispatcher';
  const displayAgentLabel = isDispatcherSession
    ? t('session.dispatcher')
    : agentType;
  const tooltipAgentLine = isDispatcherSession
    ? t('session.dispatcher')
    : agentType;
  const showAgentTypeIcon = !!displayAgentLabel && !isDispatcherSession;

  const parentLabel = btwParentTitle || t('btw.parent', { defaultValue: 'parent session' });
  const backTooltip = btwOrigin?.parentTurnIndex
    ? t('flowChatHeader.btwBackTooltipWithTurn', {
      title: parentLabel,
      turn: btwOrigin.parentTurnIndex,
      defaultValue: `Go back to the source session: ${parentLabel} (Turn ${btwOrigin.parentTurnIndex})`,
    })
    : t('flowChatHeader.btwBackTooltipWithoutTurn', {
      title: parentLabel,
      defaultValue: `Go back to the source session: ${parentLabel}`,
    });
  const turnListTooltip = t('flowChatHeader.turnList', {
    defaultValue: 'Turn list',
  });
  const untitledTurnLabel = t('flowChatHeader.untitledTurn', {
    defaultValue: 'Untitled turn',
  });
  const backToAgenticOsTooltip = t('flowChatHeader.backToAgenticOs', {
    defaultValue: 'Back to Agentic OS',
  });
  const previousTurnDisabled = currentTurn <= 1;
  const nextTurnDisabled = currentTurn <= 0 || currentTurn >= totalTurns;
  const hasTurnNavigation = turns.length > 0 && !!onJumpToTurn;
  const displayTurns = useMemo(() => (
    turns.map(turn => ({
      ...turn,
      title: turn.title.trim() || untitledTurnLabel,
    }))
  ), [turns, untitledTurnLabel]);

  useEffect(() => {
    if (!isTurnListOpen) return;

    const handlePointerDown = (event: MouseEvent) => {
      if (!turnListRef.current?.contains(event.target as Node)) {
        setIsTurnListOpen(false);
      }
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsTurnListOpen(false);
      }
    };

    document.addEventListener('mousedown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isTurnListOpen]);

  useEffect(() => {
    setIsTurnListOpen(false);
  }, [currentTurn]);

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

  // Sync open state from parent (e.g. Ctrl+F shortcut).
  // Using a counter so every new request opens the bar, even after a prior close.
  const prevSearchOpenRequestRef = useRef(0);
  useEffect(() => {
    if (searchOpenRequest > 0 && searchOpenRequest !== prevSearchOpenRequestRef.current) {
      prevSearchOpenRequestRef.current = searchOpenRequest;
      setIsSearchOpen(true);
    }
  }, [searchOpenRequest]);

  // Focus the search input whenever it opens.
  useEffect(() => {
    if (isSearchOpen) {
      const frameId = requestAnimationFrame(() => {
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      });
      return () => cancelAnimationFrame(frameId);
    }
    return undefined;
  }, [isSearchOpen]);

  // Close turn list when search opens.
  useEffect(() => {
    if (isSearchOpen) {
      setIsTurnListOpen(false);
    }
  }, [isSearchOpen]);

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
      } else if (e.key === 'Enter') {
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

  const hasNoResults = searchQuery.trim().length > 0 && searchMatchCount === 0;

  const handleBackToParent = () => {
    const parentId = btwOrigin?.parentSessionId;
    if (!parentId) return;
    const requestId = btwOrigin?.requestId;
    const itemId = requestId ? `btw_marker_${requestId}` : undefined;
    const request: FlowChatFocusItemRequest = {
      sessionId: parentId,
      turnIndex: btwOrigin?.parentTurnIndex,
      itemId,
      source: 'btw-back',
    };
    globalEventBus.emit(FLOWCHAT_FOCUS_ITEM_EVENT, request, 'FlowChatHeader');
  };

  const handleToggleTurnList = () => {
    if (!hasTurnNavigation) return;
    setIsTurnListOpen(prev => !prev);
  };

  const handleTurnSelect = (turnId: string) => {
    if (!onJumpToTurn) return;
    onJumpToTurn(turnId);
    setIsTurnListOpen(false);
  };

  if (!visible) {
    return null;
  }

  return (
    <div className={`flowchat-header${isSearchOpen ? ' flowchat-header--search-open' : ''}`}>
      <div className="flowchat-header__actions flowchat-header__actions--left">
        {showBackToAgenticOs && onOpenAgenticOs ? (
          <IconButton
            className="flowchat-header__agentic-os-back"
            variant="ghost"
            size="xs"
            onClick={onOpenAgenticOs}
            tooltip={backToAgenticOsTooltip}
            aria-label={backToAgenticOsTooltip}
            data-testid="flowchat-header-agentic-os-back"
          >
            <Orbit size={14} />
          </IconButton>
        ) : null}
        <SessionFilesBadge sessionId={sessionId} />
      </div>

      {isSearchOpen ? (
        <div className="flowchat-header__search" role="search">
          <Search size={13} className="flowchat-header__search-icon" aria-hidden="true" />
          <input
            ref={searchInputRef}
            className={`flowchat-header__search-input${hasNoResults ? ' flowchat-header__search-input--no-results' : ''}`}
            type="text"
            value={searchQuery}
            onChange={e => onSearchChange?.(e.target.value)}
            onKeyDown={handleSearchKeyDown}
            placeholder={t('flowChatHeader.searchPlaceholder', { defaultValue: 'Search messages' })}
            aria-label={t('flowChatHeader.searchPlaceholder', { defaultValue: 'Search messages' })}
          />
          <span className="flowchat-header__search-count" aria-live="polite">
            {searchQuery.trim()
              ? hasNoResults
                ? t('flowChatHeader.searchNoResults', { defaultValue: 'No results' })
                : t('flowChatHeader.searchResult', {
                    current: searchCurrentMatch,
                    total: searchMatchCount,
                    defaultValue: `${searchCurrentMatch} / ${searchMatchCount}`,
                  })
              : null}
          </span>
          <IconButton
            variant="ghost"
            size="xs"
            onClick={onSearchPrev}
            disabled={searchMatchCount === 0}
            tooltip={t('flowChatHeader.searchPrevious', { defaultValue: 'Previous match' })}
            aria-label={t('flowChatHeader.searchPrevious', { defaultValue: 'Previous match' })}
          >
            <ChevronUp size={14} />
          </IconButton>
          <IconButton
            variant="ghost"
            size="xs"
            onClick={onSearchNext}
            disabled={searchMatchCount === 0}
            tooltip={t('flowChatHeader.searchNext', { defaultValue: 'Next match' })}
            aria-label={t('flowChatHeader.searchNext', { defaultValue: 'Next match' })}
          >
            <ChevronDown size={14} />
          </IconButton>
          <IconButton
            variant="ghost"
            size="xs"
            onClick={handleCloseSearch}
            tooltip={t('flowChatHeader.searchClose', { defaultValue: 'Close search' })}
            aria-label={t('flowChatHeader.searchClose', { defaultValue: 'Close search' })}
          >
            <X size={14} />
          </IconButton>
        </div>
      ) : (
        <Tooltip
          content={[tooltipAgentLine, workspacePath].filter(Boolean).join(': ')}
          placement="bottom"
        >
          <div className="flowchat-header__info">
            {displayAgentLabel ? (
              <span className={`flowchat-header__agent-type${isDispatcherSession ? ' flowchat-header__agent-type--plain' : ''}`}>
                {showAgentTypeIcon ? <Bot size={11} /> : null}
                <span>{displayAgentLabel}</span>
              </span>
            ) : null}
            {displayAgentLabel && workspaceName ? (
              <span className="flowchat-header__info-sep">/</span>
            ) : null}
            {workspaceName ? (
              <span className="flowchat-header__workspace">
                <FolderOpen size={12} />
                <span>{workspaceName}</span>
              </span>
            ) : null}
          </div>
        </Tooltip>
      )}

      <div className="flowchat-header__actions">
        {!isSearchOpen && (
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
              tooltip={t('flowChatHeader.previousTurn', { defaultValue: 'Previous turn' })}
              disabled={previousTurnDisabled || !onJumpToPreviousTurn}
              aria-label={t('flowChatHeader.previousTurn', { defaultValue: 'Previous turn' })}
              data-testid="flowchat-header-turn-prev"
            >
              <ChevronUp size={14} />
            </IconButton>
            <IconButton
              className="flowchat-header__turn-nav-button"
              variant="ghost"
              size="xs"
              onClick={onJumpToNextTurn}
              tooltip={t('flowChatHeader.nextTurn', { defaultValue: 'Next turn' })}
              disabled={nextTurnDisabled || !onJumpToNextTurn}
              aria-label={t('flowChatHeader.nextTurn', { defaultValue: 'Next turn' })}
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
                          current: turn.turnIndex,
                          defaultValue: `Turn ${turn.turnIndex}`,
                        })}
                      </span>
                      <span className="flowchat-header__turn-list-title">{turn.title}</span>
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
        {!isSearchOpen && (
          <IconButton
            className="flowchat-header__search-btn"
            variant="ghost"
            size="xs"
            onClick={handleOpenSearch}
            tooltip={t('flowChatHeader.searchOpen', { defaultValue: 'Search messages' })}
            aria-label={t('flowChatHeader.searchOpen', { defaultValue: 'Search messages' })}
            data-testid="flowchat-header-search"
          >
            <Search size={14} />
          </IconButton>
        )}
        {!!btwOrigin?.parentSessionId && (
          <IconButton
            className="flowchat-header__btw-back"
            variant="ghost"
            size="xs"
            onClick={handleBackToParent}
            tooltip={backTooltip}
            disabled={!btwOrigin.parentSessionId}
            aria-label={t('btw.back', { defaultValue: 'Back' })}
            data-testid="flowchat-header-btw-back"
          >
            <CornerUpLeft size={12} />
          </IconButton>
        )}
      </div>
    </div>
  );
};

FlowChatHeader.displayName = 'FlowChatHeader';


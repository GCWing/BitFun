/**
 * FlowChat header.
 * Shows the currently viewed turn and user message.
 * Height matches side panel headers (40px).
 */

import React, { useEffect, useMemo, useRef, useState, useCallback } from 'react';
import { ChevronDown, ChevronUp, List, Search, X } from 'lucide-react';
import { Tooltip, IconButton, Input } from '@/component-library';
import { useTranslation } from 'react-i18next';
import { SessionFilesBadge } from './SessionFilesBadge';
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

  // Truncate long messages.
  const truncatedMessage = currentUserMessage.length > 50
    ? currentUserMessage.slice(0, 50) + '...'
    : currentUserMessage;
  const turnListTooltip = t('flowChatHeader.turnList', {
    defaultValue: 'Turn list',
  });
  const untitledTurnLabel = t('flowChatHeader.untitledTurn', {
    defaultValue: 'Untitled turn',
  });
  const turnBadgeLabel = t('flowChatHeader.turnBadge', {
    current: currentTurn,
    defaultValue: `Turn ${currentTurn}`,
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
  const hasNoResults = searchQuery.trim().length > 0 && searchMatchCount === 0;

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
    setIsTurnListOpen(prev => !prev);
  };

  const handleTurnSelect = (turnId: string) => {
    if (!onJumpToTurn) return;
    onJumpToTurn(turnId);
    setIsTurnListOpen(false);
  };

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
            turn: currentTurn,
            defaultValue: `Jump to Turn ${currentTurn}`,
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
                        ? t('flowChatHeader.searchNoResults', { defaultValue: 'No results' })
                        : t('flowChatHeader.searchResult', {
                          current: searchCurrentMatch,
                          total: searchMatchCount,
                          defaultValue: `${searchCurrentMatch} / ${searchMatchCount}`,
                        })
                      : null}
                  </span>
                  <span className="flowchat-header__search-nav">
                    <button
                      className="flowchat-header__search-nav-btn"
                      onClick={onSearchPrev}
                      disabled={searchMatchCount === 0}
                      title={t('flowChatHeader.searchPrevious', { defaultValue: 'Previous match' })}
                      aria-label={t('flowChatHeader.searchPrevious', { defaultValue: 'Previous match' })}
                      type="button"
                    >
                      <ChevronUp size={10} />
                    </button>
                    <button
                      className="flowchat-header__search-nav-btn"
                      onClick={onSearchNext}
                      disabled={searchMatchCount === 0}
                      title={t('flowChatHeader.searchNext', { defaultValue: 'Next match' })}
                      aria-label={t('flowChatHeader.searchNext', { defaultValue: 'Next match' })}
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
              placeholder={t('flowChatHeader.searchPlaceholder', { defaultValue: 'Search messages' })}
              aria-label={t('flowChatHeader.searchPlaceholder', { defaultValue: 'Search messages' })}
              error={hasNoResults}
            />
            <IconButton
              className="flowchat-header__search-close"
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
      </div>
    </div>
  );
};

FlowChatHeader.displayName = 'FlowChatHeader';


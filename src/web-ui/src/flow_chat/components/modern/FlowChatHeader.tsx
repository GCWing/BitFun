/**
 * FlowChat header — message search (when turn list is collapsed), turn list toggle, BTW back.
 *
 * The session title and the "return to Agentic OS" button have been moved to
 * UnifiedTopBar so the whole application shares a single top chrome.
 * This component now owns only the right-side session controls.
 */

import React, { useEffect, useRef, useState, useCallback } from 'react';
import { ChevronDown, ChevronUp, CornerUpLeft, List, Search, X } from 'lucide-react';
import { IconButton, Input } from '@/component-library';
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
  /** Whether the header is visible. */
  visible: boolean;
  /** Session ID. */
  sessionId?: string;
  /** BTW child-session origin metadata. */
  btwOrigin?: Session['btwOrigin'] | null;
  /** BTW parent session title. */
  btwParentTitle?: string;
  /** Ordered turn summaries used by header navigation. */
  turns?: FlowChatHeaderTurnSummary[];
  /** Jump to a specific turn (used by turn list sidebar). */
  onJumpToTurn?: (turnId: string) => void;

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

  /** Turn list sidebar open state (controlled by parent). */
  turnListOpen?: boolean;
  /** Toggle or close the turn list sidebar. */
  onTurnListOpenChange?: (open: boolean) => void;
}
export const FlowChatHeader: React.FC<FlowChatHeaderProps> = ({
  visible,
  sessionId,
  btwOrigin,
  btwParentTitle = '',
  turns = [],
  onJumpToTurn,
  searchQuery = '',
  onSearchChange,
  searchMatchCount = 0,
  searchCurrentMatch = 0,
  onSearchNext,
  onSearchPrev,
  onSearchClose,
  searchOpenRequest = 0,
  turnListOpen = false,
  onTurnListOpenChange,
}) => {
  const { t } = useTranslation('flow-chat');
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const searchInputRef = useRef<HTMLInputElement | null>(null);

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
  const hasTurnNavigation = turns.length > 0 && !!onJumpToTurn;

  // When collapsing the turn list with an active query, reopen the header search bar.
  const prevTurnListOpenRef = useRef(turnListOpen);
  useEffect(() => {
    if (prevTurnListOpenRef.current && !turnListOpen && searchQuery.trim().length > 0) {
      setIsSearchOpen(true);
    }
    prevTurnListOpenRef.current = turnListOpen;
  }, [turnListOpen, searchQuery]);

  // Sync open state from parent (e.g. Ctrl+F shortcut).
  // Using a counter so every new request opens the bar, even after a prior close.
  const prevSearchOpenRequestRef = useRef(0);
  useEffect(() => {
    if (turnListOpen) return;
    if (searchOpenRequest > 0 && searchOpenRequest !== prevSearchOpenRequestRef.current) {
      prevSearchOpenRequestRef.current = searchOpenRequest;
      setIsSearchOpen(true);
    }
  }, [searchOpenRequest, turnListOpen]);

  // Focus the search input whenever it opens (header search is hidden while turn list is open).
  useEffect(() => {
    if (turnListOpen || !isSearchOpen) return undefined;
    const frameId = requestAnimationFrame(() => {
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    });
    return () => cancelAnimationFrame(frameId);
  }, [isSearchOpen, turnListOpen]);

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
    onTurnListOpenChange?.(!turnListOpen);
  };

  if (!visible) {
    return null;
  }

  return (
    <div className="flowchat-header">
      <div className="flowchat-header__actions flowchat-header__actions--left">
        <SessionFilesBadge sessionId={sessionId} />
      </div>

      <div className="flowchat-header__actions">
        {!turnListOpen && isSearchOpen ? (
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
              variant="ghost"
              size="xs"
              onClick={handleCloseSearch}
              tooltip={t('flowChatHeader.searchClose', { defaultValue: 'Close search' })}
              aria-label={t('flowChatHeader.searchClose', { defaultValue: 'Close search' })}
            >
              <X size={14} />
            </IconButton>
          </div>
        ) : null}
        {!turnListOpen && !isSearchOpen && (
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
        <div className="flowchat-header__turn-nav">
          <IconButton
            className={`flowchat-header__turn-nav-button${turnListOpen ? ' flowchat-header__turn-nav-button--active' : ''}`}
            variant="ghost"
            size="xs"
            onClick={handleToggleTurnList}
            tooltip={turnListTooltip}
            disabled={!hasTurnNavigation}
            aria-label={turnListTooltip}
            aria-expanded={turnListOpen}
            aria-controls="flowchat-turn-list-sidebar"
            data-testid="flowchat-header-turn-list"
          >
            <List size={14} />
          </IconButton>
        </div>
      </div>
    </div>
  );
};

FlowChatHeader.displayName = 'FlowChatHeader';


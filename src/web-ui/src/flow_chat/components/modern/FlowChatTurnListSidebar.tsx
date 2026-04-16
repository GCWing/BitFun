/**
 * Right-side turn list panel — pushes the message area when open.
 */

import React, { useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronUp, Search, X } from 'lucide-react';
import { IconButton, Input } from '@/component-library';
import './FlowChatTurnListSidebar.scss';

export interface FlowChatTurnListEntry {
  turnId: string;
  turnIndex: number;
  title: string;
}

export interface FlowChatTurnListSidebarProps {
  open: boolean;
  turns: FlowChatTurnListEntry[];
  currentTurn: number;
  totalTurns: number;
  onSelectTurn: (turnId: string) => void;
  /** Dialog turns that contain the active search query (whole-turn search). */
  searchMatchedTurnIds?: ReadonlySet<string>;

  // Message search (shown here while the panel is open; header search is hidden)
  searchQuery?: string;
  onSearchChange?: (query: string) => void;
  searchMatchCount?: number;
  searchCurrentMatch?: number;
  onSearchNext?: () => void;
  onSearchPrev?: () => void;
  onSearchClose?: () => void;
  /** Increments when parent requests focus (e.g. Ctrl+F while panel is open). */
  searchFocusRequest?: number;
}

export const FlowChatTurnListSidebar = React.forwardRef<HTMLElement, FlowChatTurnListSidebarProps>(
  function FlowChatTurnListSidebar(
    {
      open,
      turns,
      currentTurn,
      totalTurns,
      onSelectTurn,
      searchMatchedTurnIds,
      searchQuery = '',
      onSearchChange,
      searchMatchCount = 0,
      searchCurrentMatch = 0,
      onSearchNext,
      onSearchPrev,
      onSearchClose,
      searchFocusRequest = 0,
    },
    ref,
  ) {
    const { t } = useTranslation('flow-chat');
    const activeTurnItemRef = useRef<HTMLButtonElement | null>(null);
    const searchInputRef = useRef<HTMLInputElement | null>(null);
    const listTitle = t('flowChatHeader.turnList', { defaultValue: 'Turn list' });

    const prevFocusRequestRef = useRef(0);
    useEffect(() => {
      if (!open) return;
      if (searchFocusRequest > 0 && searchFocusRequest !== prevFocusRequestRef.current) {
        prevFocusRequestRef.current = searchFocusRequest;
        const frameId = requestAnimationFrame(() => {
          searchInputRef.current?.focus();
          searchInputRef.current?.select();
        });
        return () => cancelAnimationFrame(frameId);
      }
      return undefined;
    }, [open, searchFocusRequest]);

    useEffect(() => {
      if (!open) return;

      const frameId = requestAnimationFrame(() => {
        activeTurnItemRef.current?.scrollIntoView({
          block: 'center',
          inline: 'nearest',
        });
      });

      return () => {
        cancelAnimationFrame(frameId);
      };
    }, [open, currentTurn, turns.length]);

    const handleSearchKeyDown = useCallback(
      (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Escape') {
          onSearchClose?.();
        } else if (e.key === 'Enter') {
          if (e.shiftKey) {
            onSearchPrev?.();
          } else {
            onSearchNext?.();
          }
          e.preventDefault();
        }
      },
      [onSearchClose, onSearchNext, onSearchPrev],
    );

    const hasNoResults = searchQuery.trim().length > 0 && searchMatchCount === 0;

    return (
      <aside
        id="flowchat-turn-list-sidebar"
        ref={ref}
        className={`flowchat-turn-sidebar${open ? ' flowchat-turn-sidebar--open' : ''}`}
        aria-hidden={!open}
        data-testid="flowchat-turn-list-sidebar"
      >
        <div className="flowchat-turn-sidebar__inner">
          <div className="flowchat-turn-sidebar__header">
            <div className="flowchat-turn-sidebar__heading">
              <span className="flowchat-turn-sidebar__heading-text">{listTitle}</span>
              <span className="flowchat-turn-sidebar__counter">
                {currentTurn}/{totalTurns}
              </span>
            </div>
            {open ? (
              <div className="flowchat-turn-sidebar__search" role="search" data-testid="flowchat-turn-list-search-bar">
                <Input
                  ref={searchInputRef}
                  className="flowchat-turn-sidebar__search-field"
                  variant="filled"
                  inputSize="small"
                  prefix={<Search size={12} className="flowchat-turn-sidebar__search-prefix-icon" aria-hidden="true" />}
                  type="text"
                  value={searchQuery}
                  onChange={e => onSearchChange?.(e.target.value)}
                  onKeyDown={handleSearchKeyDown}
                  placeholder={t('flowChatHeader.searchPlaceholder', { defaultValue: 'Search messages' })}
                  aria-label={t('flowChatHeader.searchPlaceholder', { defaultValue: 'Search messages' })}
                  error={hasNoResults}
                />
                <span className="flowchat-turn-sidebar__search-count" aria-live="polite">
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
                  onClick={onSearchClose}
                  tooltip={t('flowChatHeader.searchClose', { defaultValue: 'Close search' })}
                  aria-label={t('flowChatHeader.searchClose', { defaultValue: 'Close search' })}
                >
                  <X size={14} />
                </IconButton>
              </div>
            ) : null}
          </div>
          <div className="flowchat-turn-sidebar__list" role="list">
            {turns.map(turn => (
              <button
                key={turn.turnId}
                type="button"
                role="listitem"
                className={`flowchat-turn-sidebar__item${
                  turn.turnIndex === currentTurn ? ' flowchat-turn-sidebar__item--active' : ''
                }${
                  searchMatchedTurnIds?.has(turn.turnId)
                    ? ' flowchat-turn-sidebar__item--search-match'
                    : ''
                }`}
                onClick={() => onSelectTurn(turn.turnId)}
                ref={turn.turnIndex === currentTurn ? activeTurnItemRef : undefined}
              >
                <span className="flowchat-turn-sidebar__badge">
                  {t('flowChatHeader.turnBadge', {
                    current: turn.turnIndex,
                    defaultValue: `Turn ${turn.turnIndex}`,
                  })}
                </span>
                <span className="flowchat-turn-sidebar__title">{turn.title}</span>
              </button>
            ))}
          </div>
        </div>
      </aside>
    );
  },
);

FlowChatTurnListSidebar.displayName = 'FlowChatTurnListSidebar';

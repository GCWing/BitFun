/**
 * FlowChat message search hook.
 * Searches user + model text, deduplicated by dialog turn: one match per turn,
 * navigation moves between turns that contain the query.
 */

import { useState, useMemo, useCallback } from 'react';
import type { VirtualItem } from '../../store/modernFlowChatStore';

interface SearchableFlowItem {
  type: string;
  content?: string;
}

export interface SearchMatch {
  /** Smallest virtual index in this turn where text matched (scroll / “current” anchor). */
  virtualItemIndex: number;
  turnId: string;
  type: VirtualItem['type'];
}

export interface UseFlowChatSearchReturn {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  matches: SearchMatch[];
  matchIndices: ReadonlySet<number>;
  currentMatchIndex: number;
  currentMatchVirtualIndex: number;
  goToNext: () => void;
  goToPrev: () => void;
  clearSearch: () => void;
}

function extractSearchableText(items: readonly SearchableFlowItem[]): string {
  return items
    .filter(item => item.type === 'text' || item.type === 'thinking')
    .map(item => item.content ?? '')
    .join(' ');
}

function getVirtualItemSearchText(item: VirtualItem): string {
  if (item.type === 'user-message') {
    return item.data?.content ?? '';
  }
  if (item.type === 'model-round') {
    return extractSearchableText(item.data.items);
  }
  if (item.type === 'explore-group') {
    return extractSearchableText(item.data.allItems);
  }
  return '';
}

export function useFlowChatSearch(virtualItems: VirtualItem[]): UseFlowChatSearchReturn {
  const [searchQuery, setSearchQuery] = useState('');
  const [currentMatchIndex, setCurrentMatchIndex] = useState(0);

  const matches = useMemo<SearchMatch[]>(() => {
    const trimmed = searchQuery.trim();
    if (!trimmed) return [];
    const q = trimmed.toLowerCase();

    const minIndexByTurn = new Map<string, number>();

    virtualItems.forEach((item, index) => {
      const text = getVirtualItemSearchText(item);
      if (!text.toLowerCase().includes(q)) return;

      const turnId = item.turnId;
      const prev = minIndexByTurn.get(turnId);
      if (prev === undefined || index < prev) {
        minIndexByTurn.set(turnId, index);
      }
    });

    const ordered = [...minIndexByTurn.entries()].sort((a, b) => a[1] - b[1]);

    return ordered.map(([turnId, virtualItemIndex]) => {
      const item = virtualItems[virtualItemIndex];
      return {
        virtualItemIndex,
        turnId,
        type: item.type,
      };
    });
  }, [virtualItems, searchQuery]);

  const matchIndices = useMemo<ReadonlySet<number>>(() => {
    if (matches.length === 0) return new Set();
    const matchedTurnIds = new Set(matches.map(m => m.turnId));
    const indices = new Set<number>();
    virtualItems.forEach((item, index) => {
      if (matchedTurnIds.has(item.turnId)) {
        indices.add(index);
      }
    });
    return indices;
  }, [virtualItems, matches]);

  const currentMatchVirtualIndex = matches[currentMatchIndex]?.virtualItemIndex ?? -1;

  const onSearchChange = useCallback((query: string) => {
    setSearchQuery(query);
    setCurrentMatchIndex(0);
  }, []);

  const goToNext = useCallback(() => {
    if (matches.length === 0) return;
    setCurrentMatchIndex(prev => (prev + 1) % matches.length);
  }, [matches.length]);

  const goToPrev = useCallback(() => {
    if (matches.length === 0) return;
    setCurrentMatchIndex(prev => (prev - 1 + matches.length) % matches.length);
  }, [matches.length]);

  const clearSearch = useCallback(() => {
    setSearchQuery('');
    setCurrentMatchIndex(0);
  }, []);

  return {
    searchQuery,
    onSearchChange,
    matches,
    matchIndices,
    currentMatchIndex,
    currentMatchVirtualIndex,
    goToNext,
    goToPrev,
    clearSearch,
  };
}

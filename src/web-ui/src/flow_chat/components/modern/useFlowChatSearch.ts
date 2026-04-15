/**
 * FlowChat message search hook.
 * Searches user messages and model output text across the virtual item list.
 */

import { useState, useMemo, useCallback } from 'react';
import type { VirtualItem } from '../../store/modernFlowChatStore';
import type { AnyFlowItem } from '../../types/flow-chat';

export interface SearchMatch {
  virtualItemIndex: number;
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

function extractSearchableText(items: readonly AnyFlowItem[]): string {
  return items
    .filter(item => item.type === 'text' || item.type === 'thinking')
    .map(item => (item as { content?: string }).content ?? '')
    .join(' ');
}

export function useFlowChatSearch(virtualItems: VirtualItem[]): UseFlowChatSearchReturn {
  const [searchQuery, setSearchQuery] = useState('');
  const [currentMatchIndex, setCurrentMatchIndex] = useState(0);

  const matches = useMemo<SearchMatch[]>(() => {
    const trimmed = searchQuery.trim();
    if (!trimmed) return [];
    const q = trimmed.toLowerCase();

    return virtualItems.reduce<SearchMatch[]>((acc, item, index) => {
      let text = '';

      if (item.type === 'user-message') {
        text = item.data?.content ?? '';
      } else if (item.type === 'model-round') {
        text = extractSearchableText(item.data.items);
      } else if (item.type === 'explore-group') {
        text = extractSearchableText(item.data.allItems);
      }

      if (text.toLowerCase().includes(q)) {
        acc.push({ virtualItemIndex: index, type: item.type });
      }

      return acc;
    }, []);
  }, [virtualItems, searchQuery]);

  const matchIndices = useMemo<ReadonlySet<number>>(
    () => new Set(matches.map(m => m.virtualItemIndex)),
    [matches],
  );

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

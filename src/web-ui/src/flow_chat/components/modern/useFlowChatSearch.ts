/**
 * FlowChat message search hook.
 * Searches user + model text, deduplicated by dialog turn: one match per turn.
 * Each match also keeps the concrete rendered source so navigation can land on
 * the matching text instead of only centering a potentially very tall turn.
 */

import { useState, useMemo, useCallback } from 'react';
import type { VirtualItem } from '../../store/modernFlowChatStore';

interface SearchableFlowItem {
  id?: string;
  type: string;
  content?: string;
}

export interface SearchMatch {
  /** Smallest virtual index in this turn where text matched. */
  virtualItemIndex: number;
  turnId: string;
  type: VirtualItem['type'];
  /** Rendered FlowItem containing the first match in this turn, when applicable. */
  flowItemId?: string;
  /** Collapsible containers that must be opened, from outermost to innermost. */
  expandableIds?: readonly string[];
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

interface SearchableSource {
  content: string;
  flowItemId?: string;
  expandableIds?: readonly string[];
}

function flowItemSearchSources(
  items: readonly SearchableFlowItem[],
  outerExpandableId?: string,
): SearchableSource[] {
  return items.flatMap(item => {
    if (item.type !== 'text' && item.type !== 'thinking') {
      return [];
    }

    const expandableIds = [
      ...(outerExpandableId ? [outerExpandableId] : []),
      ...(item.type === 'thinking' && item.id ? [item.id] : []),
    ];

    return [{
      content: item.content ?? '',
      flowItemId: item.id,
      expandableIds: expandableIds.length > 0 ? expandableIds : undefined,
    }];
  });
}

function getVirtualItemSearchSources(item: VirtualItem): SearchableSource[] {
  if (item.type === 'user-message' || item.type === 'user-steering-message') {
    return [{ content: item.data?.content ?? '' }];
  }
  if (item.type === 'model-round') {
    return flowItemSearchSources(item.data.items);
  }
  if (item.type === 'explore-group') {
    return flowItemSearchSources(item.data.allItems, item.data.groupId);
  }
  if (item.type === 'turn-completion-notice') {
    return [{ content: item.data.reasonCode }];
  }
  if (item.type === 'turn-failure-notice') {
    return [{
      content: [
        item.data.error,
        item.data.errorDetail?.provider,
        item.data.errorDetail?.providerCode,
        item.data.errorDetail?.providerMessage,
        item.data.errorDetail?.requestId,
      ].filter(Boolean).join(' '),
    }];
  }
  return [];
}

export function buildFlowChatSearchMatches(
  virtualItems: readonly VirtualItem[],
  searchQuery: string,
): SearchMatch[] {
  const trimmed = searchQuery.trim();
  if (!trimmed) return [];
  const query = trimmed.toLowerCase();
  const firstMatchByTurn = new Map<string, SearchMatch>();

  virtualItems.forEach((item, virtualItemIndex) => {
    if (firstMatchByTurn.has(item.turnId)) {
      return;
    }

    const source = getVirtualItemSearchSources(item).find(candidate => (
      candidate.content.toLowerCase().includes(query)
    ));
    if (!source) {
      return;
    }

    firstMatchByTurn.set(item.turnId, {
      virtualItemIndex,
      turnId: item.turnId,
      type: item.type,
      flowItemId: source.flowItemId,
      expandableIds: source.expandableIds,
    });
  });

  return [...firstMatchByTurn.values()]
    .sort((left, right) => left.virtualItemIndex - right.virtualItemIndex);
}

export function useFlowChatSearch(virtualItems: VirtualItem[]): UseFlowChatSearchReturn {
  const [searchQuery, setSearchQuery] = useState('');
  const [currentMatchIndex, setCurrentMatchIndex] = useState(0);

  const matches = useMemo<SearchMatch[]>(() => (
    buildFlowChatSearchMatches(virtualItems, searchQuery)
  ), [virtualItems, searchQuery]);

  const resolvedCurrentMatchIndex = matches.length > 0
    ? Math.min(currentMatchIndex, matches.length - 1)
    : 0;

  const matchIndices = useMemo<ReadonlySet<number>>(() => {
    if (matches.length === 0) return new Set();
    const matchedTurnIds = new Set(matches.map(match => match.turnId));
    const indices = new Set<number>();
    virtualItems.forEach((item, index) => {
      if (matchedTurnIds.has(item.turnId)) {
        indices.add(index);
      }
    });
    return indices;
  }, [virtualItems, matches]);

  const currentMatchVirtualIndex = matches[resolvedCurrentMatchIndex]?.virtualItemIndex ?? -1;

  const onSearchChange = useCallback((query: string) => {
    setSearchQuery(query);
    setCurrentMatchIndex(0);
  }, []);

  const goToNext = useCallback(() => {
    if (matches.length === 0) return;
    setCurrentMatchIndex(prev => {
      const current = Math.min(prev, matches.length - 1);
      return (current + 1) % matches.length;
    });
  }, [matches.length]);

  const goToPrev = useCallback(() => {
    if (matches.length === 0) return;
    setCurrentMatchIndex(prev => {
      const current = Math.min(prev, matches.length - 1);
      return (current - 1 + matches.length) % matches.length;
    });
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
    currentMatchIndex: resolvedCurrentMatchIndex,
    currentMatchVirtualIndex,
    goToNext,
    goToPrev,
    clearSearch,
  };
}

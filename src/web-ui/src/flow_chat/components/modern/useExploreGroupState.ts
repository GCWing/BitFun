/**
 * Explore-group expansion state for Modern FlowChat.
 */

import { useCallback, useRef, useState } from 'react';
import type { VirtualItem } from '../../store/modernFlowChatStore';

type ExploreGroupVirtualItem = Extract<VirtualItem, { type: 'explore-group' }>;

interface UseExploreGroupStateResult {
  /**
   * Expanded/collapsed state for each explore group.
   * key: groupId, value: true means expanded.
   */
  exploreGroupStates: Map<string, boolean>;
  onExploreGroupToggle: (groupId: string) => void;
  onExpandGroup: (groupId: string) => void;
  onExpandAllInTurn: (turnId: string) => void;
  onCollapseGroup: (groupId: string) => void;
}

export function useExploreGroupState(
  virtualItems: VirtualItem[],
): UseExploreGroupStateResult {
  const [exploreGroupStates, setExploreGroupStates] = useState<Map<string, boolean>>(new Map());
  const virtualItemsRef = useRef(virtualItems);
  virtualItemsRef.current = virtualItems;

  const onExploreGroupToggle = useCallback((groupId: string) => {
    setExploreGroupStates(prev => {
      const next = new Map(prev);
      const currentExpanded = prev.get(groupId) ?? false;
      next.set(groupId, !currentExpanded);
      return next;
    });
  }, []);

  const onExpandGroup = useCallback((groupId: string) => {
    setExploreGroupStates(prev => {
      if (prev.get(groupId) === true) {
        return prev;
      }
      const next = new Map(prev);
      next.set(groupId, true);
      return next;
    });
  }, []);

  const onExpandAllInTurn = useCallback((turnId: string) => {
    const groupIds = virtualItemsRef.current
      .filter((item): item is ExploreGroupVirtualItem => (
        item.type === 'explore-group' && item.turnId === turnId
      ))
      .map(item => item.data.groupId);

    setExploreGroupStates(prev => {
      const next = new Map(prev);
      [...new Set(groupIds)].forEach(id => next.set(id, true));
      return next;
    });
  }, []);

  const onCollapseGroup = useCallback((groupId: string) => {
    setExploreGroupStates(prev => {
      const next = new Map(prev);
      next.set(groupId, false);
      return next;
    });
  }, []);

  return {
    exploreGroupStates,
    onExploreGroupToggle,
    onExpandGroup,
    onExpandAllInTurn,
    onCollapseGroup,
  };
}

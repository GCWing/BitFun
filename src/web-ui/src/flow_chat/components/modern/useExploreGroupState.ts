/**
 * Explore-group expansion state for Modern FlowChat.
 */

import { useCallback, useState } from 'react';
import type { VirtualItem } from '../../store/modernFlowChatStore';

type ExploreGroupVirtualItem = Extract<VirtualItem, { type: 'explore-group' }>;

interface UseExploreGroupStateResult {
  /**
   * Expanded/collapsed state for each explore group.
   * key: groupId, value: true means expanded.
   */
  exploreGroupStates: Map<string, boolean>;
  onExploreGroupToggle: (groupId: string) => void;
  onExpandAllInTurn: (turnId: string) => void;
  onCollapseGroup: (groupId: string) => void;
}

export function useExploreGroupState(
  virtualItems: VirtualItem[],
): UseExploreGroupStateResult {
  const [exploreGroupStates, setExploreGroupStates] = useState<Map<string, boolean>>(new Map());

  const onExploreGroupToggle = useCallback((groupId: string) => {
    setExploreGroupStates(prev => {
      const next = new Map(prev);
      const currentExpanded = prev.get(groupId) ?? false;
      next.set(groupId, !currentExpanded);
      return next;
    });
  }, []);

  const onExpandAllInTurn = useCallback((turnId: string) => {
    const groupIds = virtualItems
      .filter((item): item is ExploreGroupVirtualItem => (
        item.type === 'explore-group' && item.turnId === turnId
      ))
      .map(item => item.data.groupId);

    setExploreGroupStates(prev => {
      const next = new Map(prev);
      [...new Set(groupIds)].forEach(id => next.set(id, true));
      return next;
    });
  }, [virtualItems]);

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
    onExpandAllInTurn,
    onCollapseGroup,
  };
}

/**
 * Modern FlowChat Store
 * High-performance state management using Zustand + Immer
 * Preserves original concept: Session → DialogTurn → ModelRound → FlowItem
 */

import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import type { Session, DialogTurn, ModelRound, FlowToolItem } from '../types/flow-chat';
import { isCollapsibleTool, READ_TOOL_NAMES, SEARCH_TOOL_NAMES } from '../tool-cards';

/**
 * Explore group statistics (merged computed stats)
 */
export interface ExploreGroupStats {
  readCount: number;
  searchCount: number;
  thinkingCount: number;
}

/**
 * Explore group data (for explore-group type VirtualItem)
 * Merges consecutive explore-only rounds into a single render unit
 */
export interface ExploreGroupData {
  groupId: string;
  rounds: ModelRound[];
  allItems: import('../types/flow-chat').FlowItem[];
  stats: ExploreGroupStats;
  isGroupStreaming: boolean;
  isLastGroupInTurn: boolean;
  isFollowedByCritical: boolean;
}

/**
 * Virtualized render unit
 * Used for virtual scrolling, flattens DialogTurn into renderable items
 */
export type VirtualItem =
  | { type: 'user-message'; data: DialogTurn['userMessage']; turnId: string }
  | { type: 'model-round'; data: ModelRound; turnId: string; isLastRound: boolean }
  | { type: 'explore-group'; data: ExploreGroupData; turnId: string }
  | { type: 'image-analyzing'; turnId: string };

/**
 * Currently visible turn information
 */
export interface VisibleTurnInfo {
  turnIndex: number;
  totalTurns: number;
  userMessage: string;
  turnId: string;
}

interface ModernFlowChatState {
  activeSession: Session | null;
  virtualItems: VirtualItem[];
  visibleTurnInfo: VisibleTurnInfo | null;

  setActiveSession: (session: Session | null) => void;
  updateVirtualItems: () => void;
  setVisibleTurnInfo: (info: VisibleTurnInfo | null) => void;
  clear: () => void;
}

/**
 * Check if ModelRound is explore-only (contains only exploration tools)
 * Explore-only rounds can be collapsed
 * 
 * Key check: must contain at least one collapsible tool OR be a pure thinking round.
 * Pure thinking rounds (thinking without critical tools) are merged into
 * adjacent explore groups to reduce visual noise from standalone "thinking N chars" lines.
 * Pure text rounds (like final replies) should not be collapsed.
 */
function isExploreOnlyRound(round: ModelRound): boolean {
  if (!round.items || round.items.length === 0) return false;
  
  const hasCollapsibleTool = round.items.some(item => 
    item.type === 'tool' && isCollapsibleTool((item as FlowToolItem).toolName)
  );
  
  const hasAnyTool = round.items.some(item => item.type === 'tool');
  if (!hasAnyTool) return false;
  
  if (!hasCollapsibleTool) return false;
  
  const allItemsCollapsible = round.items.every(item => {
    if (item.type === 'tool') {
      return isCollapsibleTool((item as FlowToolItem).toolName);
    }
    return item.type === 'text' || item.type === 'thinking';
  });
  
  return allItemsCollapsible;
}

/**
 * Compute statistics for a single ModelRound
 */
function computeRoundStats(round: ModelRound): { readCount: number; searchCount: number; thinkingCount: number } {
  let readCount = 0;
  let searchCount = 0;
  let thinkingCount = 0;
  
  for (const item of round.items) {
    if (item.type === 'tool') {
      const toolName = (item as FlowToolItem).toolName;
      if (READ_TOOL_NAMES.has(toolName)) readCount++;
      else if (SEARCH_TOOL_NAMES.has(toolName)) searchCount++;
    } else if (item.type === 'thinking') {
      thinkingCount++;
    }
  }
  
  return { readCount, searchCount, thinkingCount };
}

let cachedSession: Session | null = null;
let cachedDialogTurnsRef: DialogTurn[] | null = null;
let cachedVirtualItems: VirtualItem[] = [];

/**
 * Convert Session to virtualized render items
 *
 * Performance optimizations:
 * 1. Uses references directly, relies on FlowChatStore immutable updates to detect reference changes
 * 2. Memoization cache: only recalculates when dialogTurns reference changes
 * 
 * Explore group merging: consecutive explore-only rounds merged into single explore-group VirtualItem
 */
export function sessionToVirtualItems(session: Session | null): VirtualItem[] {
  if (!session) {
    if (cachedSession !== null) {
      cachedSession = null;
      cachedDialogTurnsRef = null;
      cachedVirtualItems = [];
    }
    return cachedVirtualItems;
  }
  
  if (
    cachedSession?.sessionId === session.sessionId && 
    cachedDialogTurnsRef === session.dialogTurns
  ) {
    return cachedVirtualItems;
  }
  
  cachedSession = session;
  cachedDialogTurnsRef = session.dialogTurns;
  if (!session) return [];

  const items: VirtualItem[] = [];

  session.dialogTurns.forEach(turn => {
    if (turn.userMessage) {
      items.push({
        type: 'user-message',
        data: turn.userMessage,
        turnId: turn.id,
      });
    }

    if (turn.status === 'image_analyzing' && turn.modelRounds.length === 0) {
      items.push({ type: 'image-analyzing', turnId: turn.id });
      return;
    }

    const nonEmptyRounds = turn.modelRounds.filter(round => round.items && round.items.length > 0);
    
    interface TempExploreGroup {
      rounds: ModelRound[];
      allItems: import('../types/flow-chat').FlowItem[];
      readCount: number;
      searchCount: number;
      thinkingCount: number;
      startIndex: number;
      endIndex: number;
    }
    
    const tempGroups: TempExploreGroup[] = [];
    let currentGroup: TempExploreGroup | null = null;
    
    nonEmptyRounds.forEach((round, index) => {
      if (isExploreOnlyRound(round)) {
        const stats = computeRoundStats(round);
        if (currentGroup) {
          currentGroup.rounds.push(round);
          currentGroup.allItems.push(...round.items);
          currentGroup.readCount += stats.readCount;
          currentGroup.searchCount += stats.searchCount;
          currentGroup.thinkingCount += stats.thinkingCount;
          currentGroup.endIndex = index;
        } else {
          currentGroup = {
            rounds: [round],
            allItems: [...round.items],
            readCount: stats.readCount,
            searchCount: stats.searchCount,
            thinkingCount: stats.thinkingCount,
            startIndex: index,
            endIndex: index,
          };
        }
      } else {
        if (currentGroup) {
          tempGroups.push(currentGroup);
          currentGroup = null;
        }
      }
    });
    if (currentGroup) {
      tempGroups.push(currentGroup);
    }
    
    let roundIndex = 0;
    let groupIndex = 0;
    
    while (roundIndex < nonEmptyRounds.length) {
      const round = nonEmptyRounds[roundIndex];
      const group = tempGroups[groupIndex];
      
      if (group && group.startIndex === roundIndex) {
        const isLastGroup = groupIndex === tempGroups.length - 1;
        
        let isFollowedByCritical = false;
        const nextRoundIndex = group.endIndex + 1;
        if (nextRoundIndex < nonEmptyRounds.length) {
          const nextRound = nonEmptyRounds[nextRoundIndex];
          
          const hasAnyTool = nextRound.items.some(item => item.type === 'tool');
          
          if (nextRound.isStreaming && !hasAnyTool) {
            isFollowedByCritical = false;
          } else {
            isFollowedByCritical = !isExploreOnlyRound(nextRound);
          }
        }
        
        const isGroupStreaming = group.rounds.some(r => r.isStreaming);
        
        items.push({
          type: 'explore-group',
          turnId: turn.id,
          data: {
            groupId: group.rounds.map(r => r.id).join('-'),
            rounds: group.rounds,
            allItems: group.allItems,
            stats: { readCount: group.readCount, searchCount: group.searchCount, thinkingCount: group.thinkingCount },
            isGroupStreaming,
            isLastGroupInTurn: isLastGroup,
            isFollowedByCritical,
          }
        });
        
        roundIndex = group.endIndex + 1;
        groupIndex++;
      } else {
        const isLastRound = roundIndex === nonEmptyRounds.length - 1;
        items.push({
          type: 'model-round',
          data: round,
          turnId: turn.id,
          isLastRound,
        });
        roundIndex++;
      }
    }
  });

  cachedVirtualItems = items;
  return items;
}

export const useModernFlowChatStore = create<ModernFlowChatState>()(
  immer((set, get) => ({
    activeSession: null,
    virtualItems: [],
    visibleTurnInfo: null,

    setActiveSession: (session) => {
      set((state) => {
        state.activeSession = session;
      });

      get().updateVirtualItems();
    },

    updateVirtualItems: () => {
      const session = get().activeSession;
      const items = sessionToVirtualItems(session);
      
      set((state) => {
        state.virtualItems = items;
      });
    },

    setVisibleTurnInfo: (info) => {
      set((state) => {
        state.visibleTurnInfo = info;
      });
    },

    clear: () => {
      cachedSession = null;
      cachedDialogTurnsRef = null;
      cachedVirtualItems = [];

      set((state) => {
        state.activeSession = null;
        state.virtualItems = [];
        state.visibleTurnInfo = null;
      });
    },
  }))
);

export const useVirtualItems = () =>
  useModernFlowChatStore(state => state.virtualItems);

export const useActiveSession = () =>
  useModernFlowChatStore(state => state.activeSession);

export const useVisibleTurnInfo = () =>
  useModernFlowChatStore(state => state.visibleTurnInfo);

/**
 * Get actions (does not trigger re-render)
 */
export const useFlowChatActions = () =>
  useModernFlowChatStore(state => ({
    setActiveSession: state.setActiveSession,
    updateVirtualItems: state.updateVirtualItems,
    setVisibleTurnInfo: state.setVisibleTurnInfo,
    clear: state.clear,
  }));
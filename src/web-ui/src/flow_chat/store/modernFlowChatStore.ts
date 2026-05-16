/**
 * Modern FlowChat Store
 * High-performance state management using Zustand + Immer
 * Preserves original concept: Session → DialogTurn → ModelRound → FlowItem
 */

import { create } from 'zustand';
import { useShallow } from 'zustand/react/shallow';
import { immer } from 'zustand/middleware/immer';
import type { Session, DialogTurn, ModelRound, FlowItem, FlowToolItem, FlowUserSteeringItem } from '../types/flow-chat';
import { isCollapsibleTool, READ_TOOL_NAMES, SEARCH_TOOL_NAMES, COMMAND_TOOL_NAMES } from '../tool-cards';
import { COMPLETED_TOOL_TRANSIENT_MS } from '../components/modern/modelRoundItemGrouping';
import { flowChatStore } from './FlowChatStore';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ModernFlowChatStore');

/**
 * Explore group statistics (merged computed stats)
 */
export interface ExploreGroupStats {
  readCount: number;
  searchCount: number;
  commandCount: number;
}

/**
 * Explore group data (for explore-group type VirtualItem)
 * Merges consecutive explore-only rounds into a single render unit
 */
export interface ExploreGroupData {
  groupId: string;
  rounds: ModelRound[];
  allItems: FlowItem[];
  stats: ExploreGroupStats;
  isGroupStreaming: boolean;
  isLastGroupInTurn: boolean;
  /**
   * True when this group is no longer the tail of the turn — a non-explore
   * (critical) round or turn completion has ended the group. The renderer uses
   * this to trigger a one-shot auto-collapse instead of continuously watching
   * isGroupStreaming.
   */
  wasCutByCritical: boolean;
}

/**
 * Virtualized render unit
 * Used for virtual scrolling, flattens DialogTurn into renderable items
 */
export type VirtualItem =
  | { type: 'user-message'; data: DialogTurn['userMessage']; turnId: string }
  | {
      type: 'user-steering-message';
      data: NonNullable<DialogTurn['userMessage']>;
      turnId: string;
      steeringId: string;
      steeringStatus: FlowUserSteeringItem['status'];
    }
  | { type: 'model-round'; data: ModelRound; turnId: string; isLastRound: boolean; isTurnComplete: boolean }
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
 * Keep streaming narrative visible in-place until the stream settles; otherwise
 * a mid-stream switch to explore-group remounts the text block and replays the
 * typewriter animation from the beginning.
 */
function hasActiveStreamingNarrative(round: ModelRound): boolean {
  return round.items.some(item => {
    if (item.type !== 'text' && item.type !== 'thinking') return false;
    const maybeStreaming = item as { isStreaming?: boolean; status?: string };
    return maybeStreaming.isStreaming === true &&
      (maybeStreaming.status === 'streaming' || maybeStreaming.status === 'running');
  });
}

function hasActiveTool(round: ModelRound): boolean {
  return round.items.some(item => {
    if (item.type !== 'tool') return false;
    return item.status !== 'completed' && item.status !== 'cancelled' && item.status !== 'error';
  });
}

function hasRecentlyCompletedTool(round: ModelRound, nowMs: number): boolean {
  return round.items.some(item => {
    if (item.type !== 'tool' || item.status !== 'completed') return false;
    const endTime = (item as FlowToolItem).endTime;
    return typeof endTime === 'number' && nowMs - endTime < COMPLETED_TOOL_TRANSIENT_MS;
  });
}

function isExploreOnlyRound(round: ModelRound, nowMs: number): boolean {
  if (!round.items || round.items.length === 0) return false;

  if (round.renderHints?.disableExploreGrouping === true) {
    return false;
  }

  if (round.isStreaming && hasActiveStreamingNarrative(round)) {
    return false;
  }

  if (hasActiveTool(round) || hasRecentlyCompletedTool(round, nowMs)) {
    return false;
  }
  
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
function computeRoundStats(round: ModelRound): ExploreGroupStats {
  let readCount = 0;
  let searchCount = 0;
  let commandCount = 0;
  
  for (const item of round.items) {
    if (item.type === 'tool') {
      const toolName = (item as FlowToolItem).toolName;
      if (READ_TOOL_NAMES.has(toolName)) readCount++;
      else if (SEARCH_TOOL_NAMES.has(toolName)) searchCount++;
      else if (COMMAND_TOOL_NAMES.has(toolName)) commandCount++;
    }
  }
  
  return { readCount, searchCount, commandCount };
}

function steeringItemToUserMessage(item: FlowUserSteeringItem): NonNullable<DialogTurn['userMessage']> {
  return {
    id: `user_steering_${item.steeringId}`,
    content: item.content,
    timestamp: item.timestamp,
  };
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

  const items: VirtualItem[] = [];
  const nowMs = Date.now();

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

    const renderEntries: Array<
      | { type: 'round'; round: ModelRound }
      | { type: 'steering'; item: FlowUserSteeringItem }
    > = [];

    turn.modelRounds.forEach(round => {
      if (!round.items || round.items.length === 0) return;
      const nonSteeringItems = round.items.filter(item => item.type !== 'user-steering');
      if (nonSteeringItems.length > 0) {
        renderEntries.push({
          type: 'round',
          round: nonSteeringItems.length === round.items.length
            ? round
            : { ...round, items: nonSteeringItems },
        });
      }
      round.items
        .filter((item): item is FlowUserSteeringItem => item.type === 'user-steering')
        .forEach(item => {
          renderEntries.push({ type: 'steering', item });
        });
    });
    
    const isTurnComplete = turn.status === 'completed' || turn.status === 'cancelled' || turn.status === 'error';

    const flushRoundEntries = (
      rounds: ModelRound[],
      _options: { collapseTrailingExploreGroup: boolean },
    ) => {
      if (rounds.length === 0) return;

      interface TempExploreGroup {
        rounds: ModelRound[];
        allItems: FlowItem[];
        readCount: number;
        searchCount: number;
        commandCount: number;
        startIndex: number;
        endIndex: number;
      }

      const tempGroups: TempExploreGroup[] = [];
      let currentGroup: TempExploreGroup | null = null;

      rounds.forEach((round, index) => {
        const exploreOnly = isExploreOnlyRound(round, nowMs);
        if (exploreOnly) {
          const stats = computeRoundStats(round);
          if (currentGroup) {
            currentGroup.rounds.push(round);
            currentGroup.allItems.push(...round.items);
            currentGroup.readCount += stats.readCount;
            currentGroup.searchCount += stats.searchCount;
            currentGroup.commandCount += stats.commandCount;
            currentGroup.endIndex = index;
          } else {
            currentGroup = {
              rounds: [round],
              allItems: [...round.items],
              readCount: stats.readCount,
              searchCount: stats.searchCount,
              commandCount: stats.commandCount,
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

      // Always flush the trailing explore group so its container is stable
      // throughout streaming. The wasCutByCritical flag distinguishes "still
      // growing" from "permanently closed" for the renderer.
      if (currentGroup) {
        tempGroups.push(currentGroup);
      }

      let roundIndex = 0;
      let groupIndex = 0;

      while (roundIndex < rounds.length) {
        const round = rounds[roundIndex];
        const group = tempGroups[groupIndex];

        if (group && group.startIndex === roundIndex) {
          const isLastGroup = groupIndex === tempGroups.length - 1;
          const isGroupStreaming = group.rounds.some(r => r.isStreaming);
          // A group is "cut by critical" when it is no longer the tail of the
          // turn. Two conditions cover all cases:
          //   1. group.endIndex < rounds.length - 1: there are rounds after
          //      this group's last round — they could be non-explore (critical)
          //      rounds OR another explore group. Either way this group is no
          //      longer the tail.
          //      NOTE: checking !isLastGroup alone is NOT sufficient because
          //      tempGroups only contains explore-only groups; a following
          //      critical round (e.g. TodoWrite) is invisible to tempGroups
          //      yet still sits after this group in the rounds array.
          //   2. turn is complete and no round in this group is still streaming.
          const wasCutByCritical =
            group.endIndex < rounds.length - 1 ||
            (isTurnComplete && !isGroupStreaming);

          const groupId = group.rounds[0]?.id ?? `explore-group-${turn.id}-${group.startIndex}`;

          if (wasCutByCritical) {
            log.debug('explore-group marked wasCutByCritical', {
              groupId,
              endIndex: group.endIndex,
              totalRounds: rounds.length,
              isTurnComplete,
              isGroupStreaming,
            });
          }

          items.push({
            type: 'explore-group',
            turnId: turn.id,
            data: {
              groupId,
              rounds: group.rounds,
              allItems: group.allItems,
              stats: {
                readCount: group.readCount,
                searchCount: group.searchCount,
                commandCount: group.commandCount,
              },
              isGroupStreaming,
              isLastGroupInTurn: isLastGroup,
              wasCutByCritical,
            },
          });

          roundIndex = group.endIndex + 1;
          groupIndex++;
        } else {
          const isLastRound = roundIndex === rounds.length - 1;
          items.push({
            type: 'model-round',
            data: round,
            turnId: turn.id,
            isLastRound,
            isTurnComplete,
          });
          roundIndex++;
        }
      }
    };

    let pendingRounds: ModelRound[] = [];

    renderEntries.forEach(entry => {
      if (entry.type === 'round') {
        pendingRounds.push(entry.round);
        return;
      }

      flushRoundEntries(pendingRounds, { collapseTrailingExploreGroup: true });
      pendingRounds = [];

      items.push({
        type: 'user-steering-message',
        data: steeringItemToUserMessage(entry.item),
        turnId: turn.id,
        steeringId: entry.item.steeringId,
        steeringStatus: entry.item.status,
      });
    });

    flushRoundEntries(pendingRounds, { collapseTrailingExploreGroup: true });

  });

  cachedVirtualItems = items;
  return items;
}

function getInitialModernState(): Pick<
  ModernFlowChatState,
  'activeSession' | 'virtualItems' | 'visibleTurnInfo'
> {
  const legacyState = flowChatStore.getState();
  const activeSession = legacyState.activeSessionId
    ? legacyState.sessions.get(legacyState.activeSessionId) ?? null
    : null;

  return {
    activeSession,
    virtualItems: sessionToVirtualItems(activeSession),
    visibleTurnInfo: null,
  };
}

export const useModernFlowChatStore = create<ModernFlowChatState>()(
  immer((set, get) => ({
    ...getInitialModernState(),

    setActiveSession: (session) => {
      const items = sessionToVirtualItems(session);
      set((state) => {
        state.activeSession = session;
        state.virtualItems = items;
      });
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
  useModernFlowChatStore(useShallow(state => ({
    setActiveSession: state.setActiveSession,
    updateVirtualItems: state.updateVirtualItems,
    setVisibleTurnInfo: state.setVisibleTurnInfo,
    clear: state.clear,
  })));

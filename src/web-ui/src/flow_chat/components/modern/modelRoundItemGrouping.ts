import type { FlowItem, FlowToolItem } from '../../types/flow-chat';
import { getEffectiveToolName } from '../../utils/toolInvocationIdentity';

export type ModelRoundItemGroup =
  | { type: 'explore'; items: FlowItem[]; isLast: boolean }
  | { type: 'critical'; item: FlowItem };

interface BuildModelRoundItemGroupsInput {
  items: FlowItem[];
  isStreaming: boolean;
  disableExploreGrouping: boolean;
  isCollapsibleTool: (toolName: string) => boolean;
}

function hasActiveStreamingNarrative(items: FlowItem[]): boolean {
  return items.some(item => {
    if (item.type !== 'text' && item.type !== 'thinking') return false;
    const maybeStreaming = item as { isStreaming?: boolean; status?: string };
    return maybeStreaming.isStreaming === true &&
      (maybeStreaming.status === 'streaming' || maybeStreaming.status === 'running');
  });
}

function isActiveToolItem(item: FlowItem): boolean {
  if (item.type !== 'tool') return false;
  return item.status !== 'completed' && item.status !== 'cancelled' && item.status !== 'rejected' && item.status !== 'error';
}

/**
 * Grouping is intentionally a pure function of the round data. It must never
 * depend on wall-clock time: a time-dependent grouping re-runs on a timer,
 * restructures the round, and remounts cards long after the data settled —
 * which reads as the chat pane spontaneously refreshing itself.
 */
export function buildModelRoundItemGroups({
  items,
  isStreaming,
  disableExploreGrouping,
  isCollapsibleTool,
}: BuildModelRoundItemGroupsInput): ModelRoundItemGroup[] {
  const deferExploreGrouping = disableExploreGrouping || (isStreaming && hasActiveStreamingNarrative(items));
  const intermediateGroups: Array<{ type: 'normal'; item: FlowItem }> = items.map(item => ({
    type: 'normal',
    item,
  }));

  const finalGroups: ModelRoundItemGroup[] = [];
  let exploreBuffer: FlowItem[] = [];
  let pendingBuffer: FlowItem[] = [];

  const normalItems = intermediateGroups
    .filter((group): group is { type: 'normal'; item: FlowItem } => group.type === 'normal')
    .map(group => group.item);

  const flushExploreBuffer = (isLast: boolean) => {
    if (exploreBuffer.length > 0) {
      finalGroups.push({ type: 'explore', items: [...exploreBuffer], isLast });
      exploreBuffer = [];
    }
  };

  const flushPendingAsCritical = () => {
    for (const item of pendingBuffer) {
      finalGroups.push({ type: 'critical', item });
    }
    pendingBuffer = [];
  };

  let normalItemIndex = 0;

  for (let i = 0; i < intermediateGroups.length; i++) {
    const group = intermediateGroups[i];
    const isLastGroup = i === intermediateGroups.length - 1;

    const item = group.item;
    const isLastNormalItem = normalItemIndex === normalItems.length - 1;

    if (item.type === 'text' || item.type === 'thinking') {
      pendingBuffer.push(item);

      if (isLastNormalItem) {
        flushExploreBuffer(false);
        flushPendingAsCritical();
      }
    } else if (item.type === 'tool') {
      const toolName = getEffectiveToolName(item as FlowToolItem);
      const isExploreTool = isCollapsibleTool(toolName);

      if (isExploreTool) {
        const keepAsCritical = deferExploreGrouping || isActiveToolItem(item);

        if (keepAsCritical) {
          flushExploreBuffer(false);
          flushPendingAsCritical();
          finalGroups.push({ type: 'critical', item });
          normalItemIndex++;
          continue;
        }
        exploreBuffer.push(...pendingBuffer, item);
        pendingBuffer = [];

        if (isLastNormalItem || isLastGroup) {
          flushExploreBuffer(true);
        }
      } else {
        flushExploreBuffer(false);
        flushPendingAsCritical();
        finalGroups.push({ type: 'critical', item });
      }
    } else {
      flushExploreBuffer(false);
      flushPendingAsCritical();
      finalGroups.push({ type: 'critical', item });
    }

    normalItemIndex++;
  }

  flushExploreBuffer(true);
  flushPendingAsCritical();

  return finalGroups;
}

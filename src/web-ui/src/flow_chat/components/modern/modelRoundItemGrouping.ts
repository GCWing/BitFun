import type { FlowItem, FlowToolItem } from '../../types/flow-chat';

export type ModelRoundItemGroup =
  | { type: 'explore'; items: FlowItem[]; isLast: boolean }
  | { type: 'critical'; item: FlowItem }
  | { type: 'subagent'; parentTaskToolId: string; items: FlowItem[] };

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

export function buildModelRoundItemGroups({
  items,
  isStreaming,
  disableExploreGrouping,
  isCollapsibleTool,
}: BuildModelRoundItemGroupsInput): ModelRoundItemGroup[] {
  const deferExploreGrouping = disableExploreGrouping || (isStreaming && hasActiveStreamingNarrative(items));
  const intermediateGroups: Array<
    | { type: 'normal'; item: FlowItem }
    | { type: 'subagent'; parentTaskToolId: string; items: FlowItem[] }
  > = [];
  let currentSubagentGroup: { parentTaskToolId: string; items: FlowItem[] } | null = null;

  for (const item of items) {
    const isSubagentItemFlag = (item as any).isSubagentItem === true;
    const parentTaskToolId = (item as any).parentTaskToolId;

    if (isSubagentItemFlag && parentTaskToolId) {
      if (currentSubagentGroup && currentSubagentGroup.parentTaskToolId === parentTaskToolId) {
        currentSubagentGroup.items.push(item);
      } else {
        if (currentSubagentGroup) {
          intermediateGroups.push({ type: 'subagent', ...currentSubagentGroup });
        }
        currentSubagentGroup = { parentTaskToolId, items: [item] };
      }
    } else {
      if (currentSubagentGroup) {
        intermediateGroups.push({ type: 'subagent', ...currentSubagentGroup });
        currentSubagentGroup = null;
      }
      intermediateGroups.push({ type: 'normal', item });
    }
  }

  if (currentSubagentGroup) {
    intermediateGroups.push({ type: 'subagent', ...currentSubagentGroup });
  }

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

    if (group.type === 'subagent') {
      flushExploreBuffer(false);
      flushPendingAsCritical();
      finalGroups.push(group);
      continue;
    }

    const item = group.item;
    const isLastNormalItem = normalItemIndex === normalItems.length - 1;

    if (item.type === 'text' || item.type === 'thinking') {
      pendingBuffer.push(item);

      if (isLastNormalItem) {
        flushExploreBuffer(false);
        flushPendingAsCritical();
      }
    } else if (item.type === 'tool') {
      const toolName = (item as FlowToolItem).toolName;
      const isExploreTool = isCollapsibleTool(toolName);

      if (isExploreTool) {
        if (deferExploreGrouping) {
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

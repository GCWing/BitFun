import { describe, expect, it } from 'vitest';
import {
  buildModelRoundItemGroups,
  isCompletedToolInTransientWindow,
} from './modelRoundItemGrouping';
import type { FlowTextItem, FlowToolItem, FlowUserSteeringItem } from '../../types/flow-chat';

function makeTextItem(id: string): FlowTextItem {
  return {
    id,
    type: 'text',
    content: 'assistant text',
    isStreaming: false,
    isMarkdown: true,
    timestamp: 1000,
    status: 'completed',
  };
}

function makeReadTool(
  id: string,
  status: FlowToolItem['status'] = 'completed',
  endTime?: number,
): FlowToolItem {
  return {
    id,
    type: 'tool',
    toolName: 'Read',
    timestamp: 1001,
    status,
    toolCall: {
      id,
      input: { file_path: 'src/main.rs' },
    },
    ...(status === 'completed'
      ? {
          toolResult: {
            result: 'file contents',
            success: true,
          },
        }
      : {}),
    ...(endTime !== undefined ? { endTime } : {}),
  };
}

function makeSteeringItem(id: string): FlowUserSteeringItem {
  return {
    id,
    type: 'user-steering',
    steeringId: id,
    content: 'Run the newly queued request now',
    roundIndex: 0,
    timestamp: 1002,
    status: 'pending',
  };
}

describe('buildModelRoundItemGroups', () => {
  it('keeps user-steering items as critical visible content', () => {
    const steeringItem = makeSteeringItem('steering-1');

    const groups = buildModelRoundItemGroups({
      items: [steeringItem],
      isStreaming: true,
      disableExploreGrouping: false,
      isCollapsibleTool: () => false,
    });

    expect(groups).toEqual([
      {
        type: 'critical',
        item: steeringItem,
      },
    ]);
  });

  it('flushes pending assistant text before rendering user-steering content', () => {
    const textItem = makeTextItem('text-1');
    const steeringItem = makeSteeringItem('steering-1');

    const groups = buildModelRoundItemGroups({
      items: [textItem, steeringItem],
      isStreaming: true,
      disableExploreGrouping: false,
      isCollapsibleTool: () => false,
    });

    expect(groups).toEqual([
      {
        type: 'critical',
        item: textItem,
      },
      {
        type: 'critical',
        item: steeringItem,
      },
    ]);
  });

  it('preserves existing explore grouping for collapsible tool rounds', () => {
    const textItem = makeTextItem('text-1');
    const toolItem = makeReadTool('tool-1');

    const groups = buildModelRoundItemGroups({
      items: [textItem, toolItem],
      isStreaming: false,
      disableExploreGrouping: false,
      isCollapsibleTool: toolName => toolName === 'Read',
    });

    expect(groups).toEqual([
      {
        type: 'explore',
        items: [textItem, toolItem],
        isLast: true,
      },
    ]);
  });

  it('keeps an active collapsible tool outside the preceding explore group', () => {
    const completedTool = makeReadTool('tool-1');
    const runningTool = makeReadTool('tool-2', 'running');

    const groups = buildModelRoundItemGroups({
      items: [completedTool, runningTool],
      isStreaming: true,
      disableExploreGrouping: false,
      isCollapsibleTool: toolName => toolName === 'Read',
    });

    expect(groups).toEqual([
      {
        type: 'explore',
        items: [completedTool],
        isLast: false,
      },
      {
        type: 'critical',
        item: runningTool,
      },
    ]);
  });

  it('keeps a just-completed collapsible tool visible before merging it', () => {
    const completedTool = makeReadTool('tool-1');
    const justCompletedTool = makeReadTool('tool-2', 'completed', 10_000);

    const groups = buildModelRoundItemGroups({
      items: [completedTool, justCompletedTool],
      isStreaming: true,
      disableExploreGrouping: false,
      isCollapsibleTool: toolName => toolName === 'Read',
      nowMs: 10_200,
    });

    expect(groups).toEqual([
      {
        type: 'explore',
        items: [completedTool],
        isLast: false,
      },
      {
        type: 'critical',
        item: justCompletedTool,
      },
    ]);
  });

  it('merges a completed collapsible tool after the transition window', () => {
    const completedTool = makeReadTool('tool-1');
    const settledTool = makeReadTool('tool-2', 'completed', 10_000);

    const groups = buildModelRoundItemGroups({
      items: [completedTool, settledTool],
      isStreaming: true,
      disableExploreGrouping: false,
      isCollapsibleTool: toolName => toolName === 'Read',
      nowMs: 11_001,
    });

    expect(groups).toEqual([
      {
        type: 'explore',
        items: [completedTool, settledTool],
        isLast: true,
      },
    ]);
  });

  it('does not keep non-streaming completed tools in a time-based critical state', () => {
    const completedTool = makeReadTool('tool-1');
    const justCompletedTool = makeReadTool('tool-2', 'completed', 10_000);

    const groups = buildModelRoundItemGroups({
      items: [completedTool, justCompletedTool],
      isStreaming: false,
      disableExploreGrouping: false,
      isCollapsibleTool: toolName => toolName === 'Read',
      nowMs: 10_200,
    });

    expect(groups).toEqual([
      {
        type: 'explore',
        items: [completedTool, justCompletedTool],
        isLast: true,
      },
    ]);
  });
});

describe('isCompletedToolInTransientWindow', () => {
  it('treats only freshly completed tools as transient', () => {
    expect(isCompletedToolInTransientWindow(
      makeReadTool('tool-1', 'completed', 10_000),
      10_200,
    )).toBe(true);
    expect(isCompletedToolInTransientWindow(
      makeReadTool('tool-2', 'completed', 10_000),
      11_001,
    )).toBe(false);
  });

  it('does not classify historical or invalid completion times as transient', () => {
    expect(isCompletedToolInTransientWindow(
      makeReadTool('tool-1', 'completed'),
      10_200,
    )).toBe(false);
    expect(isCompletedToolInTransientWindow(
      makeReadTool('tool-2', 'completed', 12_000),
      10_200,
    )).toBe(false);
    expect(isCompletedToolInTransientWindow(
      makeReadTool('tool-3', 'running'),
      10_200,
    )).toBe(false);
  });
});

import { describe, expect, it } from 'vitest';
import { buildModelRoundItemGroups } from './modelRoundItemGrouping';
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

function makeReadTool(id: string): FlowToolItem {
  return {
    id,
    type: 'tool',
    toolName: 'Read',
    timestamp: 1001,
    status: 'completed',
    toolCall: {
      id,
      input: { file_path: 'src/main.rs' },
    },
    toolResult: {
      result: 'file contents',
      success: true,
    },
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
});

import { describe, expect, it } from 'vitest';
import type { FlowToolItem, ModelRound, Session } from '../../types/flow-chat';
import type { VirtualItem } from '../../store/modernFlowChatStore';
import { resolveFlowChatFocusTarget } from './flowChatFocusTarget';

function makeReadTool(id: string): FlowToolItem {
  return {
    id,
    type: 'tool',
    toolName: 'Read',
    timestamp: 1000,
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

function makeRound(items: FlowToolItem[]): ModelRound {
  return {
    id: 'round-1',
    index: 0,
    items,
    isStreaming: false,
    isComplete: true,
    status: 'completed',
    startTime: 1000,
  };
}

function makeSession(round: ModelRound): Session {
  return {
    sessionId: 'session-1',
    dialogTurns: [{
      id: 'turn-1',
      sessionId: 'session-1',
      userMessage: {
        id: 'user-1',
        content: 'Inspect the file',
        timestamp: 900,
      },
      modelRounds: [round],
      status: 'completed',
      startTime: 900,
    }],
    status: 'idle',
    config: {},
    createdAt: 800,
    lastActiveAt: 1000,
    error: null,
    sessionKind: 'flow_chat',
  };
}

describe('useFlowChatNavigation focus resolution', () => {
  it('requests explore group expansion before focusing a grouped tool item', () => {
    const tool = makeReadTool('tool-1');
    const round = makeRound([tool]);
    const session = makeSession(round);
    const virtualItems: VirtualItem[] = [
      {
        type: 'user-message',
        data: session.dialogTurns[0].userMessage,
        turnId: 'turn-1',
      },
      {
        type: 'explore-group',
        turnId: 'turn-1',
        data: {
          groupId: 'round-1',
          rounds: [round],
          allItems: [tool],
          stats: {
            readCount: 1,
            searchCount: 0,
            commandCount: 0,
          },
          isGroupStreaming: false,
          isLastGroupInTurn: true,
        },
      },
    ];

    const target = resolveFlowChatFocusTarget({
      sessionId: session.sessionId,
      turnIndex: 1,
      itemId: tool.id,
      source: 'usage-report',
    }, virtualItems, session);

    expect(target).toMatchObject({
      resolvedVirtualIndex: 1,
      resolvedTurnId: 'turn-1',
      resolvedTurnIndex: 1,
      expandExploreGroupId: 'round-1',
      focusItemId: tool.id,
      preferPinnedTurnNavigation: false,
    });
  });
});

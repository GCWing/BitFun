import { afterEach, describe, expect, it } from 'vitest';
import { FlowChatStore } from '../../store/FlowChatStore';
import type { DialogTurn, FlowToolItem, ModelRound, Session } from '../../types/flow-chat';
import { processToolParamsPartialInternal } from './ToolEventModule';

function resetStore(): void {
  FlowChatStore.getInstance().setState(() => ({
    sessions: new Map(),
    activeSessionId: null,
  }));
}

function createSessionWithTool(tool: FlowToolItem): Session {
  const round: ModelRound = {
    id: 'round-1',
    index: 0,
    items: [tool],
    isStreaming: true,
    isComplete: false,
    status: 'streaming',
    startTime: 1000,
  };
  const turn: DialogTurn = {
    id: 'turn-1',
    sessionId: 'session-1',
    userMessage: {
      id: 'user-1',
      content: 'Inspect this file',
      timestamp: 900,
    },
    modelRounds: [round],
    status: 'processing',
    startTime: 900,
  };

  return {
    sessionId: 'session-1',
    title: 'Session 1',
    dialogTurns: [turn],
    status: 'active',
    config: { agentType: 'agentic' },
    createdAt: 800,
    lastActiveAt: 1000,
    error: null,
    sessionKind: 'normal',
  };
}

describe('processToolParamsPartialInternal', () => {
  afterEach(() => {
    resetStore();
  });

  it('drops malformed non-string params fragments without replacing existing preview state', () => {
    const existingParams = { file_path: 'src/main.rs' };
    const tool: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      toolName: 'Read',
      timestamp: 1001,
      status: 'streaming',
      toolCall: {
        id: 'tool-1',
        input: existingParams,
      },
      isParamsStreaming: true,
      partialParams: existingParams,
      _paramsBuffer: '{"file_path":"src/main.rs"}',
    };

    FlowChatStore.getInstance().setState(() => ({
      sessions: new Map([['session-1', createSessionWithTool(tool)]]),
      activeSessionId: 'session-1',
    }));

    expect(() => {
      processToolParamsPartialInternal('session-1', 'turn-1', {
        event_type: 'ParamsPartial',
        tool_id: 'tool-1',
        tool_name: 'Read',
        params: { file_path: 'src/lib.rs' } as any,
      });
    }).not.toThrow();

    const updatedTool = FlowChatStore.getInstance()
      .findToolItem('session-1', 'turn-1', 'tool-1') as FlowToolItem;

    expect(updatedTool._paramsBuffer).toBe('{"file_path":"src/main.rs"}');
    expect(updatedTool.partialParams).toEqual(existingParams);
    expect(updatedTool.toolCall.input).toEqual(existingParams);
  });
});

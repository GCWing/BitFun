import { afterEach, describe, expect, it } from 'vitest';
import { FlowChatStore } from '../../store/FlowChatStore';
import type { DialogTurn, FlowToolItem, ModelRound, Session } from '../../types/flow-chat';
import { processToolEvent, processToolParamsPartialInternal } from './ToolEventModule';

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

function makeToolContext(): any {
  return {
    eventBatcher: {
      getBufferSize: () => 0,
      flushNow: () => {},
    },
  };
}

function makeAskUserQuestionTool(
  id: string,
  status: FlowToolItem['status'],
  error?: string,
): FlowToolItem {
  return {
    id,
    type: 'tool',
    toolName: 'AskUserQuestion',
    timestamp: 1000,
    status,
    toolCall: {
      id,
      input: {},
    },
    toolResult: error
      ? {
          result: null,
          success: false,
          error,
        }
      : undefined,
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

  it('injects file_path from write params buffer when content streams first', () => {
    const tool: FlowToolItem = {
      id: 'tool-1',
      type: 'tool',
      toolName: 'Write',
      timestamp: 1001,
      status: 'preparing',
      toolCall: {
        id: 'tool-1',
        input: {},
      },
      isParamsStreaming: true,
      partialParams: {},
      _paramsBuffer: '',
    };

    FlowChatStore.getInstance().setState(() => ({
      sessions: new Map([['session-1', createSessionWithTool(tool)]]),
      activeSessionId: 'session-1',
    }));

    processToolParamsPartialInternal('session-1', 'turn-1', {
      event_type: 'ParamsPartial',
      tool_id: 'tool-1',
      tool_name: 'Write',
      params: '{"file_path":"src/app.ts","content":"const value = 1;',
    });

    const updatedTool = FlowChatStore.getInstance()
      .findToolItem('session-1', 'turn-1', 'tool-1') as FlowToolItem;

    expect(updatedTool.partialParams?.file_path).toBe('src/app.ts');
    expect(updatedTool.partialParams?.content).toBe('const value = 1;');
    expect(updatedTool.status).toBe('receiving');
  });
});

describe('processToolEvent late Started event behavior', () => {
  afterEach(() => {
    resetStore();
  });

  it('attaches a late Started event back to its original round when roundId is provided', () => {
    const round1: ModelRound = {
      id: 'round-1',
      index: 0,
      items: [
        {
          id: 'text-1',
          type: 'text',
          content: 'First round response',
          timestamp: 1000,
          status: 'completed',
          isStreaming: false,
          isMarkdown: true,
        } as any,
        {
          id: 'steering-1',
          type: 'user-steering',
          timestamp: 1001,
          status: 'completed',
          content: 'background result',
          steeringId: 'steering-1',
          roundIndex: 0,
        } as any,
      ],
      isStreaming: false,
      isComplete: true,
      status: 'completed',
      startTime: 900,
      endTime: 1100,
    };

    const round2: ModelRound = {
      id: 'round-2',
      index: 1,
      items: [],
      isStreaming: true,
      isComplete: false,
      status: 'streaming',
      startTime: 1200,
    };

    const turn: DialogTurn = {
      id: 'turn-1',
      sessionId: 'session-1',
      userMessage: {
        id: 'user-1',
        content: 'Test steering timing',
        timestamp: 800,
      },
      modelRounds: [round1, round2],
      status: 'processing',
      startTime: 800,
    };

    const session: Session = {
      sessionId: 'session-1',
      title: 'Session 1',
      dialogTurns: [turn],
      status: 'active',
      config: { agentType: 'agentic' },
      createdAt: 700,
      lastActiveAt: 1200,
      error: null,
      sessionKind: 'normal',
    };

    FlowChatStore.getInstance().setState(() => ({
      sessions: new Map([['session-1', session]]),
      activeSessionId: 'session-1',
    }));

    processToolEvent(
      makeToolContext(),
      'session-1',
      'turn-1',
      'round-1',
      {
        event_type: 'Started',
        tool_id: 'tool-late-1',
        tool_name: 'Read',
        params: { file_path: 'src/main.rs' },
      },
    );

    const state = FlowChatStore.getInstance().getState();
    const updatedTurn = state.sessions.get('session-1')?.dialogTurns[0];
    const updatedRound1 = updatedTurn?.modelRounds[0];
    const updatedRound2 = updatedTurn?.modelRounds[1];

    expect(updatedRound1?.items.some(item => item.id === 'tool-late-1')).toBe(true);
    expect(updatedRound2?.items.some(item => item.id === 'tool-late-1')).toBe(false);
  });

  it('drops a Started event when the referenced round does not exist', () => {
    const turn: DialogTurn = {
      id: 'turn-1',
      sessionId: 'session-1',
      userMessage: {
        id: 'user-1',
        content: 'Test steering timing',
        timestamp: 800,
      },
      modelRounds: [{
        id: 'round-1',
        index: 0,
        items: [],
        isStreaming: true,
        isComplete: false,
        status: 'streaming',
        startTime: 900,
      }],
      status: 'processing',
      startTime: 800,
    };

    const session: Session = {
      sessionId: 'session-1',
      title: 'Session 1',
      dialogTurns: [turn],
      status: 'active',
      config: { agentType: 'agentic' },
      createdAt: 700,
      lastActiveAt: 1200,
      error: null,
      sessionKind: 'normal',
    };

    FlowChatStore.getInstance().setState(() => ({
      sessions: new Map([['session-1', session]]),
      activeSessionId: 'session-1',
    }));

    processToolEvent(
      makeToolContext(),
      'session-1',
      'turn-1',
      'round-missing',
      {
        event_type: 'Started',
        tool_id: 'tool-late-1',
        tool_name: 'Read',
        params: { file_path: 'src/main.rs' },
      },
    );

    const updatedTurn = FlowChatStore.getInstance().getState().sessions.get('session-1')?.dialogTurns[0];
    expect(updatedTurn?.modelRounds[0]?.items.some(item => item.id === 'tool-late-1')).toBe(false);
  });
});

describe('processToolEvent AskUserQuestion retry cleanup', () => {
  afterEach(() => {
    resetStore();
  });

  it('removes stale parse failure cards when a retry question is early detected', () => {
    const staleTool = makeAskUserQuestionTool(
      'ask-stale',
      'error',
      'Failed to parse input parameters: missing field `questions`',
    );
    const cancelledTool = makeAskUserQuestionTool(
      'ask-cancelled',
      'cancelled',
      'User cancelled operation',
    );
    const ordinaryFailedTool = makeAskUserQuestionTool(
      'ask-failed',
      'error',
      'User input channel closed',
    );

    const turn: DialogTurn = {
      id: 'turn-1',
      sessionId: 'session-1',
      userMessage: {
        id: 'user-1',
        content: 'Ask me if needed',
        timestamp: 800,
      },
      modelRounds: [
        {
          id: 'round-1',
          index: 0,
          items: [staleTool, cancelledTool, ordinaryFailedTool],
          isStreaming: false,
          isComplete: true,
          status: 'completed',
          startTime: 900,
        },
        {
          id: 'round-2',
          index: 1,
          items: [],
          isStreaming: true,
          isComplete: false,
          status: 'streaming',
          startTime: 1200,
        },
      ],
      status: 'processing',
      startTime: 800,
    };

    const session: Session = {
      sessionId: 'session-1',
      title: 'Session 1',
      dialogTurns: [turn],
      status: 'active',
      config: { agentType: 'agentic' },
      createdAt: 700,
      lastActiveAt: 1200,
      error: null,
      sessionKind: 'normal',
    };

    FlowChatStore.getInstance().setState(() => ({
      sessions: new Map([['session-1', session]]),
      activeSessionId: 'session-1',
    }));

    processToolEvent(
      makeToolContext(),
      'session-1',
      'turn-1',
      'round-2',
      {
        event_type: 'EarlyDetected',
        tool_id: 'ask-retry',
        tool_name: 'AskUserQuestion',
      },
    );

    const updatedTurn = FlowChatStore.getInstance().getState().sessions.get('session-1')?.dialogTurns[0];
    const allItemIds = updatedTurn?.modelRounds.flatMap(round => round.items.map(item => item.id)) || [];

    expect(allItemIds).not.toContain('ask-stale');
    expect(allItemIds).toContain('ask-cancelled');
    expect(allItemIds).toContain('ask-failed');
    expect(allItemIds).toContain('ask-retry');
  });
});

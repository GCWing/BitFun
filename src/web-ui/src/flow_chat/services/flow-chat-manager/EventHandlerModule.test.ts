import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { normalizeSubagentParentInfo } from './subagentParentInfo';
import {
  formatDialogErrorForNotification,
  handleSessionStateChanged,
  insertSteeringItemIfAbsent,
  shouldProcessEvent,
} from './EventHandlerModule';
import { stateMachineManager } from '../../state-machine';
import { SessionExecutionEvent, SessionExecutionState } from '../../state-machine/types';
import { FlowChatStore } from '../../store/FlowChatStore';
import type { DialogTurn, FlowUserSteeringItem, ModelRound, Session } from '../../types/flow-chat';
import type { FlowChatContext } from './types';

vi.mock('@/infrastructure/i18n/core/I18nService', () => ({
  i18nService: {
    t: (key: string) => ({
      'errors:ai.invalidRequest.title': 'Model request invalid',
      'errors:ai.invalidRequest.message': 'The provider rejected the request format, parameters, model name, or payload size. Adjust the request or choose another model.',
      'errors:ai.actions.copyDiagnostics': 'Copy diagnostics',
    }[key] ?? key),
  },
}));

describe('normalizeSubagentParentInfo', () => {
  it('normalizes snake_case subagent parent metadata from backend events', () => {
    expect(
      normalizeSubagentParentInfo({
        subagent_parent_info: {
          session_id: 'parent',
          dialog_turn_id: 'turn',
          tool_call_id: 'tool',
        },
      }),
    ).toEqual({
      sessionId: 'parent',
      dialogTurnId: 'turn',
      toolCallId: 'tool',
    });
  });

  it('keeps camelCase subagent parent metadata intact', () => {
    expect(
      normalizeSubagentParentInfo({
        subagentParentInfo: {
          sessionId: 'parent',
          dialogTurnId: 'turn',
          toolCallId: 'tool',
        },
      }),
    ).toEqual({
      sessionId: 'parent',
      dialogTurnId: 'turn',
      toolCallId: 'tool',
    });
  });
});

describe('shouldProcessEvent', () => {
  const mockSessionId = 'test-session';
  const mockTurnId = 'test-turn';

  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    stateMachineManager.clear();
  });

  it('returns false for data event when no state machine exists', () => {
    expect(
      shouldProcessEvent(mockSessionId, mockTurnId, 'data', 'TextChunk'),
    ).toBe(false);
  });

  it('returns true for state_sync event even when no state machine exists', () => {
    expect(
      shouldProcessEvent(mockSessionId, mockTurnId, 'state_sync', 'SessionStateChanged'),
    ).toBe(true);
  });

  it('returns true for control event when state is IDLE', () => {
    vi.spyOn(stateMachineManager, 'get').mockReturnValue({
      getCurrentState: () => SessionExecutionState.IDLE,
      getContext: () => ({ currentDialogTurnId: mockTurnId }),
    } as any);

    expect(
      shouldProcessEvent(mockSessionId, mockTurnId, 'control', 'DialogTurnStarted'),
    ).toBe(true);
  });

  it('returns false for control event when state is PROCESSING', () => {
    vi.spyOn(stateMachineManager, 'get').mockReturnValue({
      getCurrentState: () => SessionExecutionState.PROCESSING,
      getContext: () => ({ currentDialogTurnId: mockTurnId }),
    } as any);

    expect(
      shouldProcessEvent(mockSessionId, mockTurnId, 'control', 'DialogTurnStarted'),
    ).toBe(false);
  });

  it('returns false for data event when state is not streaming', () => {
    vi.spyOn(stateMachineManager, 'get').mockReturnValue({
      getCurrentState: () => SessionExecutionState.IDLE,
      getContext: () => ({ currentDialogTurnId: mockTurnId }),
    } as any);

    expect(
      shouldProcessEvent(mockSessionId, mockTurnId, 'data', 'TextChunk'),
    ).toBe(false);
  });

  it('returns false for data event when turn ID mismatches', () => {
    vi.spyOn(stateMachineManager, 'get').mockReturnValue({
      getCurrentState: () => SessionExecutionState.PROCESSING,
      getContext: () => ({ currentDialogTurnId: 'different-turn' }),
    } as any);

    expect(
      shouldProcessEvent(mockSessionId, mockTurnId, 'data', 'TextChunk'),
    ).toBe(false);
  });

  it('returns true for data event when all conditions match', () => {
    vi.spyOn(stateMachineManager, 'get').mockReturnValue({
      getCurrentState: () => SessionExecutionState.PROCESSING,
      getContext: () => ({ currentDialogTurnId: mockTurnId }),
    } as any);

    expect(
      shouldProcessEvent(mockSessionId, mockTurnId, 'data', 'TextChunk'),
    ).toBe(true);
  });
});

describe('formatDialogErrorForNotification', () => {
  it('shows friendly copy while preserving raw error details for diagnostics', () => {
    const rawError = 'Provider error: code=invalid_request_error, request_id=req-1, message=bad payload';
    const formatted = formatDialogErrorForNotification(rawError, {
      category: 'invalid_request',
      provider: 'openai',
      providerCode: 'invalid_request_error',
      requestId: 'req-1',
      rawMessage: rawError,
    });

    expect(formatted.type).toBe('error');
    expect(formatted.title).toBe('Model request invalid');
    expect(formatted.message).not.toContain('Provider error');
    expect(formatted.rawError).toBe(rawError);
    expect(formatted.metadata?.aiError?.rawError).toBe(rawError);
    expect(formatted.metadata?.aiError?.diagnostics).toContain('code=invalid_request_error');
    expect(formatted.actions?.map((action) => action.label)).toContain('Copy diagnostics');
  });
});

function resetFlowChatStore(): void {
  FlowChatStore.getInstance().setState(() => ({
    sessions: new Map(),
    activeSessionId: null,
  }));
}

function makeRound(id: string, items: ModelRound['items'] = []): ModelRound {
  return {
    id,
    index: 0,
    items,
    isStreaming: true,
    isComplete: false,
    status: 'streaming',
    startTime: 1000,
  };
}

function createSessionWithTurn(turn: DialogTurn): void {
  const store = FlowChatStore.getInstance();
  store.createSession('session-1', {});
  store.addDialogTurn('session-1', turn);
}

function createFinishingTurn(): DialogTurn {
  return {
    id: 'turn-1',
    sessionId: 'session-1',
    userMessage: {
      id: 'user-1',
      content: 'Initial request',
      timestamp: 900,
    },
    modelRounds: [{
      ...makeRound('round-1'),
      items: [],
    }],
    status: 'finishing',
    startTime: 900,
  };
}

function createFinishingSession(): Session {
  return {
    sessionId: 'session-1',
    title: 'Session 1',
    dialogTurns: [createFinishingTurn()],
    status: 'idle',
    config: { agentType: 'agentic' },
    createdAt: 800,
    lastActiveAt: 1000,
    error: null,
    isTransient: true,
  };
}

function createFlowChatContext(): FlowChatContext {
  return {
    flowChatStore: FlowChatStore.getInstance(),
    processingManager: {
      clearSessionStatus: vi.fn(),
    } as any,
    eventBatcher: {
      getBufferSize: vi.fn(() => 0),
      flushNow: vi.fn(),
      clear: vi.fn(),
    } as any,
    pendingTurnCompletions: new Map(),
    pendingHistoryLoads: new Map(),
    contentBuffers: new Map(),
    activeTextItems: new Map(),
    saveDebouncers: new Map(),
    lastSaveTimestamps: new Map(),
    lastSaveHashes: new Map(),
    turnSaveInFlight: new Map(),
    turnSavePending: new Set(),
    runtimeStatusTimers: new Map(),
    userCancelledSessionIds: new Set(),
    handledTerminalTurnEvents: new Set(),
    currentWorkspacePath: null,
  };
}

async function setFinishingMachine(): Promise<void> {
  await stateMachineManager.transition('session-1', SessionExecutionEvent.START, {
    taskId: 'session-1',
    dialogTurnId: 'turn-1',
  });
  await stateMachineManager.transition('session-1', SessionExecutionEvent.BACKEND_STREAM_COMPLETED);
}

function putFinishingSessionInStore(): void {
  FlowChatStore.getInstance().setState(() => ({
    sessions: new Map([['session-1', createFinishingSession()]]),
    activeSessionId: 'session-1',
  }));
}

describe('insertSteeringItemIfAbsent', () => {
  beforeEach(() => {
    resetFlowChatStore();
  });

  afterEach(() => {
    resetFlowChatStore();
  });

  it('inserts a visible steering item even before the first model round starts', () => {
    createSessionWithTurn({
      id: 'turn-1',
      sessionId: 'session-1',
      userMessage: {
        id: 'user-1',
        content: 'Initial request',
        timestamp: 900,
      },
      modelRounds: [],
      status: 'processing',
      startTime: 900,
    });

    const inserted = insertSteeringItemIfAbsent({
      sessionId: 'session-1',
      turnId: 'turn-1',
      steeringId: 'steer-1',
      content: 'Please adjust this now',
      status: 'pending',
    });

    const turn = FlowChatStore.getInstance()
      .getState()
      .sessions.get('session-1')
      ?.dialogTurns.find(item => item.id === 'turn-1');

    expect(inserted).toBe(true);
    expect(turn?.modelRounds).toHaveLength(1);
    expect(turn?.modelRounds[0]?.items[0]).toMatchObject({
      id: 'steering_steer-1',
      type: 'user-steering',
      content: 'Please adjust this now',
      status: 'pending',
    });
  });

  it('dedupes an existing steering item across all rounds when backend confirms it', () => {
    const pendingSteering: FlowUserSteeringItem = {
      id: 'steering_steer-1',
      type: 'user-steering',
      steeringId: 'steer-1',
      content: 'Original steering',
      roundIndex: 0,
      timestamp: 1001,
      status: 'pending',
    };
    createSessionWithTurn({
      id: 'turn-1',
      sessionId: 'session-1',
      userMessage: {
        id: 'user-1',
        content: 'Initial request',
        timestamp: 900,
      },
      modelRounds: [
        makeRound('round-1', [pendingSteering]),
        makeRound('round-2'),
      ],
      status: 'processing',
      startTime: 900,
    });

    const inserted = insertSteeringItemIfAbsent({
      sessionId: 'session-1',
      turnId: 'turn-1',
      steeringId: 'steer-1',
      content: 'Original steering',
      roundIndex: 1,
      status: 'completed',
    });

    const rounds = FlowChatStore.getInstance()
      .getState()
      .sessions.get('session-1')
      ?.dialogTurns.find(item => item.id === 'turn-1')
      ?.modelRounds ?? [];
    const steeringItems = rounds.flatMap(round =>
      round.items.filter(item => item.type === 'user-steering'),
    );

    expect(inserted).toBe(false);
    expect(steeringItems).toHaveLength(1);
    expect(steeringItems[0]).toMatchObject({
      id: 'steering_steer-1',
      status: 'completed',
      roundIndex: 1,
    });
  });
});

describe('handleSessionStateChanged', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    resetFlowChatStore();
    stateMachineManager.clear();
  });

  afterEach(() => {
    resetFlowChatStore();
    stateMachineManager.clear();
  });

  it('finalizes pending turn completion when backend reports idle during finishing', async () => {
    putFinishingSessionInStore();
    const context = createFlowChatContext();
    context.pendingTurnCompletions.set('session-1', {
      turnId: 'turn-1',
      lastActivityAt: Date.now(),
      timer: null,
    });
    await setFinishingMachine();

    handleSessionStateChanged(context, { sessionId: 'session-1', newState: 'Idle' });

    const turn = FlowChatStore.getInstance()
      .getState()
      .sessions.get('session-1')
      ?.dialogTurns[0];
    expect(turn?.status).toBe('completed');
    expect(context.pendingTurnCompletions.has('session-1')).toBe(false);
    expect(stateMachineManager.getCurrentState('session-1')).toBe(SessionExecutionState.IDLE);
  });

  it('finalizes a finishing turn even if the pending completion record was lost', async () => {
    putFinishingSessionInStore();
    const context = createFlowChatContext();
    await setFinishingMachine();

    handleSessionStateChanged(context, { sessionId: 'session-1', newState: 'Idle' });

    const turn = FlowChatStore.getInstance()
      .getState()
      .sessions.get('session-1')
      ?.dialogTurns[0];
    expect(turn?.status).toBe('completed');
    expect(stateMachineManager.getCurrentState('session-1')).toBe(SessionExecutionState.IDLE);
  });
});

import { afterEach, describe, expect, it } from 'vitest';
import { flowChatStore } from '../store/FlowChatStore';
import { stateMachineManager } from '../state-machine/SessionStateMachineManager';
import { SessionExecutionEvent } from '../state-machine/types';
import type { DialogTurn, Session } from '../types/flow-chat';
import {
  findPendingAskUserQuestion,
  hasPendingAskUserQuestion,
  resolveTrackedTurn,
} from './askUserQuestionState';

function resetState(): void {
  flowChatStore.setState(() => ({
    sessions: new Map(),
    activeSessionId: null,
  }));
  stateMachineManager.clear();
}

function createAskUserQuestionTool(turnId: string, status: string): DialogTurn {
  return {
    id: turnId,
    sessionId: 'session-1',
    userMessage: {
      id: `user-${turnId}`,
      content: 'Help me',
      timestamp: 1000,
    },
    modelRounds: [{
      id: 'round-1',
      index: 0,
      items: [{
        id: 'tool-1',
        type: 'tool',
        toolName: 'AskUserQuestion',
        timestamp: 1500,
        status: status as any,
        toolCall: {
          id: 'tool-call-1',
          input: {
            questions: [{
              header: 'Auth method',
              question: 'Which library should we use?',
              options: [
                { label: 'date-fns', description: 'Lightweight' },
                { label: 'moment', description: 'Legacy' },
              ],
            }],
          },
        },
        requiresConfirmation: false,
        isParamsStreaming: false,
      }],
      isStreaming: false,
      isComplete: false,
      status: 'running',
      startTime: 1500,
    }],
    status: 'processing',
    startTime: 1000,
  };
}

function createQueuedTurn(turnId: string): DialogTurn {
  return {
    id: turnId,
    sessionId: 'session-1',
    userMessage: {
      id: `user-${turnId}`,
      content: 'Follow-up question',
      timestamp: 2000,
    },
    modelRounds: [],
    status: 'pending',
    startTime: 2000,
  };
}

function createSessionWithTwoTurns(): Session {
  return {
    sessionId: 'session-1',
    title: 'Test Session',
    dialogTurns: [
      createAskUserQuestionTool('turn-A', 'running'),
      createQueuedTurn('turn-B'),
    ],
    status: 'idle',
    config: { agentType: 'agentic' },
    createdAt: 900,
    lastActiveAt: 2000,
    updatedAt: 2000,
    error: null,
    isTransient: false,
  };
}

describe('resolveTrackedTurn', () => {
  afterEach(() => {
    resetState();
  });

  it('returns the tracked turn (by currentDialogTurnId), not the last turn', async () => {
    const session = createSessionWithTwoTurns();
    flowChatStore.setState(() => ({
      sessions: new Map([['session-1', session]]),
      activeSessionId: 'session-1',
    }));
    // State machine tracks turn-A (the one with pending AskUserQuestion)
    await stateMachineManager.transition('session-1', SessionExecutionEvent.START, {
      taskId: 'session-1',
      dialogTurnId: 'turn-A',
    });

    const tracked = resolveTrackedTurn(session);

    expect(tracked?.id).toBe('turn-A');
  });

  it('detects pending AskUserQuestion via tracked turn even when a newer turn is queued', async () => {
    const session = createSessionWithTwoTurns();
    flowChatStore.setState(() => ({
      sessions: new Map([['session-1', session]]),
      activeSessionId: 'session-1',
    }));
    await stateMachineManager.transition('session-1', SessionExecutionEvent.START, {
      taskId: 'session-1',
      dialogTurnId: 'turn-A',
    });

    // Last turn is turn-B (no AskUserQuestion), but tracked turn is turn-A
    const lastTurn = session.dialogTurns[session.dialogTurns.length - 1];
    expect(hasPendingAskUserQuestion(lastTurn)).toBe(false);

    const trackedTurn = resolveTrackedTurn(session);
    expect(hasPendingAskUserQuestion(trackedTurn)).toBe(true);
  });

  it('returns false for pending AskUserQuestion once the tool is completed', async () => {
    const session = createSessionWithTwoTurns();
    flowChatStore.setState(() => ({
      sessions: new Map([['session-1', session]]),
      activeSessionId: 'session-1',
    }));
    await stateMachineManager.transition('session-1', SessionExecutionEvent.START, {
      taskId: 'session-1',
      dialogTurnId: 'turn-A',
    });

    // Before resolving: pending AskUserQuestion detected
    expect(hasPendingAskUserQuestion(resolveTrackedTurn(session))).toBe(true);

    // Resolve: mark the tool as completed
    session.dialogTurns[0].modelRounds[0].items[0].status = 'completed';

    // After resolving: no longer pending
    expect(hasPendingAskUserQuestion(resolveTrackedTurn(session))).toBe(false);
  });

  it('falls back to the last turn when no state machine exists', () => {
    const session = createSessionWithTwoTurns();

    // No state machine set up — should fall back to last turn (turn-B)
    const tracked = resolveTrackedTurn(session);
    expect(tracked?.id).toBe('turn-B');
    expect(hasPendingAskUserQuestion(tracked)).toBe(false);
  });

  it('falls back to the last turn when currentDialogTurnId does not match any turn', async () => {
    const session = createSessionWithTwoTurns();
    flowChatStore.setState(() => ({
      sessions: new Map([['session-1', session]]),
      activeSessionId: 'session-1',
    }));
    // State machine tracks a turn that doesn't exist in the session
    await stateMachineManager.transition('session-1', SessionExecutionEvent.START, {
      taskId: 'session-1',
      dialogTurnId: 'turn-X',
    });

    const tracked = resolveTrackedTurn(session);
    expect(tracked?.id).toBe('turn-B');
  });
});

describe('findPendingAskUserQuestion', () => {
  it('returns the tool item when found', () => {
    const turn = createAskUserQuestionTool('turn-A', 'running');
    const item = findPendingAskUserQuestion(turn);
    expect(item).toBeDefined();
    expect(item?.toolName).toBe('AskUserQuestion');
  });

  it('returns undefined for a turn without AskUserQuestion', () => {
    const turn = createQueuedTurn('turn-B');
    expect(findPendingAskUserQuestion(turn)).toBeUndefined();
  });
});

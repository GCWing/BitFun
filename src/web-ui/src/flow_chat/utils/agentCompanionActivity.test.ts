import { afterEach, describe, expect, it } from 'vitest';
import { flowChatStore } from '../store/FlowChatStore';
import { stateMachineManager } from '../state-machine/SessionStateMachineManager';
import { ProcessingPhase, SessionExecutionEvent, SessionExecutionState } from '../state-machine/types';
import type { DialogTurn, Session } from '../types/flow-chat';
import { buildAgentCompanionActivity } from './agentCompanionActivity';

function resetState(): void {
  flowChatStore.setState(() => ({
    sessions: new Map(),
    activeSessionId: null,
  }));
  stateMachineManager.clear();
}

function createTurn(status: DialogTurn['status']): DialogTurn {
  return {
    id: 'turn-1',
    sessionId: 'session-1',
    userMessage: {
      id: 'user-1',
      content: 'Help me',
      timestamp: 1000,
    },
    modelRounds: [],
    status,
    startTime: 1000,
    endTime: status === 'completed' ? 2000 : undefined,
  };
}

function createSession(turnStatus: DialogTurn['status']): Session {
  return {
    sessionId: 'session-1',
    title: 'Remote Task',
    dialogTurns: [createTurn(turnStatus)],
    status: 'idle',
    config: { agentType: 'agentic' },
    createdAt: 900,
    lastActiveAt: 2000,
    updatedAt: 2000,
    error: null,
    isTransient: false,
  };
}

async function putStateMachineInFinishing(): Promise<void> {
  await stateMachineManager.transition('session-1', SessionExecutionEvent.START, {
    taskId: 'session-1',
    dialogTurnId: 'turn-1',
  });
  await stateMachineManager.transition('session-1', SessionExecutionEvent.BACKEND_STREAM_COMPLETED);
}

describe('buildAgentCompanionActivity', () => {
  afterEach(() => {
    resetState();
  });

  it('keeps showing finishing while the tracked turn is still finishing', async () => {
    flowChatStore.setState(() => ({
      sessions: new Map([['session-1', createSession('finishing')]]),
      activeSessionId: 'session-1',
    }));
    await putStateMachineInFinishing();

    const activity = buildAgentCompanionActivity();

    expect(activity.tasks).toHaveLength(1);
    expect(activity.tasks[0]).toMatchObject({
      sessionId: 'session-1',
      labelKey: 'agentCompanion.activity.finishing',
    });
  });

  it('drops a stale finishing machine once the tracked turn is completed', async () => {
    flowChatStore.setState(() => ({
      sessions: new Map([['session-1', createSession('completed')]]),
      activeSessionId: 'session-1',
    }));
    await putStateMachineInFinishing();

    const snapshot = stateMachineManager.getSnapshot('session-1');
    expect(snapshot?.currentState).toBe(SessionExecutionState.FINISHING);
    expect(snapshot?.context.processingPhase).toBe(ProcessingPhase.FINALIZING);

    expect(buildAgentCompanionActivity()).toEqual({
      mood: 'rest',
      tasks: [],
    });
  });
});

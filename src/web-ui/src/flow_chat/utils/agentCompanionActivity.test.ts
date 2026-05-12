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

function createStreamingSessionWithText(content: string): Session {
  const turn = createTurn('processing');
  turn.modelRounds = [{
    id: 'round-1',
    index: 0,
    items: [{
      id: 'text-1',
      type: 'text',
      timestamp: 1500,
      status: 'streaming',
      content,
      isStreaming: true,
    }],
    isStreaming: true,
    isComplete: false,
    status: 'streaming',
    startTime: 1500,
  }];

  return {
    ...createSession('processing'),
    dialogTurns: [turn],
  };
}

function createCompletedSessionWithText(content: string): Session {
  const turn = createTurn('completed');
  turn.modelRounds = [{
    id: 'round-1',
    index: 0,
    items: [{
      id: 'text-1',
      type: 'text',
      timestamp: 1500,
      status: 'completed',
      content,
      isStreaming: false,
    }],
    isStreaming: false,
    isComplete: true,
    status: 'completed',
    startTime: 1500,
    endTime: 2000,
  }];

  return {
    ...createSession('completed'),
    dialogTurns: [turn],
    hasUnreadCompletion: 'completed',
    lastFinishedAt: 2100,
  };
}

async function putStateMachineInFinishing(): Promise<void> {
  await stateMachineManager.transition('session-1', SessionExecutionEvent.START, {
    taskId: 'session-1',
    dialogTurnId: 'turn-1',
  });
  await stateMachineManager.transition('session-1', SessionExecutionEvent.BACKEND_STREAM_COMPLETED);
}

async function putStateMachineInStreaming(): Promise<void> {
  await stateMachineManager.transition('session-1', SessionExecutionEvent.START, {
    taskId: 'session-1',
    dialogTurnId: 'turn-1',
  });
  await stateMachineManager.transition('session-1', SessionExecutionEvent.TEXT_CHUNK_RECEIVED);
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

  it('keeps the latest output source anchored to the newest text', async () => {
    const oldText = 'Older context that should not stay visible in the companion bubble. ';
    const latestText = 'Newest streaming words should keep appearing normally at the end';
    flowChatStore.setState(() => ({
      sessions: new Map([['session-1', createStreamingSessionWithText(oldText.repeat(4) + latestText)]]),
      activeSessionId: 'session-1',
    }));
    await putStateMachineInStreaming();

    const activity = buildAgentCompanionActivity();

    expect(activity.tasks[0].latestOutput).toContain(latestText);
    expect(activity.tasks[0].latestOutput?.endsWith('...')).toBe(false);
  });

  it('keeps the final assistant output visible after completion', () => {
    const finalText = 'Final analysis summary remains visible in the companion bubble.';
    flowChatStore.setState(() => ({
      sessions: new Map([['session-1', createCompletedSessionWithText(finalText)]]),
      activeSessionId: 'session-1',
    }));

    const activity = buildAgentCompanionActivity();

    expect(activity.tasks[0]).toMatchObject({
      state: 'completed',
      latestOutput: finalText,
    });
  });
});

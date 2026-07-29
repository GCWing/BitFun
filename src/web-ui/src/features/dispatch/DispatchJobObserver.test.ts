/**
 * @vitest-environment jsdom
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  dispatchEventId,
  installDispatchJobObserver,
  projectDispatchAgentEvent,
  requestDispatchJobRefresh,
} from './DispatchJobObserver';
import { dispatchJobStore } from './dispatchJobStore';
import type { DispatchEvent, DispatchStatusResponse } from './types';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { stateMachineManager } from '@/flow_chat/state-machine';
import {
  SessionExecutionEvent,
  SessionExecutionState,
} from '@/flow_chat/state-machine/types';
import { scheduleModelResponseStatus } from '@/flow_chat/services/flow-chat-manager/RuntimeStatusModule';
import {
  getRuntimeStatus,
  showRuntimeStatus,
  useRuntimeStatusStore,
} from '@/flow_chat/store/runtimeStatusStore';

const mocks = vi.hoisted(() => ({
  listJobs: vi.fn(),
  status: vi.fn(),
  dispatchExternal: vi.fn(),
}));

vi.mock('./dispatchApi', () => ({
  dispatchApi: {
    listJobs: mocks.listJobs,
    status: mocks.status,
  },
}));

vi.mock('@/infrastructure/peer-device/peerModeFlag', () => ({
  isPeerDeviceModeActive: () => false,
}));

vi.mock('@/flow_chat/services/AgenticEventListener', () => ({
  agenticEventListener: {
    dispatchExternal: mocks.dispatchExternal,
  },
}));

function registerRunningJob(): void {
  dispatchJobStore.getState().registerJob({
    jobId: 'job-1',
    sessionId: 'session-1',
    targetRequest: {
      kind: 'ssh',
      connectionId: 'ssh-1',
      workspacePath: '/repo',
    },
    target: {
      kind: 'ssh',
      connectionId: 'ssh-1',
      workspacePath: '/repo',
      displayName: 'build-host',
    },
    sourceWorkspacePath: '/source',
    title: 'Dispatch test',
    agentType: 'agentic',
    approvalPolicy: 'reject-and-report',
    workspaceDelivery: { kind: 'existing' },
    cursor: 0,
    state: 'running',
    appliedEventIds: [],
    pendingPermissions: [],
    eventLogComplete: true,
    historyTruncated: false,
    omittedEventCount: 0,
    createdAt: 1,
    updatedAt: 1,
  });
}

function createContext() {
  const sessions = new Map<string, any>([
    ['session-1', {
      sessionId: 'session-1',
      config: {
        dispatchTarget: {
          kind: 'ssh',
          connectionId: 'ssh-1',
          workspacePath: '/repo',
          displayName: 'build-host',
        },
        dispatchJobId: 'job-1',
        dispatchCursor: 0,
      },
    }],
  ]);
  return {
    currentWorkspacePath: '/source',
    flowChatStore: {
      getState: vi.fn(() => ({ sessions })),
      addExternalSession: vi.fn(),
      updateSessionDispatchTarget: vi.fn(),
      applyDispatchSnapshot: vi.fn((
        sessionId: string,
        snapshot: { cursor: number; state: string },
      ) => {
        const session = sessions.get(sessionId);
        session.config.dispatchCursor = snapshot.cursor;
        session.config.dispatchJobState = snapshot.state;
        return { applied: true, cursor: snapshot.cursor };
      }),
      markSessionUnreadCompletion: vi.fn(),
    },
    eventBatcher: {
      flushNow: vi.fn(),
    },
  } as any;
}

function createTerminalContext() {
  const processingManager = {
    clearSessionStatus: vi.fn(),
  };
  return {
    currentWorkspacePath: '/source',
    flowChatStore,
    processingManager,
    eventBatcher: {
      flushNow: vi.fn(),
      getBufferSize: vi.fn(() => 0),
    },
    pendingTurnCompletions: new Map(),
    pendingHistoryLoads: new Map(),
    contentBuffers: new Map([
      ['session-1', new Map([['round-1', 'partial']])],
    ]),
    activeTextItems: new Map([
      ['session-1', new Map([['round-1', 'text-1']])],
    ]),
    saveDebouncers: new Map(),
    lastSaveTimestamps: new Map(),
    lastSaveHashes: new Map(),
    turnSaveInFlight: new Map(),
    turnSavePending: new Set(),
    runtimeStatusTimers: new Map(),
    userCancelledSessionIds: new Set(),
    handledTerminalTurnEvents: new Set(),
  } as any;
}

function installProcessingProjection(): void {
  const session = {
    sessionId: 'session-1',
    title: 'Dispatch test',
    dialogTurns: [{
      id: 'turn-1',
      sessionId: 'session-1',
      userMessage: {
        id: 'user-1',
        content: 'run task',
        timestamp: 1,
      },
      modelRounds: [{
        id: 'round-1',
        index: 0,
        items: [
          {
            id: 'text-1',
            type: 'text',
            content: 'partial output',
            status: 'streaming',
            isStreaming: true,
            timestamp: 1,
          },
          {
            id: 'tool-1',
            type: 'tool',
            toolName: 'Bash',
            toolCall: {
              id: 'tool-1',
              input: {},
            },
            status: 'running',
            requiresConfirmation: false,
            isParamsStreaming: true,
            startTime: 1,
            timestamp: 1,
          },
        ],
        isStreaming: true,
        isComplete: false,
        status: 'streaming',
        startTime: 1,
      }],
      status: 'processing',
      startTime: 1,
    }],
    status: 'idle',
    config: {
      agentType: 'agentic',
      dispatchTargetRequest: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
      },
      dispatchTarget: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/repo',
        displayName: 'build-host',
      },
      dispatchJobId: 'job-1',
      dispatchApprovalPolicy: 'reject-and-report',
      dispatchJobState: 'running',
      dispatchCursor: 0,
    },
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    historyState: 'ready',
    mode: 'agentic',
    workspacePath: '/source',
    projectWorkspacePath: '/source',
    sessionKind: 'normal',
  };
  flowChatStore.setState(() => ({
    sessions: new Map([['session-1', session as any]]),
    activeSessionId: 'session-1',
  }));
}

function status(
  overrides: Partial<DispatchStatusResponse> = {},
): DispatchStatusResponse {
  return {
    state: 'running',
    cursor: 0,
    events: [],
    pendingPermissions: [],
    cursorReset: false,
    historyTruncated: false,
    eventLogComplete: true,
    omittedEventCount: 0,
    ...overrides,
  };
}

describe('DispatchJobObserver', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    dispatchJobStore.getState().clear();
    useRuntimeStatusStore.getState().reset();
    flowChatStore.setState(() => ({
      sessions: new Map(),
      activeSessionId: null,
    }));
    useRuntimeStatusStore.getState().reset();
    stateMachineManager.clear();
    mocks.listJobs.mockReset().mockResolvedValue([]);
    mocks.status.mockReset();
    mocks.dispatchExternal.mockReset().mockReturnValue(true);
  });

  afterEach(() => {
    Object.defineProperty(document, 'visibilityState', {
      configurable: true,
      value: 'visible',
    });
    stateMachineManager.clear();
    flowChatStore.setState(() => ({
      sessions: new Map(),
      activeSessionId: null,
    }));
    vi.useRealTimers();
  });

  it('projects raw target events into the existing frontend event contract', () => {
    const projected = projectDispatchAgentEvent({
      type: 'agentEvent',
      timestamp: '2026-07-28T00:00:00Z',
      event: {
        id: 'event-1',
        event: {
          type: 'TextChunk',
          session_id: 'session-1',
          turn_id: 'turn-1',
          round_id: 'round-1',
          text: 'hello',
        },
      },
    });

    expect(projected).toEqual({
      eventName: 'agentic://text-chunk',
      envelopeId: 'event-1',
      payload: {
        sessionId: 'session-1',
        turnId: 'turn-1',
        roundId: 'round-1',
        text: 'hello',
      },
    });
  });

  it('ignores subagent links until child dispatch projections have an owner', () => {
    expect(projectDispatchAgentEvent({
      type: 'agentEvent',
      timestamp: '2026-07-28T00:00:00Z',
      event: {
        id: 'event-child',
        frontendEventName: 'agentic://subagent-session-linked',
        frontendPayload: {
          parentSessionId: 'session-1',
          childSessionId: 'child-1',
        },
        event: {
          type: 'SubagentSessionLinked',
          parent_session_id: 'session-1',
          child_session_id: 'child-1',
        },
      },
    })).toBeNull();
  });

  it('keeps the cursor until an event applies, then deduplicates it on replay', async () => {
    registerRunningJob();
    const event: DispatchEvent = {
      type: 'agentEvent',
      timestamp: '2026-07-28T00:00:00Z',
      event: {
        id: 'event-1',
        frontendEventName: 'agentic://text-chunk',
        frontendPayload: {
          sessionId: 'session-1',
          turnId: 'turn-1',
          roundId: 'round-1',
          text: 'hello',
        },
      },
    };
    mocks.status.mockResolvedValue(status({
      cursor: 25,
      events: [event],
    }));
    mocks.dispatchExternal
      .mockReturnValueOnce(false)
      .mockReturnValueOnce(true);
    const cleanup = installDispatchJobObserver(createContext());

    await vi.advanceTimersByTimeAsync(0);
    expect(dispatchJobStore.getState().jobs['job-1'].cursor).toBe(0);
    expect(mocks.dispatchExternal).toHaveBeenCalledTimes(1);

    requestDispatchJobRefresh('job-1');
    await vi.advanceTimersByTimeAsync(0);
    expect(dispatchJobStore.getState().jobs['job-1'].cursor).toBe(25);
    expect(dispatchJobStore.getState().hasAppliedEvent('job-1', dispatchEventId(event))).toBe(true);
    expect(mocks.dispatchExternal).toHaveBeenCalledTimes(2);

    requestDispatchJobRefresh('job-1');
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.dispatchExternal).toHaveBeenCalledTimes(2);
    cleanup();
  });

  it('drains every terminal page before it stops polling', async () => {
    registerRunningJob();
    mocks.status
      .mockResolvedValueOnce(status({
        state: 'succeeded',
        cursor: 12,
        events: [{
          type: 'jobState',
          timestamp: '2026-07-28T00:00:00Z',
          state: 'succeeded',
        }],
      }))
      .mockResolvedValueOnce(status({
        state: 'succeeded',
        cursor: 12,
        events: [],
      }));
    const cleanup = installDispatchJobObserver(createContext());

    await vi.advanceTimersByTimeAsync(0);
    expect(dispatchJobStore.getState().jobs['job-1']).toMatchObject({
      state: 'succeeded',
      cursor: 12,
      terminalDrained: false,
    });

    requestDispatchJobRefresh('job-1');
    await vi.advanceTimersByTimeAsync(0);
    expect(dispatchJobStore.getState().jobs['job-1'].terminalDrained).toBe(true);

    requestDispatchJobRefresh('job-1');
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.status).toHaveBeenCalledTimes(2);
    cleanup();
  });

  it('does not query a target job before submit acknowledgement', async () => {
    registerRunningJob();
    dispatchJobStore.getState().updateProgress('job-1', { state: 'submitting' });
    // updateProgress cannot regress running to submitting, so register the true pre-ack shape.
    dispatchJobStore.getState().registerJob({
      ...dispatchJobStore.getState().jobs['job-1'],
      state: 'submitting',
    });
    const cleanup = installDispatchJobObserver(createContext());

    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.status).not.toHaveBeenCalled();
    cleanup();
  });

  it('continues polling while hidden so background notifications can observe changes', async () => {
    registerRunningJob();
    mocks.status.mockResolvedValue(status({ state: 'running' }));
    Object.defineProperty(document, 'visibilityState', {
      configurable: true,
      value: 'hidden',
    });
    const cleanup = installDispatchJobObserver(createContext());

    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.status).toHaveBeenCalledTimes(1);

    cleanup();
    Object.defineProperty(document, 'visibilityState', {
      configurable: true,
      value: 'visible',
    });
  });

  it('does not present target cancellation as a successful completion', async () => {
    registerRunningJob();
    const context = createContext();
    context.userCancelledSessionIds = new Set(['session-1']);
    mocks.status.mockResolvedValue(status({
      state: 'cancelled',
      cursor: 1,
      events: [],
    }));
    const cleanup = installDispatchJobObserver(context);

    await vi.advanceTimersByTimeAsync(0);

    expect(context.flowChatStore.markSessionUnreadCompletion).not.toHaveBeenCalled();
    cleanup();
  });

  it('reports transient target unreachability without terminalizing the job and clears it after recovery', async () => {
    registerRunningJob();
    mocks.status
      .mockRejectedValueOnce(new Error('SSH target is offline'))
      .mockResolvedValueOnce(status({ state: 'running' }));
    const cleanup = installDispatchJobObserver(createContext());

    await vi.advanceTimersByTimeAsync(0);
    expect(dispatchJobStore.getState().jobs['job-1'].state).toBe('running');
    expect(dispatchJobStore.getState().transportByJobId['job-1']).toEqual({
      reachability: 'unreachable',
      lastTransportError: 'SSH target is offline',
    });

    requestDispatchJobRefresh('job-1');
    await vi.advanceTimersByTimeAsync(0);
    expect(dispatchJobStore.getState().jobs['job-1'].state).toBe('running');
    expect(dispatchJobStore.getState().transportByJobId['job-1']).toEqual({
      reachability: 'reachable',
      lastTransportError: undefined,
    });
    cleanup();
  });

  it('settles cancellation after the terminal log drains without a dialog-turn-cancelled event', async () => {
    registerRunningJob();
    installProcessingProjection();
    const context = createTerminalContext();
    await stateMachineManager.transition(
      'session-1',
      SessionExecutionEvent.START,
      { taskId: 'session-1', dialogTurnId: 'turn-1' },
    );
    mocks.status
      .mockResolvedValueOnce(status({
        state: 'cancelled',
        cursor: 12,
        events: [{
          type: 'jobState',
          timestamp: '2026-07-29T00:00:00Z',
          state: 'cancelled',
        }],
      }))
      .mockResolvedValueOnce(status({
        state: 'cancelled',
        cursor: 12,
        events: [],
      }));
    const cleanup = installDispatchJobObserver(context);

    await vi.advanceTimersByTimeAsync(0);
    expect(flowChatStore.getState().sessions.get('session-1')?.dialogTurns[0].status)
      .toBe('processing');
    expect(stateMachineManager.getCurrentState('session-1'))
      .toBe(SessionExecutionState.PROCESSING);

    requestDispatchJobRefresh('job-1');
    await vi.advanceTimersByTimeAsync(0);
    const turn = flowChatStore.getState().sessions.get('session-1')?.dialogTurns[0];
    expect(turn).toMatchObject({
      status: 'cancelled',
      modelRounds: [{
        status: 'cancelled',
        isStreaming: false,
        isComplete: true,
        items: [
          { id: 'text-1', status: 'cancelled', isStreaming: false },
          { id: 'tool-1', status: 'cancelled', isParamsStreaming: false },
        ],
      }],
    });
    expect(stateMachineManager.getCurrentState('session-1'))
      .toBe(SessionExecutionState.IDLE);
    expect(context.processingManager.clearSessionStatus)
      .toHaveBeenCalledWith('session-1');
    expect(mocks.dispatchExternal).not.toHaveBeenCalled();
    cleanup();
  });

  it('cancels delayed runtime status rendering when a terminal snapshot drains', async () => {
    registerRunningJob();
    installProcessingProjection();
    const context = createTerminalContext();
    scheduleModelResponseStatus(
      context,
      'session-1',
      'turn-1',
      'round-1',
      { delayMs: 1000 },
    );
    showRuntimeStatus({
      sessionId: 'session-1',
      turnId: 'turn-1',
      roundId: 'round-1',
    });
    expect(context.runtimeStatusTimers.size).toBe(1);
    expect(getRuntimeStatus('session-1')).toBeDefined();
    mocks.status.mockResolvedValue(status({
      state: 'succeeded',
      cursor: 0,
      events: [],
    }));
    const cleanup = installDispatchJobObserver(context);

    await vi.advanceTimersByTimeAsync(0);
    expect(context.runtimeStatusTimers.size).toBe(0);
    expect(getRuntimeStatus('session-1')).toBeUndefined();

    await vi.advanceTimersByTimeAsync(1000);
    expect(getRuntimeStatus('session-1')).toBeUndefined();
    cleanup();
  });

  it('safely settles a queued job that reaches terminal state before any turn event', async () => {
    registerRunningJob();
    installProcessingProjection();
    const queuedSession = flowChatStore.getState().sessions.get('session-1')!;
    flowChatStore.setState(state => ({
      ...state,
      sessions: new Map(state.sessions).set('session-1', {
        ...queuedSession,
        dialogTurns: [],
      }),
    }));
    const context = createTerminalContext();
    mocks.status.mockResolvedValue(status({
      state: 'cancelled',
      cursor: 0,
      events: [],
    }));
    const cleanup = installDispatchJobObserver(context);

    await vi.advanceTimersByTimeAsync(0);
    const session = flowChatStore.getState().sessions.get('session-1');
    expect(session?.dialogTurns).toEqual([]);
    expect(session?.config.dispatchJobState).toBe('cancelled');
    expect(stateMachineManager.getCurrentState('session-1'))
      .toBe(SessionExecutionState.IDLE);
    cleanup();
  });

  it('does not duplicate convergence already owned by a terminal agent event', async () => {
    registerRunningJob();
    installProcessingProjection();
    const session = flowChatStore.getState().sessions.get('session-1')!;
    const terminalTurn = {
      ...session.dialogTurns[0],
      status: 'completed' as const,
      success: true,
      endTime: 5,
      modelRounds: [],
    };
    flowChatStore.setState(state => ({
      ...state,
      sessions: new Map(state.sessions).set('session-1', {
        ...session,
        dialogTurns: [terminalTurn],
      }),
    }));
    const context = createTerminalContext();
    context.handledTerminalTurnEvents.add('session-1:turn-1');
    mocks.status.mockResolvedValue(status({
      state: 'succeeded',
      cursor: 0,
      events: [],
    }));
    const cleanup = installDispatchJobObserver(context);

    await vi.advanceTimersByTimeAsync(0);
    const appliedTurn = flowChatStore
      .getState()
      .sessions
      .get('session-1')!
      .dialogTurns[0];
    expect(appliedTurn).toBe(terminalTurn);
    expect(context.processingManager.clearSessionStatus).not.toHaveBeenCalled();
    expect(stateMachineManager.getCurrentState('session-1'))
      .toBe(SessionExecutionState.IDLE);
    cleanup();
  });

  it.each([
    {
      jobState: 'failed' as const,
      turnState: 'error',
      itemState: 'error',
      lastError: 'Remote execution failed',
    },
    {
      jobState: 'succeeded' as const,
      turnState: 'completed',
      itemState: 'completed',
      lastError: undefined,
    },
  ])('settles $jobState snapshots idempotently after terminal drain', async ({
    jobState,
    turnState,
    itemState,
    lastError,
  }) => {
    registerRunningJob();
    installProcessingProjection();
    const context = createTerminalContext();
    await stateMachineManager.transition(
      'session-1',
      SessionExecutionEvent.START,
      { taskId: 'session-1', dialogTurnId: 'turn-1' },
    );
    mocks.status.mockResolvedValue(status({
      state: jobState,
      cursor: 0,
      events: [],
      lastError,
    }));
    const cleanup = installDispatchJobObserver(context);

    await vi.advanceTimersByTimeAsync(0);
    await Promise.resolve();
    const firstSession = flowChatStore.getState().sessions.get('session-1')!;
    const firstTurn = firstSession.dialogTurns[0];
    expect(firstTurn.status).toBe(turnState);
    expect(firstTurn.modelRounds[0].items[0].status).toBe(itemState);
    expect(stateMachineManager.getCurrentState('session-1'))
      .toBe(SessionExecutionState.IDLE);
    if (jobState === 'failed') {
      expect(firstSession.error).toBe(lastError);
      expect(firstTurn.error).toBe(lastError);
    }

    const repeated = flowChatStore.applyDispatchSnapshot('session-1', {
      jobId: 'job-1',
      state: jobState,
      cursor: 0,
      expectedCursor: 0,
      lastError,
      terminalDrained: true,
    });
    const repeatedTurn = flowChatStore
      .getState()
      .sessions
      .get('session-1')!
      .dialogTurns[0];
    expect(repeated).toEqual({ applied: true, cursor: 0 });
    expect(repeatedTurn).toBe(firstTurn);
    expect(repeatedTurn.endTime).toBe(firstTurn.endTime);
    expect(stateMachineManager.getCurrentState('session-1'))
      .toBe(SessionExecutionState.IDLE);
    cleanup();
  });
});

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { cancelSessionTask, sendMessage, syncSessionModelSelection } from './MessageModule';
import { SessionExecutionEvent } from '../../state-machine/types';
import {
  getRuntimeStatus,
  resetRuntimeStatuses,
} from '../../store/runtimeStatusStore';

const mockTransition = vi.fn();
const mockGetCurrentState = vi.fn(() => 'processing');
const mockGetStateMachine = vi.fn(() => null);
const mockUpdateSessionModel = vi.fn();
const mockStartDialogTurn = vi.fn();
const mockGetConfigs = vi.fn();
const mockDispatchSubmit = vi.fn();
const mockDispatchAppend = vi.fn();
const mockDispatchProgress = vi.fn();
const mockDispatchRefresh = vi.fn();
const mockBindSession = vi.fn();
const mockEnsureBackendSession = vi.fn();
const mockNotificationError = vi.fn();
const mockNotificationDismiss = vi.fn();
const mockPendingList = vi.fn((): unknown[] => []);
const mockPendingEnqueue = vi.fn();

vi.mock('../../state-machine', () => ({
  SessionExecutionEvent: {
    FINISHING_SETTLED: 'finishing_settled',
    USER_CANCEL: 'user_cancel',
  },
  SessionExecutionState: {
    PROCESSING: 'processing',
  },
  stateMachineManager: {
    getCurrentState: (...args: unknown[]) => mockGetCurrentState(...args),
    get: (...args: unknown[]) => mockGetStateMachine(...args),
    transition: (...args: any[]) => mockTransition(...args),
  },
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: {
    startDialogTurn: (...args: unknown[]) => mockStartDialogTurn(...args),
    updateSessionModel: (...args: unknown[]) => mockUpdateSessionModel(...args),
  },
}));

vi.mock('@/infrastructure/api/service-api/WorktreeAPI', () => ({
  worktreeAPI: {
    bindSession: (...args: unknown[]) => mockBindSession(...args),
  },
}));

vi.mock('@/features/dispatch/dispatchApi', () => ({
  dispatchApi: {
    submit: (...args: unknown[]) => mockDispatchSubmit(...args),
    append: (...args: unknown[]) => mockDispatchAppend(...args),
  },
}));

vi.mock('@/features/dispatch/dispatchJobStore', () => ({
  dispatchJobStore: {
    getState: () => ({
      updateProgress: (...args: unknown[]) => mockDispatchProgress(...args),
    }),
  },
}));

vi.mock('@/features/dispatch/DispatchJobObserver', () => ({
  requestDispatchJobRefresh: (...args: unknown[]) => mockDispatchRefresh(...args),
}));

vi.mock('@/infrastructure/api/service-api/ACPClientAPI', () => ({
  ACPClientAPI: {},
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: {
    getConfigs: (...args: unknown[]) => mockGetConfigs(...args),
  },
}));

vi.mock('../../../shared/notification-system', () => ({
  notificationService: {
    error: (...args: unknown[]) => mockNotificationError(...args),
    dismiss: (...args: unknown[]) => mockNotificationDismiss(...args),
  },
}));

vi.mock('./SessionModule', () => ({
  ensureBackendSession: (...args: unknown[]) => mockEnsureBackendSession(...args),
  getModelMaxTokens: vi.fn(async (modelId: string) => modelId === 'auto' ? 32000 : 64000),
  retryCreateBackendSession: vi.fn(),
}));

vi.mock('./PendingQueueModule', () => ({
  pendingQueueManager: {
    list: (...args: unknown[]) => mockPendingList(...args),
    enqueue: (...args: unknown[]) => mockPendingEnqueue(...args),
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: (key: string) => key,
  },
}));

describe('MessageModule session writer conflict', () => {
  function deferred<T>() {
    let resolve!: (value: T | PromiseLike<T>) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
      resolve = resolvePromise;
      reject = rejectPromise;
    });
    return { promise, resolve, reject };
  }

  function conflictContext(sessionId: string) {
    const session = {
      sessionId,
      mode: 'agentic',
      dialogTurns: [] as any[],
      config: { modelName: 'auto' },
      titleStatus: 'generated',
      maxContextTokens: 32000,
    };
    return {
      session,
      context: {
        flowChatStore: {
          getState: () => ({ sessions: new Map([[sessionId, session]]) }),
          addDialogTurn: vi.fn((_id: string, turn: any) => session.dialogTurns.push(turn)),
          deleteDialogTurn: vi.fn((_id: string, turnId: string) => {
            session.dialogTurns = session.dialogTurns.filter(turn => turn.id !== turnId);
          }),
          updateSessionLastSubmittedMode: vi.fn(),
          updateSessionMode: vi.fn(),
          updateSessionModelName: vi.fn(),
          updateSessionMaxContextTokens: vi.fn(),
        },
        processingManager: {
          registerStatus: vi.fn(),
          clearSessionStatus: vi.fn(),
        },
        pendingHistoryLoads: new Map(),
        contentBuffers: new Map(),
        activeTextItems: new Map(),
      } as any,
    };
  }

  beforeEach(() => {
    vi.resetAllMocks();
    mockGetCurrentState.mockReturnValue('idle');
    mockGetStateMachine.mockReturnValue(null);
    mockTransition.mockResolvedValue(true);
    mockUpdateSessionModel.mockResolvedValue(undefined);
    mockStartDialogTurn.mockResolvedValue(undefined);
    mockPendingList.mockReturnValue([]);
    mockPendingEnqueue.mockReturnValue({ id: 'queued-message' });
    mockEnsureBackendSession.mockRejectedValue(
      new Error('session_in_use: Session is already open for writing: session-1'),
    );
    mockNotificationError
      .mockReturnValueOnce('notification-1')
      .mockReturnValueOnce('notification-2')
      .mockReturnValue('notification-3');
  });

  it('keeps only the latest explicit retry for one conflicted session', async () => {
    const session = {
      sessionId: 'session-1',
      mode: 'agentic',
      dialogTurns: [],
      config: {},
      titleStatus: 'generated',
    };
    const context: any = {
      flowChatStore: {
        getState: () => ({ sessions: new Map([['session-1', session]]) }),
        deleteDialogTurn: vi.fn(),
      },
      pendingHistoryLoads: new Map(),
    };

    await expect(sendMessage(context, 'hello', 'session-1')).rejects.toThrow(
      'session_in_use',
    );

    expect(context.flowChatStore.deleteDialogTurn).not.toHaveBeenCalled();
    expect(mockEnsureBackendSession).toHaveBeenCalledTimes(1);
    expect(mockNotificationError).toHaveBeenCalledTimes(1);
    const [message, options] = mockNotificationError.mock.calls[0];
    expect(message).toBe('flow-chat:session.inUseMessage');
    expect(options).toMatchObject({
      title: 'flow-chat:session.inUseTitle',
      duration: 0,
    });
    expect(options.actions).toHaveLength(1);
    expect(options.actions[0].label).toBe('flow-chat:session.retry');

    await expect(sendMessage(context, 'hello', 'session-1')).rejects.toThrow(
      'session_in_use',
    );
    expect(mockNotificationDismiss).toHaveBeenCalledWith('notification-1');
    const staleAction = options.actions[0];
    const latestAction = mockNotificationError.mock.calls[1][1].actions[0];

    staleAction.onClick();
    expect(mockEnsureBackendSession).toHaveBeenCalledTimes(2);

    latestAction.onClick();
    latestAction.onClick();
    await vi.waitFor(() => {
      expect(mockEnsureBackendSession).toHaveBeenCalledTimes(3);
    });
  });

  it('does not let an older retry failure replace a newer conflict', async () => {
    const sessionId = 'session-race-failure';
    const { context } = conflictContext(sessionId);
    const conflict = new Error(`session_in_use: Session is already open for writing: ${sessionId}`);

    await expect(sendMessage(context, 'older', sessionId)).rejects.toThrow('session_in_use');
    const retryAction = mockNotificationError.mock.calls[0][1].actions[0];
    const olderRetry = deferred<void>();
    const newerSend = deferred<void>();
    mockEnsureBackendSession
      .mockImplementationOnce(() => olderRetry.promise)
      .mockImplementationOnce(() => newerSend.promise);

    retryAction.onClick();
    await vi.waitFor(() => expect(mockEnsureBackendSession).toHaveBeenCalledTimes(2));
    const newerResult = sendMessage(context, 'newer', sessionId);
    await vi.waitFor(() => expect(mockEnsureBackendSession).toHaveBeenCalledTimes(3));

    newerSend.reject(conflict);
    await expect(newerResult).rejects.toThrow('session_in_use');
    expect(mockNotificationError).toHaveBeenCalledTimes(2);

    olderRetry.reject(conflict);
    await vi.waitFor(() => expect(mockEnsureBackendSession).toHaveBeenCalledTimes(3));
    await Promise.resolve();

    expect(mockNotificationError).toHaveBeenCalledTimes(2);
    expect(mockNotificationDismiss).not.toHaveBeenCalledWith('notification-2');
  });

  it('does not let an older retry success dismiss a newer conflict', async () => {
    const sessionId = 'session-race-success';
    const { context } = conflictContext(sessionId);
    const conflict = new Error(`session_in_use: Session is already open for writing: ${sessionId}`);
    const retryStart = vi.fn();
    const retrySuccess = vi.fn();

    await expect(sendMessage(context, 'older', sessionId, undefined, undefined, undefined, {
      onSessionConflictRetryStart: retryStart,
      onSessionConflictRetrySuccess: retrySuccess,
    })).rejects.toThrow('session_in_use');
    const retryAction = mockNotificationError.mock.calls[0][1].actions[0];
    const olderRetry = deferred<void>();
    const newerSend = deferred<void>();
    mockEnsureBackendSession
      .mockImplementationOnce(() => olderRetry.promise)
      .mockImplementationOnce(() => newerSend.promise);

    retryAction.onClick();
    expect(retryStart).toHaveBeenCalledTimes(1);
    await vi.waitFor(() => expect(mockEnsureBackendSession).toHaveBeenCalledTimes(2));
    const newerResult = sendMessage(context, 'newer', sessionId);
    await vi.waitFor(() => expect(mockEnsureBackendSession).toHaveBeenCalledTimes(3));

    newerSend.reject(conflict);
    await expect(newerResult).rejects.toThrow('session_in_use');
    olderRetry.resolve(undefined);
    await vi.waitFor(() => expect(mockStartDialogTurn).toHaveBeenCalledTimes(1));

    expect(mockNotificationError).toHaveBeenCalledTimes(2);
    expect(mockNotificationDismiss).not.toHaveBeenCalledWith('notification-2');
    expect(retrySuccess).not.toHaveBeenCalled();
  });

  it('does not let an older retry failure reset a newer processing turn', async () => {
    const sessionId = 'session-processing-race';
    const { context } = conflictContext(sessionId);
    const conflict = new Error(`session_in_use: Session is already open for writing: ${sessionId}`);

    await expect(sendMessage(context, 'older', sessionId)).rejects.toThrow('session_in_use');
    const retryAction = mockNotificationError.mock.calls[0][1].actions[0];
    const olderRetry = deferred<void>();
    const newerTurn = deferred<void>();
    let currentState = 'idle';
    let currentDialogTurnId: string | null = null;
    mockGetCurrentState.mockImplementation(() => currentState);
    mockGetStateMachine.mockReturnValue({
      getContext: () => ({ currentDialogTurnId }),
    } as any);
    mockTransition.mockImplementation(async (_id, event, payload) => {
      if (event === 'start') {
        currentState = 'processing';
        currentDialogTurnId = payload.dialogTurnId;
      }
      return true;
    });
    mockEnsureBackendSession
      .mockImplementationOnce(() => olderRetry.promise)
      .mockResolvedValueOnce(undefined);
    mockStartDialogTurn.mockImplementationOnce(() => newerTurn.promise);

    retryAction.onClick();
    await vi.waitFor(() => expect(mockEnsureBackendSession).toHaveBeenCalledTimes(2));
    const newerResult = sendMessage(context, 'newer', sessionId);
    await vi.waitFor(() => expect(mockStartDialogTurn).toHaveBeenCalledTimes(1));

    olderRetry.reject(conflict);
    await new Promise(resolve => setTimeout(resolve, 0));
    expect(mockTransition).not.toHaveBeenCalledWith(
      sessionId,
      'error_occurred',
      expect.anything(),
    );
    expect(mockTransition).not.toHaveBeenCalledWith(sessionId, 'reset');

    newerTurn.resolve(undefined);
    await expect(newerResult).resolves.toBeUndefined();
  });

  it('invalidates an old retry when a newer message is queued', async () => {
    const sessionId = 'session-queue-success';
    const { context } = conflictContext(sessionId);

    await expect(sendMessage(context, 'older', sessionId)).rejects.toThrow('session_in_use');
    const retryAction = mockNotificationError.mock.calls[0][1].actions[0];
    mockGetCurrentState.mockReturnValue('processing');

    await expect(sendMessage(context, 'newer', sessionId)).resolves.toBeUndefined();
    expect(mockPendingEnqueue).toHaveBeenCalledWith(expect.objectContaining({
      sessionId,
      content: 'newer',
    }));
    expect(mockNotificationDismiss).toHaveBeenCalledWith('notification-1');

    retryAction.onClick();
    expect(mockEnsureBackendSession).toHaveBeenCalledTimes(1);
  });
});

describe('MessageModule cancellation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetRuntimeStatuses();
    mockGetCurrentState.mockReturnValue('processing');
    mockTransition.mockResolvedValue(true);
  });

  it('cancels persistent /btw sessions through the ordinary dialog-turn path', async () => {
    const session = {
      sessionId: 'btw-child',
      isTransient: false,
      sessionKind: 'btw',
      dialogTurns: [],
      config: {},
    };
    const mockStoreCancelSessionTask = vi.fn();
    const contentBuffers = new Map([['btw-child', new Map([['round-1', 'text']])]]);
    const activeTextItems = new Map([['btw-child', new Map([['round-1', 'item-1']])]]);
    const context: any = {
      flowChatStore: {
        getState: () => ({
          activeSessionId: 'parent',
          sessions: new Map([['btw-child', session]]),
        }),
        cancelSessionTask: mockStoreCancelSessionTask,
      },
      userCancelledSessionIds: new Set<string>(),
      eventBatcher: {
        getBufferSize: vi.fn(() => 0),
        clear: vi.fn(),
      },
      pendingTurnCompletions: new Map(),
      runtimeStatusTimers: new Map(),
      handledTerminalTurnEvents: new Set<string>(),
      contentBuffers,
      activeTextItems,
    };

    await expect(cancelSessionTask(context, 'btw-child')).resolves.toBe(true);

    expect(mockTransition).toHaveBeenCalledWith(
      'btw-child',
      SessionExecutionEvent.USER_CANCEL,
    );
    expect(mockStoreCancelSessionTask).not.toHaveBeenCalled();
    expect(context.userCancelledSessionIds.has('btw-child')).toBe(true);
    expect(contentBuffers.has('btw-child')).toBe(false);
    expect(activeTextItems.has('btw-child')).toBe(false);
  });
});

describe('MessageModule detached dispatch', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetCurrentState.mockReturnValue('idle');
    mockDispatchSubmit.mockResolvedValue({
      accepted: true,
      jobId: 'job-1',
      sessionId: 'dispatch-session',
      state: 'queued',
    });
    mockDispatchAppend.mockResolvedValue({
      accepted: true,
      messageId: 'append-1',
    });
  });

  function createDispatchContext(approvalPolicy: 'auto' | 'reject-and-report' | 'remote') {
    const session = {
      sessionId: 'dispatch-session',
      title: 'New Chat',
      titleStatus: 'generated',
      mode: 'agentic',
      dialogTurns: [] as any[],
      workspacePath: '/controller/repo',
      projectWorkspacePath: '/controller/repo',
      workspaceId: 'workspace-1',
      config: {
        workspacePath: '/controller/repo',
        projectWorkspacePath: '/controller/repo',
        workspaceId: 'workspace-1',
        modelName: 'controller-model',
        dispatchTargetRequest: {
          kind: 'ssh',
          connectionId: 'ssh-1',
          workspacePath: '/target/repo',
        },
        dispatchTarget: {
          kind: 'ssh',
          connectionId: 'ssh-1',
          workspacePath: '/target/repo',
          displayName: 'build-host',
        },
        dispatchJobId: 'job-1',
        dispatchApprovalPolicy: approvalPolicy,
        dispatchJobState: 'submitting',
        dispatchCursor: 0,
      },
    };
    return {
      session,
      context: {
        flowChatStore: {
          getState: () => ({
            activeSessionId: session.sessionId,
            sessions: new Map([[session.sessionId, session]]),
          }),
          addDialogTurn: vi.fn((_sessionId: string, turn: any) => {
            if (!session.dialogTurns.some(existing => existing.id === turn.id)) {
              session.dialogTurns.push(turn);
            }
          }),
          deleteDialogTurn: vi.fn((_sessionId: string, turnId: string) => {
            session.dialogTurns = session.dialogTurns.filter(turn => turn.id !== turnId);
          }),
          applyDispatchSnapshot: vi.fn(() => ({ applied: true, cursor: 0 })),
          updateSessionLastSubmittedMode: vi.fn(),
          updateSessionMode: vi.fn(),
        },
        pendingHistoryLoads: new Map(),
      } as any,
    };
  }

  it('projects the user message immediately while the target is still queued', async () => {
    const { context, session } = createDispatchContext('reject-and-report');
    (session.config as any).dispatchWorkspaceDelivery = {
      kind: 'snapshot-source',
      sourceWorkspacePath: '/controller/repo',
    };
    let resolveSubmit!: (value: {
      accepted: boolean;
      jobId: string;
      sessionId: string;
      state: string;
    }) => void;
    mockDispatchSubmit.mockImplementationOnce(() => new Promise(resolve => {
      resolveSubmit = resolve;
    }));

    const submission = sendMessage(
      context,
      'expanded remote prompt',
      'dispatch-session',
      'run remote checks',
    );

    expect(session.dialogTurns).toHaveLength(1);
    expect(session.dialogTurns[0]).toMatchObject({
      id: 'dispatch_pending_job-1',
      sessionId: 'dispatch-session',
      agentType: 'agentic',
      userMessage: {
        content: 'run remote checks',
        metadata: {
          __bitfunOptimisticDispatchJobId: 'job-1',
        },
      },
      modelRounds: [],
      status: 'pending',
    });
    expect(getRuntimeStatus('dispatch-session')).toMatchObject({
      turnId: 'dispatch_pending_job-1',
      roundId: 'dispatch-transfer:job-1',
      label: 'flow-chat:chatInput.dispatch.transferInProgress',
    });
    resolveSubmit({
      accepted: true,
      jobId: 'job-1',
      sessionId: 'dispatch-session',
      state: 'queued',
    });
    await submission;
    expect(getRuntimeStatus('dispatch-session')).toBeUndefined();

    expect(mockDispatchSubmit).toHaveBeenCalledWith({
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/target/repo',
      },
      workspaceDelivery: {
        kind: 'snapshot-source',
        sourceWorkspacePath: '/controller/repo',
      },
      jobId: 'job-1',
      sessionId: 'dispatch-session',
      agentType: 'agentic',
      prompt: 'expanded remote prompt',
      approvalPolicy: 'reject-and-report',
      model: undefined,
      sourceWorkspacePath: '/controller/repo',
      sourceWorkspaceId: 'workspace-1',
    });
    expect(mockStartDialogTurn).not.toHaveBeenCalled();
    expect(mockBindSession).not.toHaveBeenCalled();
  });

  it('removes the optimistic message when dispatch submission fails', async () => {
    const { context, session } = createDispatchContext('reject-and-report');
    mockDispatchSubmit.mockRejectedValueOnce(new Error('target unavailable'));

    await expect(
      sendMessage(context, 'run remote checks', 'dispatch-session'),
    ).rejects.toThrow('target unavailable');

    expect(session.dialogTurns).toEqual([]);
  });

  it('submits with the session-scoped auto-approval setting without another confirmation', async () => {
    const { context } = createDispatchContext('auto');

    await expect(
      sendMessage(context, 'run remote checks', 'dispatch-session'),
    ).resolves.toBeUndefined();
    expect(mockDispatchSubmit).toHaveBeenCalledTimes(1);
  });

  it('reuses the append message id after an ambiguous transport failure', async () => {
    const { context, session } = createDispatchContext('remote');
    session.config.dispatchJobState = 'running';
    mockDispatchAppend
      .mockRejectedValueOnce(new Error('response lost'))
      .mockResolvedValueOnce({ accepted: true, messageId: 'append-1' });

    await expect(
      sendMessage(context, 'continue with the checks', 'dispatch-session'),
    ).rejects.toThrow('response lost');
    await expect(
      sendMessage(context, 'continue with the checks', 'dispatch-session'),
    ).resolves.toBeUndefined();

    expect(mockDispatchAppend).toHaveBeenCalledTimes(2);
    expect(mockDispatchAppend.mock.calls[0][3]).toBeTruthy();
    expect(mockDispatchAppend.mock.calls[1][3]).toBe(
      mockDispatchAppend.mock.calls[0][3],
    );
  });
});

describe('MessageModule model synchronization', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetConfigs.mockResolvedValue({
      'ai.agent_model_defaults': { mode: 'model-b' },
      'ai.models': [
        { id: 'primary-model', enabled: true, context_window: 32000 },
        { id: 'model-b', enabled: true, context_window: 64000 },
      ],
      'ai.default_models': { primary: 'primary-model' },
    });
    mockUpdateSessionModel.mockResolvedValue(undefined);
  });

  it('keeps an explicit auto selector when synchronizing before send', async () => {
    const session = {
      sessionId: 'session-auto',
      config: {
        modelName: 'auto',
        workspacePath: '/remote/repo',
      },
      remoteConnectionId: 'ssh-1',
      remoteSshHost: 'example.test',
      sessionKind: 'subagent',
      maxContextTokens: 64000,
    };
    const updateSessionModelName = vi.fn();
    const updateSessionMaxContextTokens = vi.fn();
    const context: any = {
      flowChatStore: {
        getState: () => ({ sessions: new Map([['session-auto', session]]) }),
        updateSessionModelName,
        updateSessionMaxContextTokens,
      },
    };

    await syncSessionModelSelection(context, 'session-auto', 'agentic');

    expect(updateSessionModelName).not.toHaveBeenCalled();
    expect(updateSessionMaxContextTokens).toHaveBeenCalledWith('session-auto', 32000);
    expect(mockUpdateSessionModel).toHaveBeenCalledWith({
      sessionId: 'session-auto',
      modelName: 'auto',
      workspacePath: '/remote/repo',
      remoteConnectionId: 'ssh-1',
      remoteSshHost: 'example.test',
      includeInternal: true,
    });
  });

  it('migrates a legacy session without a model to the current mode default', async () => {
    const session = {
      sessionId: 'legacy-session',
      config: { workspacePath: '/local/repo' },
      maxContextTokens: 32000,
    };
    const updateSessionModelName = vi.fn();
    const updateSessionMaxContextTokens = vi.fn();
    const context: any = {
      flowChatStore: {
        getState: () => ({ sessions: new Map([['legacy-session', session]]) }),
        updateSessionModelName,
        updateSessionMaxContextTokens,
      },
    };

    await syncSessionModelSelection(context, 'legacy-session', 'agentic');

    expect(updateSessionModelName).toHaveBeenCalledWith('legacy-session', 'model-b');
    expect(updateSessionMaxContextTokens).toHaveBeenCalledWith('legacy-session', 64000);
    expect(mockUpdateSessionModel).toHaveBeenCalledWith({
      sessionId: 'legacy-session',
      modelName: 'model-b',
      workspacePath: '/local/repo',
      remoteConnectionId: undefined,
      remoteSshHost: undefined,
      includeInternal: false,
    });
  });
});

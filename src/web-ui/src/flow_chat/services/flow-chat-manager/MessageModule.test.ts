import { beforeEach, describe, expect, it, vi } from 'vitest';
import { cancelSessionTask, sendMessage, syncSessionModelSelection } from './MessageModule';
import { SessionExecutionEvent } from '../../state-machine/types';

const mockTransition = vi.fn();
const mockUpdateSessionModel = vi.fn();
const mockGetConfigs = vi.fn();
const mockGetCurrentState = vi.fn(() => 'processing');
const mockDispatchSubmit = vi.fn();
const mockDispatchProgress = vi.fn();
const mockDispatchRefresh = vi.fn();
const mockStartDialogTurn = vi.fn();
const mockBindSession = vi.fn();

vi.mock('../../state-machine', () => ({
  SessionExecutionEvent: {
    FINISHING_SETTLED: 'finishing_settled',
    USER_CANCEL: 'user_cancel',
  },
  SessionExecutionState: {
    PROCESSING: 'processing',
  },
  stateMachineManager: {
    getCurrentState: () => mockGetCurrentState(),
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

vi.mock('./PendingQueueModule', () => ({
  pendingQueueManager: {
    list: () => [],
    enqueue: vi.fn(),
  },
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
    error: vi.fn(),
  },
}));

describe('MessageModule cancellation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
  });

  function createDispatchContext(approvalPolicy: 'auto' | 'reject-and-report') {
    const session = {
      sessionId: 'dispatch-session',
      title: 'New Chat',
      titleStatus: 'generated',
      mode: 'agentic',
      dialogTurns: [],
      config: {
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
          applyDispatchSnapshot: vi.fn(() => ({ applied: true, cursor: 0 })),
          updateSessionLastSubmittedMode: vi.fn(),
          updateSessionMode: vi.fn(),
        },
        pendingHistoryLoads: new Map(),
      } as any,
    };
  }

  it('submits without controller model/title and bypasses local turn/worktree APIs', async () => {
    const { context } = createDispatchContext('reject-and-report');

    await sendMessage(context, 'run remote checks', 'dispatch-session');

    expect(mockDispatchSubmit).toHaveBeenCalledWith({
      target: {
        kind: 'ssh',
        connectionId: 'ssh-1',
        workspacePath: '/target/repo',
      },
      jobId: 'job-1',
      sessionId: 'dispatch-session',
      agentType: 'agentic',
      prompt: 'run remote checks',
      approvalPolicy: 'reject-and-report',
      model: undefined,
    });
    expect(mockStartDialogTurn).not.toHaveBeenCalled();
    expect(mockBindSession).not.toHaveBeenCalled();
  });

  it('requires a one-shot auto-approval confirmation before the actual submit', async () => {
    const { context } = createDispatchContext('auto');

    await expect(
      sendMessage(context, 'run remote checks', 'dispatch-session'),
    ).rejects.toThrow('requires an explicit confirmation');
    expect(mockDispatchSubmit).not.toHaveBeenCalled();

    await expect(
      sendMessage(
        context,
        'run remote checks',
        'dispatch-session',
        undefined,
        undefined,
        undefined,
        { dispatchAutoConfirmed: true },
      ),
    ).resolves.toBeUndefined();
    expect(mockDispatchSubmit).toHaveBeenCalledTimes(1);
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
      config: { modelName: 'auto' },
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
    });
  });

  it('migrates a legacy session without a model to the current mode default', async () => {
    const session = {
      sessionId: 'legacy-session',
      config: {},
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
    });
  });
});

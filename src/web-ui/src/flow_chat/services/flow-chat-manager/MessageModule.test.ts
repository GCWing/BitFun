import { beforeEach, describe, expect, it, vi } from 'vitest';
import { cancelSessionTask, syncSessionModelSelection } from './MessageModule';
import { SessionExecutionEvent } from '../../state-machine/types';

const mockCancelTransientBtwSession = vi.fn();
const mockTransition = vi.fn();
const mockUpdateSessionModel = vi.fn();
const mockGetConfigs = vi.fn();

vi.mock('../BtwThreadService', () => ({
  cancelTransientBtwSession: (...args: any[]) => mockCancelTransientBtwSession(...args),
  isTransientBtwSession: (session: any) =>
    session?.isTransient === true &&
    session?.sessionKind === 'btw' &&
    session?.agentBackedTransient !== true,
  sendMessageToTransientBtwSession: vi.fn(),
}));

vi.mock('../../state-machine', () => ({
  SessionExecutionEvent: {
    FINISHING_SETTLED: 'finishing_settled',
    USER_CANCEL: 'user_cancel',
  },
  SessionExecutionState: {
    PROCESSING: 'processing',
  },
  stateMachineManager: {
    getCurrentState: vi.fn(() => 'processing'),
    transition: (...args: any[]) => mockTransition(...args),
  },
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: {
    updateSessionModel: (...args: unknown[]) => mockUpdateSessionModel(...args),
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
    mockCancelTransientBtwSession.mockResolvedValue(true);
    mockTransition.mockResolvedValue(true);
  });

  it('cancels transient /btw sessions locally and through the /btw API path', async () => {
    const session = {
      sessionId: 'btw-child',
      isTransient: true,
      sessionKind: 'btw',
      agentBackedTransient: false,
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

    expect(mockStoreCancelSessionTask).toHaveBeenCalledWith('btw-child');
    expect(mockTransition).toHaveBeenCalledWith(
      'btw-child',
      SessionExecutionEvent.FINISHING_SETTLED,
    );
    expect(mockCancelTransientBtwSession).toHaveBeenCalledWith('btw-child');
    expect(context.userCancelledSessionIds.has('btw-child')).toBe(true);
    expect(contentBuffers.has('btw-child')).toBe(false);
    expect(activeTextItems.has('btw-child')).toBe(false);
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

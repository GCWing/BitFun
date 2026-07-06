import { beforeEach, describe, expect, it, vi } from 'vitest';
import { cancelSessionTask } from './MessageModule';
import { SessionExecutionEvent } from '../../state-machine/types';

const mockCancelTransientBtwSession = vi.fn();
const mockTransition = vi.fn();

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
  agentAPI: {},
}));

vi.mock('@/infrastructure/api/service-api/ACPClientAPI', () => ({
  ACPClientAPI: {},
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: {},
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

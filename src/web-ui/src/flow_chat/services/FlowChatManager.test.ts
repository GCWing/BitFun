import { beforeEach, describe, expect, it, vi } from 'vitest';
import { FlowChatManager } from './FlowChatManager';

const storeMocks = vi.hoisted(() => ({
  store: {} as any,
  initializeEventListeners: vi.fn(),
}));

vi.mock('./ProcessingStatusManager', () => ({
  processingStatusManager: {},
}));

vi.mock('../store/FlowChatStore', () => ({
  FlowChatStore: {
    getInstance: () => storeMocks.store,
  },
}));

vi.mock('../../shared/services/agent-service', () => ({
  AgentService: {
    getInstance: vi.fn(() => ({})),
  },
}));

vi.mock('@/infrastructure/api/service-api/ACPClientAPI', () => ({
  ACPClientAPI: {},
}));

vi.mock('../state-machine', () => ({
  stateMachineManager: {},
}));

vi.mock('./EventBatcher', () => ({
  EventBatcher: class {
    constructor(private readonly options: { onFlush: (events: Array<{ key: string; payload: unknown }>) => void }) {}

    flush(events: Array<{ key: string; payload: unknown }>): void {
      this.options.onFlush(events);
    }
  },
}));

vi.mock('./flow-chat-manager', () => ({
  saveAllInProgressTurns: vi.fn(),
  immediateSaveDialogTurn: vi.fn(),
  createChatSession: vi.fn(),
  switchChatSession: vi.fn(),
  deleteChatSession: vi.fn(),
  renameChatSessionTitle: vi.fn(),
  forkChatSession: vi.fn(),
  cleanupSaveState: vi.fn(),
  cleanupSessionBuffers: vi.fn(),
  sendMessage: vi.fn(),
  cancelCurrentTask: vi.fn(),
  installPendingQueueDrainListener: vi.fn(),
  drainPendingQueue: vi.fn(),
  initializeEventListeners: storeMocks.initializeEventListeners,
  processBatchedEvents: vi.fn(),
  addDialogTurn: vi.fn(),
  addImageAnalysisPhase: vi.fn(),
  updateImageAnalysisResults: vi.fn(),
  updateImageAnalysisItem: vi.fn(),
  updateSessionMetadata: vi.fn(),
}));

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function flushAsyncWork(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

function createHistoricalSession(overrides: Record<string, unknown> = {}) {
  return {
    sessionId: 'history-1',
    title: 'History 1',
    dialogTurns: [],
    status: 'idle',
    config: { agentType: 'agentic' },
    createdAt: 10,
    lastFinishedAt: 20,
    lastActiveAt: 20,
    error: null,
    isHistorical: true,
    historyState: 'metadata-only',
    todos: [],
    mode: 'agentic',
    workspacePath: 'D:/workspace/BitFun',
    sessionKind: 'normal',
    ...overrides,
  };
}

describe('FlowChatManager initialization', () => {
  beforeEach(() => {
    (FlowChatManager as any).instance = undefined;
    vi.clearAllMocks();
    storeMocks.initializeEventListeners.mockResolvedValue(() => {});
  });

  it('reuses concurrent initialization for the same workspace history restore', async () => {
    const metadataLoad = createDeferred<{
      sessions: unknown[];
      totalTopLevelCount: number;
      hasMore: boolean;
      nextCursor?: string;
    }>();
    const sessions = new Map<string, any>([
      ['history-1', createHistoricalSession()],
    ]);
    let activeSessionId: string | null = null;

    storeMocks.store = {
      registerPersistUnreadCompletionCallback: vi.fn(),
      loadSessionMetadataPage: vi.fn(() => metadataLoad.promise),
      getState: vi.fn(() => ({
        sessions,
        activeSessionId,
      })),
      loadSessionHistory: vi.fn(async () => undefined),
      switchSession: vi.fn((sessionId: string) => {
        activeSessionId = sessionId;
      }),
    };

    const manager = FlowChatManager.getInstance();
    const firstInitialize = manager.initialize('D:/workspace/BitFun');
    const secondInitialize = manager.initialize('D:/workspace/BitFun');

    await flushAsyncWork();

    expect(storeMocks.store.loadSessionMetadataPage).toHaveBeenCalledTimes(1);

    metadataLoad.resolve({
      sessions: [],
      totalTopLevelCount: 1,
      hasMore: false,
    });

    await expect(Promise.all([firstInitialize, secondInitialize])).resolves.toEqual([true, true]);

    expect(storeMocks.store.loadSessionMetadataPage).toHaveBeenCalledTimes(1);
    expect(storeMocks.store.loadSessionHistory).toHaveBeenCalledTimes(1);
    expect(storeMocks.store.switchSession).toHaveBeenCalledTimes(1);
    expect(storeMocks.store.switchSession).toHaveBeenCalledWith('history-1');
  });

  it('does not overwrite a user-selected workspace session after initial history restore completes', async () => {
    const historyRestore = createDeferred<void>();
    const sessions = new Map<string, any>([
      ['history-1', createHistoricalSession({
        sessionId: 'history-1',
        title: 'Newest history',
        lastActiveAt: 30,
        lastFinishedAt: 30,
      })],
      ['history-2', createHistoricalSession({
        sessionId: 'history-2',
        title: 'User selected history',
        lastActiveAt: 10,
        lastFinishedAt: 10,
      })],
    ]);
    let activeSessionId: string | null = null;

    storeMocks.store = {
      registerPersistUnreadCompletionCallback: vi.fn(),
      loadSessionMetadataPage: vi.fn(async () => ({
        sessions: [],
        totalTopLevelCount: 2,
        hasMore: false,
      })),
      getState: vi.fn(() => ({
        sessions,
        activeSessionId,
      })),
      loadSessionHistory: vi.fn(() => historyRestore.promise),
      switchSession: vi.fn((sessionId: string) => {
        activeSessionId = sessionId;
      }),
    };

    const manager = FlowChatManager.getInstance();
    const initialize = manager.initialize('D:/workspace/BitFun');

    await flushAsyncWork();
    expect(storeMocks.store.loadSessionHistory).toHaveBeenCalledWith(
      'history-1',
      'D:/workspace/BitFun',
      undefined,
      undefined,
      undefined,
      { deferFullHistoryUntilActive: true },
    );

    activeSessionId = 'history-2';
    historyRestore.resolve();

    await expect(initialize).resolves.toBe(true);

    expect(storeMocks.store.switchSession).not.toHaveBeenCalled();
    expect(activeSessionId).toBe('history-2');
  });

  it('does not let a stale workspace initialization overwrite a newer active workspace', async () => {
    const historyRestore = createDeferred<void>();
    const sessions = new Map<string, any>([
      ['history-1', createHistoricalSession()],
      ['other-1', createHistoricalSession({
        sessionId: 'other-1',
        title: 'Other workspace',
        workspacePath: 'D:/workspace/Other',
        lastActiveAt: 40,
        lastFinishedAt: 40,
      })],
    ]);
    let activeSessionId: string | null = null;

    storeMocks.store = {
      registerPersistUnreadCompletionCallback: vi.fn(),
      loadSessionMetadataPage: vi.fn(async () => ({
        sessions: [],
        totalTopLevelCount: 1,
        hasMore: false,
      })),
      getState: vi.fn(() => ({
        sessions,
        activeSessionId,
      })),
      loadSessionHistory: vi.fn(() => historyRestore.promise),
      switchSession: vi.fn((sessionId: string) => {
        activeSessionId = sessionId;
      }),
    };

    const manager = FlowChatManager.getInstance();
    const initialize = manager.initialize('D:/workspace/BitFun');

    await flushAsyncWork();
    activeSessionId = 'other-1';
    (manager as unknown as { context: { currentWorkspacePath: string | null } })
      .context.currentWorkspacePath = 'D:/workspace/Other';
    historyRestore.resolve();

    await expect(initialize).resolves.toBe(true);

    expect(storeMocks.store.switchSession).not.toHaveBeenCalled();
    expect(activeSessionId).toBe('other-1');
    expect((manager as unknown as { context: { currentWorkspacePath: string | null } })
      .context.currentWorkspacePath).toBe('D:/workspace/Other');
  });

  it('does not let an older workspace initialization switch after a newer workspace initialize starts', async () => {
    const bitfunHistoryRestore = createDeferred<void>();
    const sessions = new Map<string, any>([
      ['history-1', createHistoricalSession()],
      ['other-1', createHistoricalSession({
        sessionId: 'other-1',
        title: 'Other workspace',
        workspacePath: 'D:/workspace/Other',
        lastActiveAt: 40,
        lastFinishedAt: 40,
      })],
    ]);
    let activeSessionId: string | null = null;

    storeMocks.store = {
      registerPersistUnreadCompletionCallback: vi.fn(),
      loadSessionMetadataPage: vi.fn(async (
        workspacePath: string,
      ) => ({
        sessions: [],
        totalTopLevelCount: workspacePath === 'D:/workspace/BitFun' ? 1 : 0,
        hasMore: false,
      })),
      getState: vi.fn(() => ({
        sessions,
        activeSessionId,
      })),
      loadSessionHistory: vi.fn((sessionId: string) => {
        if (sessionId === 'history-1') {
          return bitfunHistoryRestore.promise;
        }
        return Promise.resolve();
      }),
      switchSession: vi.fn((sessionId: string) => {
        activeSessionId = sessionId;
      }),
    };

    const manager = FlowChatManager.getInstance();
    const bitfunInitialize = manager.initialize('D:/workspace/BitFun');

    await flushAsyncWork();
    await expect(manager.initialize('D:/workspace/Other')).resolves.toBe(true);

    bitfunHistoryRestore.resolve();
    await expect(bitfunInitialize).resolves.toBe(true);

    expect(storeMocks.store.switchSession).toHaveBeenCalledWith('other-1');
    expect(storeMocks.store.switchSession).not.toHaveBeenCalledWith('history-1');
    expect((manager as unknown as { context: { currentWorkspacePath: string | null } })
      .context.currentWorkspacePath).toBe('D:/workspace/Other');
  });
});

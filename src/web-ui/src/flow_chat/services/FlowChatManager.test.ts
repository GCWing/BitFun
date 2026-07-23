import { beforeEach, describe, expect, it, vi } from 'vitest';
import { FlowChatManager } from './FlowChatManager';

const storeMocks = vi.hoisted(() => ({
  store: {} as any,
  initializeEventListeners: vi.fn(),
  switchChatSession: vi.fn(),
  eventBatchers: [] as Array<{
    flushNow: ReturnType<typeof vi.fn>;
    destroy: ReturnType<typeof vi.fn>;
  }>,
}));

vi.mock('./ProcessingStatusManager', () => ({
  processingStatusManager: {},
}));

vi.mock('./flow-chat-manager/PeerSessionRefreshModule', () => ({
  installPeerSessionRefresh: vi.fn(() => () => {}),
}));

vi.mock('../store/FlowChatStore', () => ({
  FlowChatStore: {
    getInstance: () => storeMocks.store,
  },
  flowChatStore: {
    getState: () => ({
      activeSessionId: null,
      sessions: new Map(),
    }),
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
    public flushNow = vi.fn();
    public destroy = vi.fn();

    constructor(private readonly options: { onFlush: (events: Array<{ key: string; payload: unknown }>) => void }) {
      storeMocks.eventBatchers.push(this);
    }

    flush(events: Array<{ key: string; payload: unknown }>): void {
      this.options.onFlush(events);
    }
  },
}));

vi.mock('./flow-chat-manager/PeerSessionRefreshModule', () => ({
  installPeerSessionRefresh: vi.fn(() => () => {}),
}));

vi.mock('./flow-chat-manager', () => ({
  saveAllInProgressTurns: vi.fn(),
  immediateSaveDialogTurn: vi.fn(),
  createChatSession: vi.fn(),
  switchChatSession: (...args: unknown[]) => storeMocks.switchChatSession(...args),
  deleteChatSession: vi.fn(),
  archiveChatSession: vi.fn(),
  renameChatSessionTitle: vi.fn(),
  forkChatSession: vi.fn(),
  cleanupSaveState: vi.fn(),
  cleanupSessionBuffers: vi.fn(),
  sendMessage: vi.fn(),
  cancelCurrentTask: vi.fn(),
  cancelSessionTask: vi.fn(),
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
    storeMocks.eventBatchers.length = 0;
    storeMocks.initializeEventListeners.mockResolvedValue(() => {});
    storeMocks.switchChatSession.mockImplementation(async (context: any, sessionId: string) => {
      context.flowChatStore.switchSession(sessionId);
    });
  });

  it('flushes and destroys the batcher when the singleton is disposed', () => {
    storeMocks.store = {};

    const manager = FlowChatManager.getInstance();
    const batcher = storeMocks.eventBatchers[0];

    FlowChatManager.disposeInstance();

    expect(batcher.flushNow).toHaveBeenCalledTimes(1);
    expect(batcher.destroy).toHaveBeenCalledTimes(1);
    expect(batcher.flushNow.mock.invocationCallOrder[0]).toBeLessThan(
      batcher.destroy.mock.invocationCallOrder[0],
    );
  });

  it('runs listener cleanup if disposal wins the initialization race', async () => {
    storeMocks.store = {};
    const listenerInitialization = createDeferred<() => void>();
    const cleanup = vi.fn();
    storeMocks.initializeEventListeners.mockReturnValue(listenerInitialization.promise);

    const manager = FlowChatManager.getInstance();
    const initializeListeners = (manager as any).initializeEventListeners();

    await flushAsyncWork();
    manager.destroy();
    expect(cleanup).not.toHaveBeenCalled();

    listenerInitialization.resolve(cleanup);
    await initializeListeners;

    expect(cleanup).toHaveBeenCalledTimes(1);
    expect((manager as any).eventListenerInitialized).toBe(false);
    expect((manager as any).eventListenerCleanup).toBeNull();
  });

  it('stops workspace initialization if the manager is disposed while listeners initialize', async () => {
    const listenerInitialization = createDeferred<() => void>();
    storeMocks.initializeEventListeners.mockReturnValue(listenerInitialization.promise);
    storeMocks.store = {
      registerPersistUnreadCompletionCallback: vi.fn(),
      loadSessionMetadataPage: vi.fn(async () => ({
        sessions: [],
        totalTopLevelCount: 0,
        hasMore: false,
      })),
    };

    const manager = FlowChatManager.getInstance();
    const initialize = manager.initialize('D:/workspace/BitFun');

    await flushAsyncWork();
    manager.destroy();
    listenerInitialization.resolve(vi.fn());

    await expect(initialize).resolves.toBe(false);
    expect(storeMocks.store.registerPersistUnreadCompletionCallback).not.toHaveBeenCalled();
    expect(storeMocks.store.loadSessionMetadataPage).not.toHaveBeenCalled();
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

  it('ignores child subagent sessions when auto-selecting a workspace session', async () => {
    const sessions = new Map<string, any>([
      ['parent-1', createHistoricalSession({
        sessionId: 'parent-1',
        title: 'Parent session',
        isHistorical: false,
        historyState: 'ready',
        createdAt: 10,
        lastFinishedAt: 30,
        workspacePath: 'D:/workspace/BitFun',
        sessionKind: 'normal',
      })],
      ['subagent-1', createHistoricalSession({
        sessionId: 'subagent-1',
        title: 'Subagent session',
        isHistorical: false,
        historyState: 'ready',
        createdAt: 40,
        lastFinishedAt: undefined,
        workspacePath: 'D:/workspace/BitFun',
        sessionKind: 'subagent',
        parentSessionId: 'parent-1',
        mode: 'Explore',
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
      loadSessionHistory: vi.fn(async () => undefined),
      switchSession: vi.fn((sessionId: string) => {
        activeSessionId = sessionId;
      }),
    };

    const manager = FlowChatManager.getInstance();
    await expect(manager.initialize('D:/workspace/BitFun')).resolves.toBe(true);

    expect(storeMocks.store.switchSession).toHaveBeenCalledTimes(1);
    expect(storeMocks.store.switchSession).toHaveBeenCalledWith('parent-1');
    expect(storeMocks.store.switchSession).not.toHaveBeenCalledWith('subagent-1');
  });
});

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { FlowChatManager } from './FlowChatManager';
import {
  activateSurface,
  isSurfaceChangedError,
  resetDeviceSurfaceForTest,
} from '@/infrastructure/peer-device/deviceSurface';

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
  processingStatusManager: {
    clearSurface: vi.fn(),
  },
}));

vi.mock('./flow-chat-manager/PeerSessionRefreshModule', () => ({
  installPeerSessionRefresh: vi.fn(() => () => {}),
}));

vi.mock('@/features/dispatch/DispatchJobObserver', () => ({
  installDispatchJobObserver: vi.fn(() => () => {}),
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

vi.mock('@/infrastructure/api/service-api/ACPClientAPI', () => ({
  ACPClientAPI: {},
}));

vi.mock('../state-machine', () => ({
  stateMachineManager: {
    clearSurface: vi.fn(),
  },
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
  reloadSessionTitle: vi.fn(),
  forkChatSession: vi.fn(),
  cleanupSaveState: vi.fn(),
  cleanupSessionBuffers: vi.fn(),
  sendMessage: vi.fn(),
  cancelCurrentTask: vi.fn(),
  cancelSessionTask: vi.fn(),
  installPendingQueueDrainListener: vi.fn(),
  drainPendingQueue: vi.fn(),
  pendingQueueManager: {
    clearSurface: vi.fn(),
  },
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
    workspacePath: 'D:/workspace/OpenBitFun',
    sessionKind: 'normal',
    ...overrides,
  };
}

describe('FlowChatManager initialization', () => {
  beforeEach(() => {
    resetDeviceSurfaceForTest();
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
      getSurfaceGeneration: vi.fn(() => 0),
      loadSessionMetadataPage: vi.fn(async () => ({
        sessions: [],
        totalTopLevelCount: 0,
        hasMore: false,
      })),
    };

    const manager = FlowChatManager.getInstance();
    const initialize = manager.initialize('D:/workspace/OpenBitFun');

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
      getSurfaceGeneration: vi.fn(() => 0),
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
    const firstInitialize = manager.initialize('D:/workspace/OpenBitFun');
    const secondInitialize = manager.initialize('D:/workspace/OpenBitFun');

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
      getSurfaceGeneration: vi.fn(() => 0),
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
    const initialize = manager.initialize('D:/workspace/OpenBitFun');

    await flushAsyncWork();
    expect(storeMocks.store.loadSessionHistory).toHaveBeenCalledWith(
      'history-1',
      'D:/workspace/OpenBitFun',
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
      getSurfaceGeneration: vi.fn(() => 0),
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
    const initialize = manager.initialize('D:/workspace/OpenBitFun');

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
    const openbitfunHistoryRestore = createDeferred<void>();
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
      getSurfaceGeneration: vi.fn(() => 0),
      loadSessionMetadataPage: vi.fn(async (
        workspacePath: string,
      ) => ({
        sessions: [],
        totalTopLevelCount: workspacePath === 'D:/workspace/OpenBitFun' ? 1 : 0,
        hasMore: false,
      })),
      getState: vi.fn(() => ({
        sessions,
        activeSessionId,
      })),
      loadSessionHistory: vi.fn((sessionId: string) => {
        if (sessionId === 'history-1') {
          return openbitfunHistoryRestore.promise;
        }
        return Promise.resolve();
      }),
      switchSession: vi.fn((sessionId: string) => {
        activeSessionId = sessionId;
      }),
    };

    const manager = FlowChatManager.getInstance();
    const openbitfunInitialize = manager.initialize('D:/workspace/OpenBitFun');

    await flushAsyncWork();
    await expect(manager.initialize('D:/workspace/Other')).resolves.toBe(true);

    openbitfunHistoryRestore.resolve();
    await expect(openbitfunInitialize).resolves.toBe(true);

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
        workspacePath: 'D:/workspace/OpenBitFun',
        sessionKind: 'normal',
      })],
      ['subagent-1', createHistoricalSession({
        sessionId: 'subagent-1',
        title: 'Subagent session',
        isHistorical: false,
        historyState: 'ready',
        createdAt: 40,
        lastFinishedAt: undefined,
        workspacePath: 'D:/workspace/OpenBitFun',
        sessionKind: 'subagent',
        parentSessionId: 'parent-1',
        mode: 'Explore',
      })],
    ]);
    let activeSessionId: string | null = null;

    storeMocks.store = {
      registerPersistUnreadCompletionCallback: vi.fn(),
      getSurfaceGeneration: vi.fn(() => 0),
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
    await expect(manager.initialize('D:/workspace/OpenBitFun')).resolves.toBe(true);

    expect(storeMocks.store.switchSession).toHaveBeenCalledTimes(1);
    expect(storeMocks.store.switchSession).toHaveBeenCalledWith('parent-1');
    expect(storeMocks.store.switchSession).not.toHaveBeenCalledWith('subagent-1');
  });

  // The page describes the device it was read from and has already landed in
  // that surface's own container. Selecting out of it after the window moved
  // would apply one device's history to another; the device now on screen runs
  // its own bootstrap.
  it('abandons a workspace bootstrap when the window switches device mid-load', async () => {
    const sessions = new Map<string, any>();

    storeMocks.store = {
      registerPersistUnreadCompletionCallback: vi.fn(),
      getSurfaceGeneration: vi.fn(() => 0),
      loadSessionMetadataPage: vi.fn(async () => {
        activateSurface('device-b');
        sessions.set('history-1', createHistoricalSession({ historyState: 'ready', isHistorical: false }));
        return { sessions: [{ sessionId: 'history-1' }], totalTopLevelCount: 1, hasMore: false };
      }),
      getState: vi.fn(() => ({ sessions, activeSessionId: null })),
      loadSessionHistory: vi.fn(async () => undefined),
      switchSession: vi.fn(),
    };

    const manager = FlowChatManager.getInstance();
    await expect(manager.initialize('D:/workspace/OpenBitFun')).rejects.toSatisfy(isSurfaceChangedError);

    expect(storeMocks.store.loadSessionMetadataPage).toHaveBeenCalledTimes(1);
    expect(storeMocks.store.switchSession).not.toHaveBeenCalled();
  });

  // The same repository is routinely open at the same path on two devices, so a
  // request key without the surface handed device A's bootstrap the in-flight
  // initialization of device B — and A then read back B's session list.
  it('does not deduplicate initialization for the same path across devices', async () => {
    const metadataLoads = [
      createDeferred<Record<string, unknown>>(),
      createDeferred<Record<string, unknown>>(),
    ];
    const peerSessions = new Map<string, any>();
    let activeSessionId: string | null = null;

    storeMocks.store = {
      registerPersistUnreadCompletionCallback: vi.fn(),
      getSurfaceGeneration: vi.fn(() => 0),
      loadSessionMetadataPage: vi.fn(
        () => metadataLoads[storeMocks.store.loadSessionMetadataPage.mock.calls.length - 1].promise,
      ),
      getState: vi.fn(() => ({ sessions: peerSessions, activeSessionId })),
      loadSessionHistory: vi.fn(async () => undefined),
      switchSession: vi.fn((sessionId: string) => {
        activeSessionId = sessionId;
      }),
    };

    const manager = FlowChatManager.getInstance();
    const localInitialize = manager.initialize('D:/workspace/OpenBitFun');
    await flushAsyncWork();
    expect(storeMocks.store.loadSessionMetadataPage).toHaveBeenCalledTimes(1);

    activateSurface('device-b');
    const peerInitialize = manager.initialize('D:/workspace/OpenBitFun');
    await flushAsyncWork();
    // A shared key would have handed this bootstrap the local device's request.
    expect(storeMocks.store.loadSessionMetadataPage).toHaveBeenCalledTimes(2);

    peerSessions.set('peer-1', createHistoricalSession({
      sessionId: 'peer-1',
      historyState: 'ready',
      isHistorical: false,
    }));
    metadataLoads[0].resolve({ sessions: [], totalTopLevelCount: 0, hasMore: false });
    metadataLoads[1].resolve({
      sessions: [{ sessionId: 'peer-1' }],
      totalTopLevelCount: 1,
      hasMore: false,
    });

    // The local bootstrap started under a surface this window left; it must not
    // adopt the peer's answer.
    await expect(localInitialize).rejects.toSatisfy(isSurfaceChangedError);
    await expect(peerInitialize).resolves.toBe(true);
    expect(storeMocks.store.switchSession).toHaveBeenCalledTimes(1);
    expect(storeMocks.store.switchSession).toHaveBeenCalledWith('peer-1');
  });

  it('restores history for a session that is already active but still metadata-only', async () => {
    const activeSession = createHistoricalSession({ sessionId: 'active-1' });
    const sessions = new Map<string, any>([['active-1', activeSession]]);

    storeMocks.store = {
      registerPersistUnreadCompletionCallback: vi.fn(),
      getSurfaceGeneration: vi.fn(() => 0),
      loadSessionMetadataPage: vi.fn(async () => ({
        sessions: [{ sessionId: 'active-1' }],
        totalTopLevelCount: 1,
        hasMore: false,
      })),
      getState: vi.fn(() => ({ sessions, activeSessionId: 'active-1' })),
      loadSessionHistory: vi.fn(async () => undefined),
      switchSession: vi.fn(),
    };

    const manager = FlowChatManager.getInstance();
    await expect(manager.initialize('D:/workspace/OpenBitFun')).resolves.toBe(true);

    // Without this the breadcrumb and turn rail render from the catalog while
    // the message area stays blank until the user clicks the session.
    expect(storeMocks.store.loadSessionHistory).toHaveBeenCalledWith(
      'active-1',
      'D:/workspace/OpenBitFun',
      undefined,
      undefined,
      undefined,
    );
  });

  it('reports no history when metadata claims sessions but none are selectable', async () => {
    storeMocks.store = {
      registerPersistUnreadCompletionCallback: vi.fn(),
      getSurfaceGeneration: vi.fn(() => 7),
      loadSessionMetadataPage: vi.fn(async () => ({
        sessions: [{ sessionId: 'history-1' }],
        totalTopLevelCount: 1,
        hasMore: false,
      })),
      getState: vi.fn(() => ({ sessions: new Map(), activeSessionId: null })),
      loadSessionHistory: vi.fn(async () => undefined),
      switchSession: vi.fn(),
    };

    const manager = FlowChatManager.getInstance();

    // `false` is the caller's signal to create a session against the live
    // workspace. Returning `true` here would leave the surface with no active
    // session and no new one.
    await expect(manager.initialize('D:/workspace/OpenBitFun')).resolves.toBe(false);
    expect(storeMocks.store.switchSession).not.toHaveBeenCalled();
  });
});

describe('FlowChatManager live subscription self-healing', () => {
  beforeEach(() => {
    resetDeviceSurfaceForTest();
    (FlowChatManager as any).instance = undefined;
    vi.clearAllMocks();
    storeMocks.eventBatchers.length = 0;
    storeMocks.store = {};
    storeMocks.initializeEventListeners.mockResolvedValue(() => {});
  });

  it('re-arms the subscription on every surface activation', async () => {
    // A switch tears the subscription down and the workspace bootstrap that
    // used to rebuild it may legitimately be superseded. Activation itself must
    // therefore restore the only live view of a running Turn.
    const manager = FlowChatManager.getInstance();
    await (manager as any).ensureEventListeners();
    expect(storeMocks.initializeEventListeners).toHaveBeenCalledTimes(1);

    manager.cleanupEventListeners();
    activateSurface('device-b');
    await Promise.resolve();
    await Promise.resolve();

    expect(storeMocks.initializeEventListeners).toHaveBeenCalledTimes(2);
    manager.destroy();
  });

  it('retries a subscription that failed to start', async () => {
    vi.useFakeTimers();
    try {
      storeMocks.initializeEventListeners.mockRejectedValueOnce(new Error('bridge not ready'));
      const manager = FlowChatManager.getInstance();

      await (manager as any).ensureEventListeners();
      expect(storeMocks.initializeEventListeners).toHaveBeenCalledTimes(1);

      storeMocks.initializeEventListeners.mockResolvedValue(() => {});
      await vi.advanceTimersByTimeAsync(2500);

      expect(storeMocks.initializeEventListeners).toHaveBeenCalledTimes(2);
      manager.destroy();
    } finally {
      vi.useRealTimers();
    }
  });

  it('leaves a healthy subscription alone', async () => {
    const manager = FlowChatManager.getInstance();
    await (manager as any).ensureEventListeners();
    await (manager as any).ensureEventListeners();

    expect(storeMocks.initializeEventListeners).toHaveBeenCalledTimes(1);
    manager.destroy();
  });
});

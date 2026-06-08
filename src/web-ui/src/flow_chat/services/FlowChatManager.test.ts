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
});

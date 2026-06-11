import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  ensureBackendSession,
  preloadHistoricalSessionForOpen,
  retryCreateBackendSession,
  SESSION_ACTIVITY_TOUCH_DELAY_MS,
  switchChatSession,
} from './SessionModule';
import {
  clearRecentHistorySessionOpenIntent,
  dispatchHistorySessionOpenIntent,
} from '../sessionOpenIntent';
import type { Session } from '../../types/flow-chat';
import type { ReviewTeamRunManifest } from '@/shared/services/reviewTeamService';

const agentApiMocks = vi.hoisted(() => ({
  ensureCoordinatorSession: vi.fn(),
  createSession: vi.fn(),
}));

const persistenceMocks = vi.hoisted(() => ({
  touchSessionActivity: vi.fn(),
  cleanupSaveState: vi.fn(),
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: agentApiMocks,
}));

vi.mock('@/infrastructure/api/service-api/SessionAPI', () => ({
  sessionAPI: {},
}));

vi.mock('../../../shared/notification-system', () => ({
  notificationService: {
    error: vi.fn(),
    warning: vi.fn(),
  },
}));

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: (key: string) => key,
  },
}));

vi.mock('@/infrastructure/services/business/workspaceManager', () => ({
  workspaceManager: {
    getState: () => ({
      currentWorkspace: null,
      openedWorkspaces: new Map(),
    }),
  },
}));

vi.mock('./PersistenceModule', () => ({
  touchSessionActivity: persistenceMocks.touchSessionActivity,
  cleanupSaveState: persistenceMocks.cleanupSaveState,
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

function createSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'history-1',
    title: 'Saved session',
    dialogTurns: [],
    status: 'idle',
    config: { agentType: 'agentic' },
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    isHistorical: true,
    historyState: 'metadata-only',
    todos: [],
    mode: 'agentic',
    workspacePath: 'D:/workspace/BitFun',
    sessionKind: 'normal',
    parentSessionId: undefined,
    parentToolCallId: undefined,
    subagentType: undefined,
    btwOrigin: undefined,
    deepReviewRunManifest: undefined,
    ...overrides,
  };
}

function createContext(session: Session) {
  let state = {
    sessions: new Map([[session.sessionId, session]]),
    activeSessionId: null as string | null,
  };
  const flowChatStore = {
    getState: () => state,
    switchSession: vi.fn((sessionId: string) => {
      state = { ...state, activeSessionId: sessionId };
    }),
    loadSessionHistory: vi.fn(),
    setState: vi.fn((updater: any) => {
      state = updater(state);
    }),
  };

  return {
    context: {
      flowChatStore,
      pendingHistoryLoads: new Map<string, Promise<void>>(),
      pendingContextRestores: new Map<string, Promise<void>>(),
    } as any,
    flowChatStore,
  };
}

describe('SessionModule historical session coordination', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(async () => {
    await vi.runOnlyPendingTimersAsync();
    clearRecentHistorySessionOpenIntent();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('hydrates a metadata-only historical session before switching to avoid an empty loading page', async () => {
    const load = createDeferred<void>();
    const { context, flowChatStore } = createContext(createSession());
    flowChatStore.loadSessionHistory.mockReturnValueOnce(load.promise);
    persistenceMocks.touchSessionActivity.mockResolvedValueOnce(undefined);

    const switching = switchChatSession(context, 'history-1');
    await Promise.resolve();

    expect(flowChatStore.switchSession).not.toHaveBeenCalled();
    expect(flowChatStore.loadSessionHistory).toHaveBeenCalledTimes(1);

    load.resolve();
    await switching;

    expect(flowChatStore.switchSession).toHaveBeenCalledWith('history-1');
  });

  it('activates a metadata-only historical session immediately when a recent user open intent exists', async () => {
    const load = createDeferred<void>();
    const { context, flowChatStore } = createContext(createSession());
    flowChatStore.loadSessionHistory.mockReturnValueOnce(load.promise);
    persistenceMocks.touchSessionActivity.mockResolvedValueOnce(undefined);

    dispatchHistorySessionOpenIntent('history-1', 'Saved session');
    const switching = switchChatSession(context, 'history-1');
    await Promise.resolve();

    expect(flowChatStore.switchSession).toHaveBeenCalledWith('history-1');
    expect(flowChatStore.loadSessionHistory).toHaveBeenCalledTimes(1);
    expect(persistenceMocks.touchSessionActivity).not.toHaveBeenCalled();

    load.resolve();
    await switching;

    expect(flowChatStore.switchSession).toHaveBeenCalledTimes(1);
  });

  it('keeps metadata-only historical sessions out of the active render path until hydrated', async () => {
    const load = createDeferred<void>();
    const { context, flowChatStore } = createContext(createSession());
    context.pendingHistoryLoads.set('history-other', Promise.resolve());
    flowChatStore.loadSessionHistory.mockReturnValueOnce(load.promise);
    persistenceMocks.touchSessionActivity.mockResolvedValueOnce(undefined);

    const switching = switchChatSession(context, 'history-1');
    await Promise.resolve();

    expect(flowChatStore.loadSessionHistory).toHaveBeenCalledTimes(1);
    expect(flowChatStore.switchSession).not.toHaveBeenCalled();

    load.resolve();
    await switching;

    expect(flowChatStore.switchSession).toHaveBeenCalledWith('history-1');
  });

  it('defers activity touch until a metadata-only historical session has hydrated and switched', async () => {
    const load = createDeferred<void>();
    const { context, flowChatStore } = createContext(createSession());
    flowChatStore.loadSessionHistory.mockReturnValueOnce(load.promise);
    persistenceMocks.touchSessionActivity.mockResolvedValueOnce(undefined);

    const switching = switchChatSession(context, 'history-1');
    await Promise.resolve();

    expect(persistenceMocks.touchSessionActivity).not.toHaveBeenCalled();

    load.resolve();
    await switching;
    await Promise.resolve();

    expect(flowChatStore.switchSession).toHaveBeenCalledWith('history-1');
    expect(persistenceMocks.touchSessionActivity).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(SESSION_ACTIVITY_TOUCH_DELAY_MS - 1);
    expect(persistenceMocks.touchSessionActivity).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(persistenceMocks.touchSessionActivity).toHaveBeenCalledWith(
      'history-1',
      'D:/workspace/BitFun',
      undefined,
      undefined,
    );
  });

  it('switches immediately when a historical session already has renderable tail content', async () => {
    const load = createDeferred<void>();
    const { context, flowChatStore } = createContext(createSession({
      historyState: 'ready',
      dialogTurns: [{
        id: 'turn-1',
        userMessage: { id: 'user-turn-1', content: 'Latest prompt', timestamp: 1 },
        modelRounds: [],
        status: 'completed',
      } as any],
    }));
    flowChatStore.loadSessionHistory.mockReturnValueOnce(load.promise);
    persistenceMocks.touchSessionActivity.mockResolvedValueOnce(undefined);

    await switchChatSession(context, 'history-1');

    expect(flowChatStore.switchSession).toHaveBeenCalledWith('history-1');
    expect(flowChatStore.loadSessionHistory).toHaveBeenCalledTimes(1);

    load.resolve();
    await load.promise;
  });

  it('touches only the latest active session during rapid switches', async () => {
    const firstSession = createSession({
      sessionId: 'history-1',
      historyState: 'ready',
      dialogTurns: [{ id: 'turn-1', userMessage: { content: 'one' } } as any],
    });
    const secondSession = createSession({
      sessionId: 'history-2',
      historyState: 'ready',
      dialogTurns: [{ id: 'turn-2', userMessage: { content: 'two' } } as any],
    });
    const { context, flowChatStore } = createContext(firstSession);
    flowChatStore.setState((prev: any) => ({
      ...prev,
      sessions: new Map(prev.sessions).set(secondSession.sessionId, secondSession),
    }));
    flowChatStore.loadSessionHistory.mockResolvedValue(undefined);
    persistenceMocks.touchSessionActivity.mockResolvedValue(undefined);

    await switchChatSession(context, 'history-1');
    await switchChatSession(context, 'history-2');

    expect(flowChatStore.switchSession).toHaveBeenNthCalledWith(1, 'history-1');
    expect(flowChatStore.switchSession).toHaveBeenNthCalledWith(2, 'history-2');
    expect(persistenceMocks.touchSessionActivity).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(SESSION_ACTIVITY_TOUCH_DELAY_MS);

    expect(persistenceMocks.touchSessionActivity).toHaveBeenCalledTimes(1);
    expect(persistenceMocks.touchSessionActivity).toHaveBeenCalledWith(
      'history-2',
      'D:/workspace/BitFun',
      undefined,
      undefined,
    );
  });

  it('does not touch activity when the delayed session no longer exists', async () => {
    const session = createSession({
      historyState: 'ready',
      dialogTurns: [{ id: 'turn-1', userMessage: { content: 'one' } } as any],
    });
    const { context, flowChatStore } = createContext(session);
    persistenceMocks.touchSessionActivity.mockResolvedValue(undefined);

    await switchChatSession(context, 'history-1');
    flowChatStore.setState((prev: any) => ({
      ...prev,
      sessions: new Map(),
    }));

    await vi.advanceTimersByTimeAsync(SESSION_ACTIVITY_TOUCH_DELAY_MS);

    expect(persistenceMocks.touchSessionActivity).not.toHaveBeenCalled();
  });

  it('does not block remote metadata-only historical sessions on local pre-hydration before switching', async () => {
    const load = createDeferred<void>();
    const { context, flowChatStore } = createContext(createSession({
      remoteConnectionId: 'remote-1',
      remoteSshHost: 'remote-host',
    }));
    flowChatStore.loadSessionHistory.mockReturnValueOnce(load.promise);
    persistenceMocks.touchSessionActivity.mockResolvedValueOnce(undefined);

    await switchChatSession(context, 'history-1');

    expect(flowChatStore.switchSession).toHaveBeenCalledWith('history-1');
    expect(flowChatStore.loadSessionHistory).toHaveBeenCalledTimes(1);

    load.resolve();
    await load.promise;
  });

  it('preloads a local metadata-only historical session during a competing history load without switching', async () => {
    const load = createDeferred<void>();
    const { context, flowChatStore } = createContext(createSession());
    context.pendingHistoryLoads.set('history-other', Promise.resolve());
    flowChatStore.loadSessionHistory.mockReturnValueOnce(load.promise);

    preloadHistoricalSessionForOpen(context, 'history-1');
    await Promise.resolve();

    expect(flowChatStore.loadSessionHistory).toHaveBeenCalledTimes(1);
    expect(flowChatStore.switchSession).not.toHaveBeenCalled();

    load.resolve();
    await load.promise;
  });

  it('does not preload standalone historical opens before the transition shield paints', () => {
    const { context, flowChatStore } = createContext(createSession());

    preloadHistoricalSessionForOpen(context, 'history-1');

    expect(flowChatStore.loadSessionHistory).not.toHaveBeenCalled();
  });

  it('does not preload remote or already renderable historical sessions', async () => {
    const remoteSession = createSession({
      remoteConnectionId: 'remote-1',
      remoteSshHost: 'remote-host',
    });
    const { context, flowChatStore } = createContext(remoteSession);
    context.pendingHistoryLoads.set('history-other', Promise.resolve());

    preloadHistoricalSessionForOpen(context, 'history-1');

    expect(flowChatStore.loadSessionHistory).not.toHaveBeenCalled();

    flowChatStore.setState((prev: any) => ({
      ...prev,
      sessions: new Map(prev.sessions).set('history-1', createSession({
        dialogTurns: [{
          id: 'turn-1',
          userMessage: { id: 'user-1', content: 'Existing prompt', timestamp: 1 },
          modelRounds: [],
        } as any],
      })),
    }));

    preloadHistoricalSessionForOpen(context, 'history-1');

    expect(flowChatStore.loadSessionHistory).not.toHaveBeenCalled();
  });

  it('reuses pending historical hydration before ensuring the backend session', async () => {
    const pendingHydrate = createDeferred<void>();
    const { context, flowChatStore } = createContext(createSession());
    context.pendingHistoryLoads.set('history-1', pendingHydrate.promise);
    agentApiMocks.ensureCoordinatorSession.mockResolvedValueOnce(undefined);

    const ensure = ensureBackendSession(context, 'history-1');
    await Promise.resolve();

    expect(flowChatStore.loadSessionHistory).not.toHaveBeenCalled();
    expect(agentApiMocks.ensureCoordinatorSession).not.toHaveBeenCalled();

    pendingHydrate.resolve();
    await ensure;

    expect(agentApiMocks.ensureCoordinatorSession).toHaveBeenCalledTimes(1);
    expect(agentApiMocks.createSession).not.toHaveBeenCalled();
  });

  it('restores pending backend context for a view-restored session before send', async () => {
    const { context } = createContext(createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [{ id: 'turn-1' } as any],
    } as any));
    agentApiMocks.ensureCoordinatorSession.mockResolvedValueOnce(undefined);

    await ensureBackendSession(context, 'history-1');

    expect(agentApiMocks.ensureCoordinatorSession).toHaveBeenCalledTimes(1);
    expect(agentApiMocks.createSession).not.toHaveBeenCalled();
    expect(context.flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      contextRestoreState: 'ready',
    });
  });

  it('dedupes concurrent backend context restore for a view-restored session', async () => {
    const { context } = createContext(createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [{ id: 'turn-1' } as any],
    } as any));
    const restore = createDeferred<void>();
    agentApiMocks.ensureCoordinatorSession.mockReturnValueOnce(restore.promise);

    const firstEnsure = ensureBackendSession(context, 'history-1');
    const secondEnsure = ensureBackendSession(context, 'history-1');
    await Promise.resolve();

    expect(agentApiMocks.ensureCoordinatorSession).toHaveBeenCalledTimes(1);

    restore.resolve();
    await Promise.all([firstEnsure, secondEnsure]);

    expect(agentApiMocks.createSession).not.toHaveBeenCalled();
    expect(context.pendingContextRestores.size).toBe(0);
    expect(context.flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      contextRestoreState: 'ready',
    });
  });

  it('does not recreate a view-restored session with loaded turns when context restore fails', async () => {
    const { context } = createContext(createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [{ id: 'turn-1' } as any],
    } as any));
    agentApiMocks.ensureCoordinatorSession.mockRejectedValueOnce(
      new Error('Session metadata not found')
    );

    await expect(ensureBackendSession(context, 'history-1')).rejects.toThrow();

    expect(agentApiMocks.ensureCoordinatorSession).toHaveBeenCalledTimes(1);
    expect(agentApiMocks.createSession).not.toHaveBeenCalled();
    expect(context.flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      contextRestoreState: 'failed',
    });
  });

  it('keeps recreate fallback for empty pending context sessions', async () => {
    const { context } = createContext(createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [],
    } as any));
    agentApiMocks.ensureCoordinatorSession.mockRejectedValueOnce(
      new Error('Session metadata not found')
    );
    agentApiMocks.createSession.mockResolvedValueOnce(undefined);

    await ensureBackendSession(context, 'history-1');

    expect(agentApiMocks.ensureCoordinatorSession).toHaveBeenCalledTimes(1);
    expect(agentApiMocks.createSession).toHaveBeenCalledTimes(1);
    expect(context.flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      contextRestoreState: 'ready',
    });
  });

  it('recreates child sessions with structured relationship and deep review manifest', async () => {
    const deepReviewRunManifest = {
      workPackets: [],
      activeReviewers: [],
      optionalReviewers: [],
    } satisfies ReviewTeamRunManifest;
    const { context } = createContext(createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [],
      sessionKind: 'deep_review',
      parentSessionId: 'parent-1',
      btwOrigin: {
        requestId: 'req-1',
        parentSessionId: 'parent-1',
        parentDialogTurnId: 'turn-9',
        parentTurnIndex: 9,
      },
      deepReviewRunManifest,
    }));
    agentApiMocks.ensureCoordinatorSession.mockRejectedValueOnce(
      new Error('Session metadata not found')
    );
    agentApiMocks.createSession.mockResolvedValueOnce(undefined);

    await ensureBackendSession(context, 'history-1');

    expect(agentApiMocks.createSession).toHaveBeenCalledWith(
      expect.objectContaining({
        relationship: {
          kind: 'deep_review',
          parentSessionId: 'parent-1',
          parentRequestId: 'req-1',
          parentDialogTurnId: 'turn-9',
          parentTurnIndex: 9,
          parentToolCallId: null,
          subagentType: null,
        },
        deepReviewRunManifest,
      })
    );
  });

  it('retries child sessions with structured subagent relationship', async () => {
    const { context } = createContext(createSession({
      sessionId: 'subagent-1',
      isHistorical: false,
      historyState: 'ready',
      sessionKind: 'subagent',
      parentSessionId: 'parent-1',
      parentToolCallId: 'tool-7',
      subagentType: 'ReviewSecurity',
      btwOrigin: {
        parentSessionId: 'parent-1',
        parentDialogTurnId: 'turn-5',
        parentTurnIndex: 5,
      },
    }));
    agentApiMocks.createSession.mockResolvedValueOnce(undefined);

    await retryCreateBackendSession(context, 'subagent-1');

    expect(agentApiMocks.createSession).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: 'subagent-1',
        relationship: {
          kind: 'subagent',
          parentSessionId: 'parent-1',
          parentRequestId: null,
          parentDialogTurnId: 'turn-5',
          parentTurnIndex: 5,
          parentToolCallId: 'tool-7',
          subagentType: 'ReviewSecurity',
        },
      })
    );
  });
});

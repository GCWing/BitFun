import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  archiveChatSession,
  createChatSession,
  deleteChatSession,
  ensureBackendSession,
  preloadHistoricalSessionForOpen,
  retryCreateBackendSession,
  resolveAgentTypeForSessionCreation,
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
  getAvailableModes: vi.fn(),
}));

const configApiMocks = vi.hoisted(() => ({
  getConfig: vi.fn(),
}));

const sessionApiMocks = vi.hoisted(() => ({
  archiveSession: vi.fn(),
}));

const persistenceMocks = vi.hoisted(() => ({
  touchSessionActivity: vi.fn(),
  cleanupSaveState: vi.fn(),
  cleanupSessionBuffers: vi.fn(),
}));

const stateMachineMocks = vi.hoisted(() => ({
  delete: vi.fn(),
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: agentApiMocks,
}));

vi.mock('@/infrastructure/api/service-api/ConfigAPI', () => ({
  configAPI: configApiMocks,
}));

vi.mock('@/infrastructure/api/service-api/SessionAPI', () => ({
  sessionAPI: sessionApiMocks,
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

vi.mock('./TextChunkModule', () => ({
  cleanupSessionBuffers: persistenceMocks.cleanupSessionBuffers,
}));

vi.mock('../../state-machine', () => ({
  stateMachineManager: stateMachineMocks,
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

function createContext(
  session: Session,
  options?: {
    additionalSessions?: Session[];
    activeSessionId?: string | null;
    deleteSessionImpl?: (
      sessionId: string,
      options?: { nextActiveSessionId?: string | null },
    ) => Promise<void> | void;
    removeSessionImpl?: (
      sessionId: string,
      options?: { nextActiveSessionId?: string | null },
    ) => string[] | void;
    getCascadeSessionIdsImpl?: (sessionId: string) => string[];
  },
) {
  const initialSessions = new Map<string, Session>([
    [session.sessionId, session],
    ...((options?.additionalSessions ?? []).map(extra => [extra.sessionId, extra] as const)),
  ]);
  let state = {
    sessions: initialSessions,
    activeSessionId: options?.activeSessionId ?? null as string | null,
  };
  const processingManager = {
    clearSessionStatus: vi.fn(),
  };
  const flowChatStore = {
    getState: () => state,
    createSession: vi.fn((
      sessionId: string,
      config?: Record<string, unknown>,
      _unused?: unknown,
      title?: string,
      _maxContextTokens?: number,
      agentType?: string,
      workspacePath?: string,
      remoteConnectionId?: string,
      remoteSshHost?: string,
    ) => {
      const nextSession = createSession({
        sessionId,
        title: title ?? sessionId,
        isHistorical: false,
        historyState: 'ready',
        config: {
          agentType: agentType ?? (config?.agentType as string | undefined) ?? 'agentic',
        },
        mode: agentType ?? 'agentic',
        workspacePath: workspacePath ?? (config?.workspacePath as string | undefined) ?? session.workspacePath,
        remoteConnectionId,
        remoteSshHost,
      });
      state = {
        ...state,
        sessions: new Map(state.sessions).set(sessionId, nextSession),
        activeSessionId: sessionId,
      };
    }),
    switchSession: vi.fn((sessionId: string) => {
      state = { ...state, activeSessionId: sessionId };
    }),
    loadSessionHistory: vi.fn(),
    getCascadeSessionIds: vi.fn((sessionId: string) => (
      options?.getCascadeSessionIdsImpl?.(sessionId) ?? [sessionId]
    )),
    deleteSession: vi.fn(async (
      sessionId: string,
      deleteOptions?: { nextActiveSessionId?: string | null },
    ) => {
      if (options?.deleteSessionImpl) {
        await options.deleteSessionImpl(sessionId, deleteOptions);
        return;
      }
      const nextSessions = new Map(state.sessions);
      nextSessions.delete(sessionId);
      state = {
        ...state,
        sessions: nextSessions,
        activeSessionId: state.activeSessionId === sessionId
          ? deleteOptions && 'nextActiveSessionId' in deleteOptions
            ? deleteOptions.nextActiveSessionId ?? null
            : null
          : state.activeSessionId,
      };
    }),
    removeSession: vi.fn((
      sessionId: string,
      removeOptions?: { nextActiveSessionId?: string | null },
    ) => {
      if (options?.removeSessionImpl) {
        return options.removeSessionImpl(sessionId, removeOptions) ?? [sessionId];
      }
      const removedSessionIds = options?.getCascadeSessionIdsImpl?.(sessionId) ?? [sessionId];
      const removedSessionIdSet = new Set(removedSessionIds);
      const nextSessions = new Map(state.sessions);
      removedSessionIds.forEach(id => nextSessions.delete(id));
      state = {
        ...state,
        sessions: nextSessions,
        activeSessionId: state.activeSessionId && removedSessionIdSet.has(state.activeSessionId)
          ? removeOptions && 'nextActiveSessionId' in removeOptions
            ? removeOptions.nextActiveSessionId ?? null
            : null
          : state.activeSessionId,
      };
      return removedSessionIds;
    }),
    setState: vi.fn((updater: any) => {
      state = updater(state);
    }),
  };

  return {
    context: {
      flowChatStore,
      processingManager,
      pendingHistoryLoads: new Map<string, Promise<void>>(),
      pendingContextRestores: new Map<string, Promise<void>>(),
    } as any,
    flowChatStore,
    processingManager,
  };
}

describe('resolveAgentTypeForSessionCreation', () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it('uses the configured default mode for internal agentic session creation', async () => {
    configApiMocks.getConfig.mockResolvedValue('PlannerPlus');
    agentApiMocks.getAvailableModes.mockResolvedValue([
      { id: 'agentic' },
      { id: 'PlannerPlus' },
    ]);

    await expect(resolveAgentTypeForSessionCreation('agentic', null)).resolves.toBe('PlannerPlus');
  });

  it('does not override explicit non-agentic modes', async () => {
    await expect(resolveAgentTypeForSessionCreation('Cowork', null)).resolves.toBe('Cowork');

    expect(configApiMocks.getConfig).not.toHaveBeenCalled();
    expect(agentApiMocks.getAvailableModes).not.toHaveBeenCalled();
  });

  it('falls back to agentic when the configured default mode is unavailable', async () => {
    configApiMocks.getConfig.mockResolvedValue('MissingMode');
    agentApiMocks.getAvailableModes.mockResolvedValue([{ id: 'agentic' }]);

    await expect(resolveAgentTypeForSessionCreation('agentic', null)).resolves.toBe('agentic');
  });
});

describe('createChatSession', () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it('dedupes concurrent creates before model config loading resolves', async () => {
    // This keeps the first create suspended in the model config path while the
    // second create enters with the same creation key.
    const modelConfig = createDeferred<Record<string, unknown>>();
    configApiMocks.getConfig.mockImplementation(async (key: string) => {
      if (key === 'chat.default_mode') {
        return null;
      }
      await modelConfig.promise;
      return key === 'ai.models' ? [] : {};
    });
    agentApiMocks.getAvailableModes.mockResolvedValue([{ id: 'agentic' }]);
    agentApiMocks.createSession.mockResolvedValue({ sessionId: 'created-1' });

    const { context } = createContext(createSession({
      workspacePath: '/home/wsp/projects/Test',
    }));

    const firstCreate = createChatSession(context, { workspacePath: '/home/wsp/projects/Test' }, 'agentic');
    const secondCreate = createChatSession(context, { workspacePath: '/home/wsp/projects/Test' }, 'agentic');

    await Promise.resolve();
    expect(agentApiMocks.createSession).not.toHaveBeenCalled();

    modelConfig.resolve({});
    await expect(Promise.all([firstCreate, secondCreate])).resolves.toEqual([
      'created-1',
      'created-1',
    ]);

    expect(agentApiMocks.createSession).toHaveBeenCalledTimes(1);
  });
});

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

  it('retries a reused preload that stale-skipped after explicit activation', async () => {
    const stalePreload = createDeferred<void>();
    const retryLoad = createDeferred<void>();
    const { context, flowChatStore } = createContext(createSession());
    context.pendingHistoryLoads.set('history-other', Promise.resolve());
    persistenceMocks.touchSessionActivity.mockResolvedValue(undefined);
    flowChatStore.loadSessionHistory
      .mockReturnValueOnce(stalePreload.promise)
      .mockImplementationOnce(async () => {
        await retryLoad.promise;
        flowChatStore.setState((prev: any) => {
          const session = prev.sessions.get('history-1');
          return {
            ...prev,
            sessions: new Map(prev.sessions).set('history-1', {
              ...session,
              isHistorical: false,
              historyState: 'ready',
              dialogTurns: [{
                id: 'turn-1',
                userMessage: { id: 'user-1', content: 'Restored prompt', timestamp: 1 },
                modelRounds: [],
                status: 'completed',
              }],
            }),
          };
        });
      });

    preloadHistoricalSessionForOpen(context, 'history-1');
    await Promise.resolve();

    expect(flowChatStore.loadSessionHistory).toHaveBeenCalledTimes(1);

    dispatchHistorySessionOpenIntent('history-1', 'Saved session');
    const switching = switchChatSession(context, 'history-1');
    await Promise.resolve();

    expect(flowChatStore.switchSession).toHaveBeenCalledWith('history-1');

    stalePreload.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(flowChatStore.loadSessionHistory).toHaveBeenCalledTimes(2);

    retryLoad.resolve();
    await switching;

    expect(context.flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      isHistorical: false,
      historyState: 'ready',
    });
  });

  it('does not retry a reused stale preload after a newer switch request supersedes it', async () => {
    const stalePreload = createDeferred<void>();
    const newerSwitchLoad = createDeferred<void>();
    const { context, flowChatStore } = createContext(createSession());
    context.pendingHistoryLoads.set('history-other', Promise.resolve());
    flowChatStore.setState((prev: any) => ({
      ...prev,
      sessions: new Map(prev.sessions).set('history-2', createSession({
        sessionId: 'history-2',
        title: 'Newer target',
      })),
    }));
    persistenceMocks.touchSessionActivity.mockResolvedValue(undefined);
    flowChatStore.loadSessionHistory
      .mockReturnValueOnce(stalePreload.promise)
      .mockImplementationOnce(async () => {
        await newerSwitchLoad.promise;
        flowChatStore.setState((prev: any) => {
          const session = prev.sessions.get('history-2');
          return {
            ...prev,
            sessions: new Map(prev.sessions).set('history-2', {
              ...session,
              isHistorical: false,
              historyState: 'ready',
              dialogTurns: [{
                id: 'turn-2',
                userMessage: { id: 'user-2', content: 'Newer prompt', timestamp: 1 },
                modelRounds: [],
                status: 'completed',
              }],
            }),
          };
        });
      });

    preloadHistoricalSessionForOpen(context, 'history-1');
    await Promise.resolve();

    dispatchHistorySessionOpenIntent('history-1', 'Saved session');
    const firstSwitch = switchChatSession(context, 'history-1');
    await Promise.resolve();
    expect(flowChatStore.switchSession).toHaveBeenCalledWith('history-1');

    const secondSwitch = switchChatSession(context, 'history-2');
    await Promise.resolve();
    expect(flowChatStore.loadSessionHistory).toHaveBeenCalledTimes(2);

    stalePreload.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(flowChatStore.loadSessionHistory).toHaveBeenCalledTimes(2);

    newerSwitchLoad.resolve();
    await firstSwitch;
    await secondSwitch;

    expect(flowChatStore.switchSession).toHaveBeenLastCalledWith('history-2');
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

  it('returns to the welcome state after deleting an empty new active session', async () => {
    const activeSession = createSession({
      sessionId: 'active-1',
      title: 'Current session',
      isHistorical: false,
      historyState: 'new',
    });
    const fallbackSession = createSession({
      sessionId: 'history-2',
      title: 'Assistant session',
    });
    const { context, flowChatStore, processingManager } = createContext(activeSession, {
      additionalSessions: [fallbackSession],
      activeSessionId: 'active-1',
      deleteSessionImpl: async (
        deletedSessionId: string,
        deleteOptions?: { nextActiveSessionId?: string | null },
      ) => {
        expect(deletedSessionId).toBe('active-1');
        expect(deleteOptions).toEqual({ nextActiveSessionId: null });
        flowChatStore.setState((prev: any) => {
          const nextSessions = new Map(prev.sessions);
          nextSessions.delete(deletedSessionId);
          return {
            ...prev,
            sessions: nextSessions,
            activeSessionId: deleteOptions && 'nextActiveSessionId' in deleteOptions
              ? deleteOptions.nextActiveSessionId ?? null
              : 'history-2',
          };
        });
      },
    });
    persistenceMocks.touchSessionActivity.mockResolvedValueOnce(undefined);

    const deleting = deleteChatSession(context, 'active-1');
    await Promise.resolve();
    await Promise.resolve();

    expect(flowChatStore.deleteSession).toHaveBeenCalledWith(
      'active-1',
      { nextActiveSessionId: null },
    );
    await deleting;

    expect(flowChatStore.loadSessionHistory).not.toHaveBeenCalled();
    expect(flowChatStore.getState().activeSessionId).toBeNull();
    expect(processingManager.clearSessionStatus).toHaveBeenCalledWith('active-1');
    expect(persistenceMocks.cleanupSaveState).toHaveBeenCalledWith(context, 'active-1');
  });

  it('returns to the welcome state after deleting a non-empty active session', async () => {
    const activeSession = createSession({
      sessionId: 'active-1',
      title: 'Current session',
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [{
        id: 'turn-1',
        userMessage: { id: 'user-1', content: 'hello', timestamp: 1 },
        modelRounds: [],
        status: 'completed',
      } as any],
    });
    const fallbackSession = createSession({
      sessionId: 'history-2',
      title: 'Assistant session',
    });
    const { context, flowChatStore, processingManager } = createContext(activeSession, {
      additionalSessions: [fallbackSession],
      activeSessionId: 'active-1',
      deleteSessionImpl: async (
        deletedSessionId: string,
        deleteOptions?: { nextActiveSessionId?: string | null },
      ) => {
        expect(deletedSessionId).toBe('active-1');
        expect(deleteOptions).toEqual({ nextActiveSessionId: null });
        flowChatStore.setState((prev: any) => {
          const nextSessions = new Map(prev.sessions);
          nextSessions.delete(deletedSessionId);
          return {
            ...prev,
            sessions: nextSessions,
            activeSessionId: deleteOptions && 'nextActiveSessionId' in deleteOptions
              ? deleteOptions.nextActiveSessionId ?? null
              : 'history-2',
          };
        });
      },
    });
    persistenceMocks.touchSessionActivity.mockResolvedValueOnce(undefined);

    const deleting = deleteChatSession(context, 'active-1');
    await Promise.resolve();

    expect(flowChatStore.deleteSession).toHaveBeenCalledWith(
      'active-1',
      { nextActiveSessionId: null },
    );
    await deleting;

    expect(flowChatStore.loadSessionHistory).not.toHaveBeenCalled();
    expect(flowChatStore.switchSession).not.toHaveBeenCalled();
    expect(flowChatStore.getState().activeSessionId).toBeNull();
    expect(processingManager.clearSessionStatus).toHaveBeenCalledWith('active-1');
    expect(persistenceMocks.cleanupSaveState).toHaveBeenCalledWith(context, 'active-1');
  });

  it('returns to the welcome state after archiving an active session', async () => {
    const activeSession = createSession({
      sessionId: 'active-1',
      title: 'Current session',
      isHistorical: false,
      historyState: 'ready',
    });
    const fallbackSession = createSession({
      sessionId: 'history-2',
      title: 'Assistant session',
    });
    const { context, flowChatStore, processingManager } = createContext(activeSession, {
      additionalSessions: [fallbackSession],
      activeSessionId: 'active-1',
    });
    sessionApiMocks.archiveSession.mockResolvedValueOnce(undefined);

    await archiveChatSession(context, 'active-1');

    expect(sessionApiMocks.archiveSession).toHaveBeenCalledWith(
      'active-1',
      'D:/workspace/BitFun',
      undefined,
      undefined,
    );
    expect(flowChatStore.removeSession).toHaveBeenCalledWith(
      'active-1',
      { nextActiveSessionId: null },
    );
    expect(flowChatStore.getState().activeSessionId).toBeNull();
    expect(stateMachineMocks.delete).toHaveBeenCalledWith('active-1');
    expect(processingManager.clearSessionStatus).toHaveBeenCalledWith('active-1');
    expect(persistenceMocks.cleanupSaveState).toHaveBeenCalledWith(context, 'active-1');
    expect(persistenceMocks.cleanupSessionBuffers).toHaveBeenCalledWith(context, 'active-1');
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

  it('restores view-restored subagent sessions as internal coordinator sessions before send', async () => {
    const { context } = createContext(createSession({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
      dialogTurns: [{ id: 'turn-1' } as any],
      sessionKind: 'subagent',
      parentSessionId: 'parent-1',
      subagentType: 'GeneralPurpose',
    } as any));
    agentApiMocks.ensureCoordinatorSession.mockResolvedValueOnce(undefined);

    await ensureBackendSession(context, 'history-1');

    expect(agentApiMocks.ensureCoordinatorSession).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: 'history-1',
        includeInternal: true,
      }),
    );
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

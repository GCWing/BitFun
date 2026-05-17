import { afterEach, describe, expect, it, vi } from 'vitest';
import { ensureBackendSession, switchChatSession } from './SessionModule';
import type { Session } from '../../types/flow-chat';

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
  afterEach(() => {
    vi.clearAllMocks();
  });

  it('switches to a historical session immediately while hydrating in the background', async () => {
    const load = createDeferred<void>();
    const { context, flowChatStore } = createContext(createSession());
    flowChatStore.loadSessionHistory.mockReturnValueOnce(load.promise);
    persistenceMocks.touchSessionActivity.mockResolvedValueOnce(undefined);

    await switchChatSession(context, 'history-1');

    expect(flowChatStore.switchSession).toHaveBeenCalledWith('history-1');
    expect(flowChatStore.loadSessionHistory).toHaveBeenCalledTimes(1);

    load.resolve();
    await load.promise;
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
});

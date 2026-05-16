import { afterEach, describe, expect, it, vi } from 'vitest';
import { flowChatStore } from './FlowChatStore';
import type { FlowChatState, Session } from '../types/flow-chat';

const apiMocks = vi.hoisted(() => ({
  listSessions: vi.fn(),
  loadSessionTurns: vi.fn(),
  saveSessionTurn: vi.fn(),
  restoreSession: vi.fn(),
}));

const configManagerMock = vi.hoisted(() => ({
  getConfig: vi.fn(async (path: string) => {
    if (path === 'ai.models') return [];
    if (path === 'ai.default_models') return {};
    return undefined;
  }),
}));

const stateMachineManagerMock = vi.hoisted(() => ({
  getOrCreate: vi.fn(),
  reset: vi.fn(),
}));

vi.mock('@/infrastructure/api', () => ({
  sessionAPI: {
    listSessions: apiMocks.listSessions,
    loadSessionTurns: apiMocks.loadSessionTurns,
    saveSessionTurn: apiMocks.saveSessionTurn,
  },
  agentAPI: {
    restoreSession: apiMocks.restoreSession,
  },
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: configManagerMock,
}));

vi.mock('../state-machine', () => ({
  stateMachineManager: stateMachineManagerMock,
}));

const resetStore = () => {
  flowChatStore.setState((): FlowChatState => ({
    sessions: new Map(),
    activeSessionId: null,
  }));
  flowChatStore.registerPersistUnreadCompletionCallback(() => {});
};

const createSession = (overrides: Partial<Session> = {}): Session => ({
  sessionId: 'session-1',
  title: 'Session 1',
  dialogTurns: [],
  status: 'idle',
  config: { agentType: 'agentic' },
  createdAt: 1,
  lastActiveAt: 1,
  error: null,
  isHistorical: false,
  todos: [],
  maxContextTokens: 128128,
  mode: 'agentic',
  workspacePath: 'D:/workspace/BitFun',
  isTransient: false,
  ...overrides,
});

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

describe('FlowChatStore metadata persistence callbacks', () => {
  afterEach(() => {
    resetStore();
  });

  it('persists unread completion clear only when the session state changes', () => {
    const persist = vi.fn();
    const session = createSession({ hasUnreadCompletion: 'completed' });

    flowChatStore.setState(() => ({
      sessions: new Map([[session.sessionId, session]]),
      activeSessionId: session.sessionId,
    }));
    flowChatStore.registerPersistUnreadCompletionCallback(persist);

    flowChatStore.clearSessionUnreadCompletion(session.sessionId);
    flowChatStore.clearSessionUnreadCompletion(session.sessionId);

    expect(persist).toHaveBeenCalledTimes(1);
    expect(persist).toHaveBeenCalledWith(session.sessionId, undefined);
  });

  it('persists attention clear only when the session state changes', () => {
    const persist = vi.fn();
    const session = createSession({ needsUserAttention: 'ask_user' });

    flowChatStore.setState(() => ({
      sessions: new Map([[session.sessionId, session]]),
      activeSessionId: session.sessionId,
    }));
    flowChatStore.registerPersistUnreadCompletionCallback(persist);

    flowChatStore.clearSessionNeedsAttention(session.sessionId);
    flowChatStore.clearSessionNeedsAttention(session.sessionId);

    expect(persist).toHaveBeenCalledTimes(1);
    expect(persist).toHaveBeenCalledWith(session.sessionId, undefined);
  });
});

describe('FlowChatStore local usage reports', () => {
  afterEach(() => {
    resetStore();
  });

  it('inserts a local usage report as user-visible content', () => {
    const session = createSession({ lastActiveAt: 1234 });
    flowChatStore.setState(() => ({
      sessions: new Map([[session.sessionId, session]]),
      activeSessionId: session.sessionId,
    }));

    const turn = flowChatStore.addLocalUsageReportTurn({
      sessionId: session.sessionId,
      markdown: '# Session Usage Report',
      reportId: 'usage-1',
      schemaVersion: 1,
      generatedAt: 10,
    });

    const stored = flowChatStore.getState().sessions.get(session.sessionId)?.dialogTurns[0];
    expect(turn).not.toBeNull();
    expect(stored?.kind).toBe('local_command');
    expect(stored?.userMessage.content).toBe('# Session Usage Report');
    expect(stored?.userMessage.metadata).toMatchObject({
      localCommandKind: 'usage_report',
      modelVisible: false,
    });
    expect(flowChatStore.getState().sessions.get(session.sessionId)?.lastActiveAt)
      .toBe(1234);
  });

  it('can update local usage reports without touching session activity', () => {
    const session = createSession({ lastActiveAt: 4321 });
    flowChatStore.setState(() => ({
      sessions: new Map([[session.sessionId, session]]),
      activeSessionId: session.sessionId,
    }));

    const turn = flowChatStore.addLocalUsageReportTurn({
      sessionId: session.sessionId,
      markdown: '# Loading',
      reportId: 'usage-1',
      schemaVersion: 1,
      generatedAt: 10,
      status: 'loading',
    });

    expect(turn).not.toBeNull();
    flowChatStore.updateDialogTurn(
      session.sessionId,
      turn!.id,
      current => ({
        ...current,
        status: 'completed',
        userMessage: {
          ...current.userMessage,
          content: '# Complete',
        },
      }),
      { touchActivity: false },
    );

    const stored = flowChatStore.getState().sessions.get(session.sessionId);
    expect(stored?.dialogTurns[0].userMessage.content).toBe('# Complete');
    expect(stored?.lastActiveAt).toBe(4321);
  });

  it('appends repeated usage reports as separate snapshots', () => {
    const session = createSession();
    flowChatStore.setState(() => ({
      sessions: new Map([[session.sessionId, session]]),
      activeSessionId: session.sessionId,
    }));

    flowChatStore.addLocalUsageReportTurn({
      sessionId: session.sessionId,
      markdown: '# Usage 1',
      reportId: 'usage-1',
      schemaVersion: 1,
      generatedAt: 10,
    });
    flowChatStore.addLocalUsageReportTurn({
      sessionId: session.sessionId,
      markdown: '# Usage 2',
      reportId: 'usage-2',
      schemaVersion: 1,
      generatedAt: 20,
    });

    const turns = flowChatStore.getState().sessions.get(session.sessionId)?.dialogTurns || [];
    expect(turns).toHaveLength(2);
    expect(turns.map(turn => turn.id)).toEqual([
      'local-usage-usage-1',
      'local-usage-usage-2',
    ]);
  });
});

describe('FlowChatStore historical session hydration state', () => {
  afterEach(() => {
    resetStore();
    vi.clearAllMocks();
  });

  it('loads persisted metadata as metadata-only historical sessions', async () => {
    apiMocks.listSessions.mockResolvedValueOnce([
      {
        sessionId: 'history-1',
        title: 'Saved session',
        agentType: 'agentic',
        modelName: 'auto',
        createdAt: 10,
        lastActiveAt: 20,
      },
    ]);

    await flowChatStore.initializeFromDisk('D:/workspace/BitFun');

    const session = flowChatStore.getState().sessions.get('history-1');
    expect(session).toMatchObject({
      sessionId: 'history-1',
      isHistorical: true,
      historyState: 'metadata-only',
      dialogTurns: [],
    });
  });

  it('loads model config once while processing multiple persisted sessions', async () => {
    configManagerMock.getConfig.mockImplementation(async (path: string) => {
      if (path === 'ai.models') return [{ id: 'primary-model', context_window: 256000 }];
      if (path === 'ai.default_models') return { primary: 'primary-model' };
      return undefined;
    });
    apiMocks.listSessions.mockResolvedValueOnce([
      {
        sessionId: 'history-1',
        title: 'Saved session 1',
        agentType: 'agentic',
        createdAt: 10,
        lastActiveAt: 20,
      },
      {
        sessionId: 'history-2',
        title: 'Saved session 2',
        agentType: 'agentic',
        createdAt: 11,
        lastActiveAt: 21,
      },
    ]);

    await flowChatStore.initializeFromDisk('D:/workspace/BitFun');

    const configPaths = configManagerMock.getConfig.mock.calls.map(([path]) => path);
    expect(configPaths.filter(path => path === 'ai.models')).toHaveLength(1);
    expect(configPaths.filter(path => path === 'ai.default_models')).toHaveLength(1);
    expect(flowChatStore.getState().sessions.get('history-1')?.maxContextTokens).toBe(256000);
    expect(flowChatStore.getState().sessions.get('history-2')?.maxContextTokens).toBe(256000);
  });

  it('skips one bad metadata entry without dropping the rest of the session list', async () => {
    apiMocks.listSessions.mockResolvedValueOnce([
      {
        sessionId: 'bad-1',
        title: 'Bad session',
        agentType: 'agentic',
        createdAt: 10,
        lastActiveAt: 20,
      },
      {
        sessionId: 'good-1',
        title: 'Good session',
        agentType: 'agentic',
        createdAt: 11,
        lastActiveAt: 21,
      },
    ]);
    stateMachineManagerMock.getOrCreate.mockImplementation((sessionId: string) => {
      if (sessionId === 'bad-1') {
        throw new Error('bad metadata');
      }
      return {};
    });

    await flowChatStore.initializeFromDisk('D:/workspace/BitFun');

    expect(flowChatStore.getState().sessions.has('bad-1')).toBe(false);
    expect(flowChatStore.getState().sessions.get('good-1')).toMatchObject({
      sessionId: 'good-1',
      historyState: 'metadata-only',
    });
  });

  it('marks historical sessions hydrating while turns are loading and ready after completion', async () => {
    const turns = createDeferred<any[]>();
    apiMocks.restoreSession.mockResolvedValueOnce(undefined);
    apiMocks.loadSessionTurns.mockReturnValueOnce(turns.promise);
    flowChatStore.setState(() => ({
      sessions: new Map([
        ['history-1', createSession({
          sessionId: 'history-1',
          isHistorical: true,
          historyState: 'metadata-only',
        })],
      ]),
      activeSessionId: 'history-1',
    }));

    const load = flowChatStore.loadSessionHistory('history-1', 'D:/workspace/BitFun');
    await flushAsyncWork();

    expect(flowChatStore.getState().sessions.get('history-1')?.historyState).toBe('hydrating');

    turns.resolve([]);
    await load;

    expect(flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [],
    });
  });

  it('marks historical sessions failed when hydrate fails', async () => {
    apiMocks.restoreSession.mockResolvedValueOnce(undefined);
    apiMocks.loadSessionTurns.mockRejectedValueOnce(new Error('turn load failed'));
    flowChatStore.setState(() => ({
      sessions: new Map([
        ['history-1', createSession({
          sessionId: 'history-1',
          isHistorical: true,
          historyState: 'metadata-only',
        })],
      ]),
      activeSessionId: 'history-1',
    }));

    await expect(
      flowChatStore.loadSessionHistory('history-1', 'D:/workspace/BitFun')
    ).rejects.toThrow('turn load failed');

    expect(flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      isHistorical: true,
      historyState: 'failed',
    });
  });

  it('does not change the active session when an older hydrate completes', async () => {
    apiMocks.restoreSession.mockResolvedValueOnce(undefined);
    apiMocks.loadSessionTurns.mockResolvedValueOnce([]);
    flowChatStore.setState(() => ({
      sessions: new Map([
        ['history-1', createSession({
          sessionId: 'history-1',
          isHistorical: true,
          historyState: 'metadata-only',
        })],
        ['history-2', createSession({
          sessionId: 'history-2',
          isHistorical: true,
          historyState: 'metadata-only',
        })],
      ]),
      activeSessionId: 'history-2',
    }));

    await flowChatStore.loadSessionHistory('history-1', 'D:/workspace/BitFun');

    expect(flowChatStore.getState().activeSessionId).toBe('history-2');
    expect(flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      isHistorical: false,
      historyState: 'ready',
    });
  });

  it('does not restore ACP historical sessions through the normal backend path', async () => {
    apiMocks.loadSessionTurns.mockResolvedValueOnce([]);
    flowChatStore.setState(() => ({
      sessions: new Map([
        ['acp-1', createSession({
          sessionId: 'acp-1',
          isHistorical: true,
          historyState: 'metadata-only',
          mode: 'acp:test',
          config: { agentType: 'acp:test' },
        })],
      ]),
      activeSessionId: 'acp-1',
    }));

    await flowChatStore.loadSessionHistory('acp-1', 'D:/workspace/BitFun');

    expect(apiMocks.restoreSession).not.toHaveBeenCalled();
  });
});

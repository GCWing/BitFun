import { afterEach, describe, expect, it, vi } from 'vitest';
import { flowChatStore } from './FlowChatStore';
import type { FlowChatState, Session } from '../types/flow-chat';

const apiMocks = vi.hoisted(() => ({
  listSessions: vi.fn(),
  loadSessionTurns: vi.fn(),
  saveSessionTurn: vi.fn(),
  restoreSession: vi.fn(),
  restoreSessionView: vi.fn(),
  restoreSessionWithTurns: vi.fn(),
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
    get restoreSessionView() {
      return apiMocks.restoreSessionView;
    },
    restoreSessionWithTurns: apiMocks.restoreSessionWithTurns,
  },
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: configManagerMock,
}));

vi.mock('../state-machine', () => ({
  stateMachineManager: stateMachineManagerMock,
}));

const resetStore = () => {
  const metadataListRequests = (flowChatStore as any).metadataListRequests as
    | Map<string, { cleanupTimer?: ReturnType<typeof setTimeout> }>
    | undefined;
  metadataListRequests?.forEach(request => {
    if (request.cleanupTimer) {
      clearTimeout(request.cleanupTimer);
    }
  });
  metadataListRequests?.clear();
  ((flowChatStore as any).unsupportedRestoreCommands as Set<string> | undefined)?.clear();
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
    if (typeof apiMocks.restoreSessionView !== 'function') {
      (apiMocks as any).restoreSessionView = vi.fn();
    }
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

  it('reuses an in-flight metadata list for the same workspace and remote identity', async () => {
    const sessions = createDeferred<any[]>();
    apiMocks.listSessions.mockReturnValueOnce(sessions.promise);

    const firstLoad = flowChatStore.initializeFromDisk(
      'D:/workspace/BitFun',
      undefined,
      undefined,
      'first-source'
    );
    const secondLoad = flowChatStore.initializeFromDisk(
      'D:/workspace/BitFun',
      undefined,
      undefined,
      'second-source'
    );

    await vi.waitFor(() => {
      expect(apiMocks.listSessions).toHaveBeenCalledTimes(1);
    });

    sessions.resolve([
      {
        sessionId: 'history-1',
        title: 'Saved session',
        agentType: 'agentic',
        createdAt: 10,
        lastActiveAt: 20,
      },
    ]);

    await Promise.all([firstLoad, secondLoad]);

    expect(apiMocks.listSessions).toHaveBeenCalledTimes(1);
    expect(flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      sessionId: 'history-1',
      historyState: 'metadata-only',
    });
  });

  it('reuses a recently completed metadata list for the same workspace', async () => {
    apiMocks.listSessions.mockResolvedValueOnce([
      {
        sessionId: 'history-1',
        title: 'Saved session',
        agentType: 'agentic',
        createdAt: 10,
        lastActiveAt: 20,
      },
    ]);

    await flowChatStore.initializeFromDisk('D:/workspace/BitFun', undefined, undefined, 'first-source');
    await flowChatStore.initializeFromDisk('D:/workspace/BitFun', undefined, undefined, 'second-source');

    expect(apiMocks.listSessions).toHaveBeenCalledTimes(1);
    expect(flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      sessionId: 'history-1',
      historyState: 'metadata-only',
    });
  });

  it('marks historical sessions hydrating while turns are loading and ready after completion', async () => {
    const turns = createDeferred<any[]>();
    apiMocks.restoreSessionView.mockImplementationOnce(async () => ({
      session: {
        sessionId: 'history-1',
        sessionName: 'History 1',
        agentType: 'agentic',
        state: 'Idle',
        turnCount: 0,
        createdAt: 1,
      },
      turns: await turns.promise,
      contextRestoreState: 'pending',
    }));
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
    apiMocks.restoreSessionView.mockRejectedValueOnce(new Error('restore failed'));
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

    expect(apiMocks.restoreSessionWithTurns).not.toHaveBeenCalled();
    expect(flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      isHistorical: true,
      historyState: 'failed',
    });
  });

  it('does not change the active session when an older hydrate completes', async () => {
    apiMocks.restoreSessionView.mockResolvedValueOnce({
      session: {
        sessionId: 'history-1',
        sessionName: 'History 1',
        agentType: 'agentic',
        state: 'Idle',
        turnCount: 0,
        createdAt: 1,
      },
      turns: [],
      contextRestoreState: 'pending',
    });
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
    expect(apiMocks.restoreSessionView).not.toHaveBeenCalled();
    expect(apiMocks.restoreSessionWithTurns).not.toHaveBeenCalled();
  });

  it('uses view-restored turns without reading the turn files a second time', async () => {
    const visibleOutput = 'complete visible output '.repeat(64);
    const restoredTurn = {
      turnId: 'turn-1',
      turnIndex: 0,
      sessionId: 'history-1',
      timestamp: 1,
      userMessage: { id: 'user-1', content: 'hello', timestamp: 1 },
      modelRounds: [
        {
          id: 'round-1',
          turnId: 'turn-1',
          roundIndex: 0,
          timestamp: 1,
          textItems: [],
          toolItems: [
            {
              id: 'tool-1',
              toolName: 'Bash',
              toolCall: { id: 'call-1', input: { command: 'printf output' } },
              toolResult: {
                result: {
                  stdout: visibleOutput,
                  nested: { stderr: 'also visible' },
                },
                success: true,
                durationMs: 1,
              },
              startTime: 1,
              endTime: 2,
              durationMs: 1,
              status: 'completed',
            },
          ],
          thinkingItems: [],
          startTime: 1,
          endTime: 2,
          durationMs: 1,
          status: 'completed',
        },
      ],
      startTime: 1,
      status: 'completed',
    };
    apiMocks.restoreSessionView.mockResolvedValueOnce({
      session: {
        sessionId: 'history-1',
        sessionName: 'History 1',
        agentType: 'agentic',
        state: 'Idle',
        turnCount: 1,
        createdAt: 1,
      },
      turns: [restoredTurn],
      contextRestoreState: 'pending',
    });
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

    await flowChatStore.loadSessionHistory('history-1', 'D:/workspace/BitFun');

    expect(apiMocks.restoreSessionView).toHaveBeenCalledTimes(1);
    expect(apiMocks.restoreSessionWithTurns).not.toHaveBeenCalled();
    expect(apiMocks.loadSessionTurns).not.toHaveBeenCalled();
    expect(flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      isHistorical: false,
      historyState: 'ready',
    });
    const toolItem = flowChatStore
      .getState()
      .sessions.get('history-1')
      ?.dialogTurns[0]
      ?.modelRounds[0]
      ?.items.find(item => item.type === 'tool') as any;
    expect(toolItem?.toolResult?.result?.stdout).toBe(visibleOutput);
    expect(toolItem?.toolResult?.result?.nested?.stderr).toBe('also visible');
    expect(toolItem?.toolResult?.resultForAssistant).toBeUndefined();
  });

  it('falls back to restoreSessionWithTurns when view restore is unavailable', async () => {
    (apiMocks as any).restoreSessionView = undefined;
    const restoredTurn = {
      turnId: 'turn-1',
      turnIndex: 0,
      sessionId: 'history-1',
      timestamp: 1,
      userMessage: { id: 'user-1', content: 'hello', timestamp: 1 },
      modelRounds: [],
      startTime: 1,
      status: 'completed',
    };
    apiMocks.restoreSessionWithTurns.mockResolvedValueOnce({
      session: {
        sessionId: 'history-1',
        sessionName: 'History 1',
        agentType: 'agentic',
        state: 'Idle',
        turnCount: 1,
        createdAt: 1,
      },
      turns: [restoredTurn],
    });
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

    await flowChatStore.loadSessionHistory('history-1', 'D:/workspace/BitFun');

    expect(apiMocks.restoreSessionWithTurns).toHaveBeenCalledTimes(1);
    expect(apiMocks.loadSessionTurns).not.toHaveBeenCalled();
    expect(flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
    });
  });

  it('falls back to restoreSessionWithTurns when the view restore command is unavailable on the backend', async () => {
    const restoredTurn = {
      turnId: 'turn-1',
      turnIndex: 0,
      sessionId: 'history-1',
      timestamp: 1,
      userMessage: { id: 'user-1', content: 'hello', timestamp: 1 },
      modelRounds: [],
      startTime: 1,
      status: 'completed',
    };
    apiMocks.restoreSessionView.mockRejectedValueOnce(
      new Error('unknown command restore_session_view')
    );
    apiMocks.restoreSessionWithTurns.mockResolvedValueOnce({
      session: {
        sessionId: 'history-1',
        sessionName: 'History 1',
        agentType: 'agentic',
        state: 'Idle',
        turnCount: 1,
        createdAt: 1,
      },
      turns: [restoredTurn],
    });
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

    await flowChatStore.loadSessionHistory('history-1', 'D:/workspace/BitFun');

    expect(apiMocks.restoreSessionView).toHaveBeenCalledTimes(1);
    expect(apiMocks.restoreSessionWithTurns).toHaveBeenCalledTimes(1);
    expect(apiMocks.loadSessionTurns).not.toHaveBeenCalled();
    expect(flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
    });
  });

  it('does not retry an unsupported view restore command for later sessions in the same runtime', async () => {
    const restoredTurn = (sessionId: string) => ({
      turnId: `${sessionId}-turn-1`,
      turnIndex: 0,
      sessionId,
      timestamp: 1,
      userMessage: { id: `${sessionId}-user-1`, content: 'hello', timestamp: 1 },
      modelRounds: [],
      startTime: 1,
      status: 'completed',
    });
    apiMocks.restoreSessionView.mockRejectedValueOnce(
      new Error('unknown command restore_session_view')
    );
    apiMocks.restoreSessionWithTurns
      .mockResolvedValueOnce({
        session: {
          sessionId: 'history-1',
          sessionName: 'History 1',
          agentType: 'agentic',
          state: 'Idle',
          turnCount: 1,
          createdAt: 1,
        },
        turns: [restoredTurn('history-1')],
      })
      .mockResolvedValueOnce({
        session: {
          sessionId: 'history-2',
          sessionName: 'History 2',
          agentType: 'agentic',
          state: 'Idle',
          turnCount: 1,
          createdAt: 1,
        },
        turns: [restoredTurn('history-2')],
      });
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
      activeSessionId: 'history-1',
    }));

    await flowChatStore.loadSessionHistory('history-1', 'D:/workspace/BitFun');
    await flowChatStore.loadSessionHistory('history-2', 'D:/workspace/BitFun');

    expect(apiMocks.restoreSessionView).toHaveBeenCalledTimes(1);
    expect(apiMocks.restoreSessionWithTurns).toHaveBeenCalledTimes(2);
    expect(apiMocks.loadSessionTurns).not.toHaveBeenCalled();
  });

  it('falls back to legacy restore and turn loading when restoreSessionWithTurns is unavailable on the backend', async () => {
    (apiMocks as any).restoreSessionView = undefined;
    const restoredTurn = {
      turnId: 'turn-1',
      turnIndex: 0,
      sessionId: 'history-1',
      timestamp: 1,
      userMessage: { id: 'user-1', content: 'hello', timestamp: 1 },
      modelRounds: [],
      startTime: 1,
      status: 'completed',
    };
    apiMocks.restoreSessionWithTurns.mockRejectedValueOnce(
      new Error('unknown command restore_session_with_turns')
    );
    apiMocks.restoreSession.mockResolvedValueOnce({
      sessionId: 'history-1',
      sessionName: 'History 1',
      agentType: 'agentic',
      state: 'Idle',
      turnCount: 1,
      createdAt: 1,
    });
    apiMocks.loadSessionTurns.mockResolvedValueOnce([restoredTurn]);
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

    await flowChatStore.loadSessionHistory('history-1', 'D:/workspace/BitFun');

    expect(apiMocks.restoreSessionWithTurns).toHaveBeenCalledTimes(1);
    expect(apiMocks.restoreSession).toHaveBeenCalledTimes(1);
    expect(apiMocks.loadSessionTurns).toHaveBeenCalledTimes(1);
    expect(flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'ready',
      dialogTurns: expect.arrayContaining([
        expect.objectContaining({ id: 'turn-1' }),
      ]),
    });
  });

  it('uses view restore when available and marks backend context pending', async () => {
    const restoredTurn = {
      turnId: 'turn-1',
      turnIndex: 0,
      sessionId: 'history-1',
      timestamp: 1,
      userMessage: { id: 'user-1', content: 'hello', timestamp: 1 },
      modelRounds: [],
      startTime: 1,
      status: 'completed',
    };
    apiMocks.restoreSessionView.mockResolvedValueOnce({
      session: {
        sessionId: 'history-1',
        sessionName: 'History 1',
        agentType: 'agentic',
        state: 'Idle',
        turnCount: 1,
        createdAt: 1,
      },
      turns: [restoredTurn],
      contextRestoreState: 'pending',
    });
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

    await flowChatStore.loadSessionHistory('history-1', 'D:/workspace/BitFun');

    expect(apiMocks.restoreSessionView).toHaveBeenCalledTimes(1);
    expect(apiMocks.restoreSessionWithTurns).not.toHaveBeenCalled();
    expect(apiMocks.loadSessionTurns).not.toHaveBeenCalled();
    expect(flowChatStore.getState().sessions.get('history-1')).toMatchObject({
      isHistorical: false,
      historyState: 'ready',
      contextRestoreState: 'pending',
    });
  });
});

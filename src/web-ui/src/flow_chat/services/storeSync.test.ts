import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Session } from '../types/flow-chat';

const syncMocks = vi.hoisted(() => {
  const flowState = {
    sessions: new Map<string, Session>(),
    activeSessionId: null as string | null,
  };
  const listeners = new Set<(state: typeof flowState) => void>();
  const modernState = {
    activeSession: null as Session | null,
    virtualItems: [] as unknown[],
    setActiveSession: vi.fn((session: Session | null) => {
      modernState.activeSession = session;
    }),
    clear: vi.fn(() => {
      modernState.activeSession = null;
    }),
  };

  return {
    flowState,
    listeners,
    modernState,
  };
});

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => syncMocks.flowState,
    subscribe: vi.fn((listener: (state: typeof syncMocks.flowState) => void) => {
      syncMocks.listeners.add(listener);
      return () => {
        syncMocks.listeners.delete(listener);
      };
    }),
  },
}));

vi.mock('../store/modernFlowChatStore', () => ({
  useModernFlowChatStore: {
    getState: () => syncMocks.modernState,
  },
}));

import { startAutoSync, syncSessionToModernStore } from './storeSync';

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

describe('storeSync history session state', () => {
  afterEach(() => {
    syncMocks.flowState.sessions = new Map();
    syncMocks.flowState.activeSessionId = null;
    syncMocks.listeners.clear();
    syncMocks.modernState.activeSession = null;
    syncMocks.modernState.virtualItems = [];
    syncMocks.modernState.setActiveSession.mockClear();
    syncMocks.modernState.clear.mockClear();
  });

  it('preserves historyState when syncing historical sessions to the modern store', () => {
    const session = createSession();
    syncMocks.flowState.sessions = new Map([[session.sessionId, session]]);
    syncMocks.flowState.activeSessionId = session.sessionId;

    syncSessionToModernStore(session.sessionId);

    expect(syncMocks.modernState.setActiveSession).toHaveBeenCalledWith(session);
    expect(syncMocks.modernState.activeSession).toBe(session);
    expect(syncMocks.modernState.activeSession?.historyState).toBe('metadata-only');
  });

  it('repairs a ready active session when the modern item projection is empty', () => {
    const session = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'history-1',
        userMessage: {
          id: 'user-1',
          content: 'Loaded history',
          timestamp: 1,
        },
        modelRounds: [],
        status: 'completed',
        startTime: 1,
      }],
    });
    syncMocks.flowState.sessions = new Map([[session.sessionId, session]]);
    syncMocks.flowState.activeSessionId = session.sessionId;
    syncMocks.modernState.activeSession = session;
    syncMocks.modernState.virtualItems = [];

    syncSessionToModernStore(session.sessionId);

    expect(syncMocks.modernState.setActiveSession).toHaveBeenCalledWith(session);
  });

  it('repairs an empty projection when auto sync starts on a ready active session', () => {
    const session = createSession({
      isHistorical: false,
      historyState: 'ready',
      dialogTurns: [{
        id: 'turn-1',
        sessionId: 'history-1',
        userMessage: {
          id: 'user-1',
          content: 'Loaded history',
          timestamp: 1,
        },
        modelRounds: [],
        status: 'completed',
        startTime: 1,
      }],
    });
    syncMocks.flowState.sessions = new Map([[session.sessionId, session]]);
    syncMocks.flowState.activeSessionId = session.sessionId;
    syncMocks.modernState.activeSession = session;
    syncMocks.modernState.virtualItems = [];

    const unsubscribe = startAutoSync();
    unsubscribe();

    expect(syncMocks.modernState.setActiveSession).toHaveBeenCalledWith(session);
  });
});

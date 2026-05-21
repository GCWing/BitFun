import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Session } from '../types/flow-chat';

const syncMocks = vi.hoisted(() => {
  const flowState = {
    sessions: new Map<string, Session>(),
    activeSessionId: null as string | null,
  };
  const modernState = {
    activeSession: null as Session | null,
    setActiveSession: vi.fn((session: Session | null) => {
      modernState.activeSession = session;
    }),
    clear: vi.fn(() => {
      modernState.activeSession = null;
    }),
  };

  return {
    flowState,
    modernState,
  };
});

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => syncMocks.flowState,
  },
}));

vi.mock('../store/modernFlowChatStore', () => ({
  useModernFlowChatStore: {
    getState: () => syncMocks.modernState,
  },
}));

import { syncSessionToModernStore } from './storeSync';

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
    syncMocks.modernState.activeSession = null;
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
});

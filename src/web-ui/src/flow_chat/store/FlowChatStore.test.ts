import { afterEach, describe, expect, it, vi } from 'vitest';
import { flowChatStore } from './FlowChatStore';
import type { FlowChatState, Session } from '../types/flow-chat';

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
    const session = createSession();
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

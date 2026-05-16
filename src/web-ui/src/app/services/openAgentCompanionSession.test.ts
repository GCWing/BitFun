import { beforeEach, describe, expect, it, vi } from 'vitest';
import { openAgentCompanionSession } from './openAgentCompanionSession';
import type { Session } from '@/flow_chat/types/flow-chat';

const mocks = vi.hoisted(() => ({
  openBtwSessionInAuxPane: vi.fn(),
  openMainSession: vi.fn(() => Promise.resolve()),
  switchSession: vi.fn(),
  sessions: new Map<string, Session>(),
}));

vi.mock('@/flow_chat/services/openBtwSession', () => ({
  openBtwSessionInAuxPane: (...args: unknown[]) => mocks.openBtwSessionInAuxPane(...args),
  openMainSession: (...args: unknown[]) => mocks.openMainSession(...args),
}));

vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  FlowChatStore: {
    getInstance: () => ({
      getState: () => ({
        sessions: mocks.sessions,
      }),
      switchSession: (...args: unknown[]) => mocks.switchSession(...args),
    }),
  },
}));

function createSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'session-1',
    title: 'Session',
    dialogTurns: [],
    status: 'idle',
    config: {},
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    ...overrides,
  } as Session;
}

describe('openAgentCompanionSession', () => {
  beforeEach(() => {
    mocks.openBtwSessionInAuxPane.mockClear();
    mocks.openMainSession.mockClear();
    mocks.switchSession.mockClear();
    mocks.sessions.clear();
  });

  it('opens deep review child sessions in the aux pane instead of switching to the child chat', async () => {
    mocks.sessions.set('deep-review-child', createSession({
      sessionId: 'deep-review-child',
      sessionKind: 'deep_review',
      parentSessionId: 'parent-session',
      workspacePath: 'D:/workspace/project',
    }));

    const opened = await openAgentCompanionSession('deep-review-child');

    expect(opened).toBe(true);
    expect(mocks.openMainSession).toHaveBeenCalledWith('parent-session');
    expect(mocks.openBtwSessionInAuxPane).toHaveBeenCalledWith({
      childSessionId: 'deep-review-child',
      parentSessionId: 'parent-session',
      workspacePath: 'D:/workspace/project',
    });
    expect(mocks.switchSession).not.toHaveBeenCalled();
  });

  it('keeps regular sessions on the main chat route', async () => {
    mocks.sessions.set('session-1', createSession());

    const opened = await openAgentCompanionSession('session-1');

    expect(opened).toBe(true);
    expect(mocks.switchSession).toHaveBeenCalledWith('session-1');
    expect(mocks.openMainSession).not.toHaveBeenCalled();
    expect(mocks.openBtwSessionInAuxPane).not.toHaveBeenCalled();
  });
});

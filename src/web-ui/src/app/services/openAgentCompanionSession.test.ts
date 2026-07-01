import { beforeEach, describe, expect, it, vi } from 'vitest';
import { openAgentCompanionSession } from './openAgentCompanionSession';
import type { Session } from '@/flow_chat/types/flow-chat';

const mocks = vi.hoisted(() => ({
  openBtwSessionInAuxPane: vi.fn(),
  openMainSession: vi.fn(() => Promise.resolve()),
  activateMainSession: vi.fn(() => Promise.resolve(true)),
  sessions: new Map<string, Session>(),
  openedWorkspaces: new Map<string, { id: string; rootPath: string }>(),
  activeWorkspaceId: null as string | null,
  sessionBelongsToWorkspaceNavRow: vi.fn(() => false),
  setActiveWorkspace: vi.fn((id: string) => Promise.resolve({ id })),
}));

vi.mock('@/flow_chat/services/btwSessionPane', () => ({
  openBtwSessionInAuxPane: (...args: unknown[]) => mocks.openBtwSessionInAuxPane(...args),
}));

vi.mock('@/flow_chat/services/sessionActivation', () => ({
  openMainSession: (...args: unknown[]) => mocks.openMainSession(...args),
  activateMainSession: (...args: unknown[]) => mocks.activateMainSession(...args),
}));

vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  FlowChatStore: {
    getInstance: () => ({
      getState: () => ({
        sessions: mocks.sessions,
      }),
    }),
  },
}));

vi.mock('@/infrastructure/services/business/workspaceManager', () => ({
  workspaceManager: {
    getState: () => ({
      openedWorkspaces: mocks.openedWorkspaces,
      activeWorkspaceId: mocks.activeWorkspaceId,
    }),
    setActiveWorkspace: (id: string) => mocks.setActiveWorkspace(id),
  },
}));

vi.mock('@/flow_chat/utils/sessionOrdering', () => ({
  sessionBelongsToWorkspaceNavRow: (...args: unknown[]) =>
    mocks.sessionBelongsToWorkspaceNavRow(...args),
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
    mocks.activateMainSession.mockClear();
    mocks.setActiveWorkspace.mockClear();
    mocks.sessions.clear();
    mocks.openedWorkspaces.clear();
    mocks.activeWorkspaceId = null;
    mocks.sessionBelongsToWorkspaceNavRow.mockClear();
    mocks.sessionBelongsToWorkspaceNavRow.mockReturnValue(false);
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
    expect(mocks.openMainSession).toHaveBeenCalledWith('parent-session', {
      workspaceId: undefined,
      activateWorkspace: undefined,
    });
    expect(mocks.openBtwSessionInAuxPane).toHaveBeenCalledWith({
      childSessionId: 'deep-review-child',
      parentSessionId: 'parent-session',
      workspacePath: 'D:/workspace/project',
    });
    expect(mocks.activateMainSession).not.toHaveBeenCalled();
  });

  it('keeps regular sessions on the main chat route', async () => {
    mocks.sessions.set('session-1', createSession());

    const opened = await openAgentCompanionSession('session-1');

    expect(opened).toBe(true);
    expect(mocks.openMainSession).toHaveBeenCalledWith('session-1', {
      workspaceId: undefined,
      activateWorkspace: undefined,
    });
    expect(mocks.activateMainSession).not.toHaveBeenCalled();
    expect(mocks.openBtwSessionInAuxPane).not.toHaveBeenCalled();
  });

  it('activates the session workspace when it differs from the current workspace', async () => {
    mocks.sessions.set('session-1', createSession({
      sessionId: 'session-1',
      workspacePath: '/home/user/project',
    }));
    mocks.sessionBelongsToWorkspaceNavRow.mockReturnValue(true);
    mocks.openedWorkspaces.set('ws-1', { id: 'ws-1', rootPath: '/home/user/project' });
    mocks.activeWorkspaceId = 'ws-other';

    await openAgentCompanionSession('session-1');

    expect(mocks.openMainSession).toHaveBeenCalledWith('session-1', {
      workspaceId: 'ws-1',
      activateWorkspace: expect.any(Function),
    });
  });

  it('does not activate workspace when session belongs to current workspace', async () => {
    mocks.sessions.set('session-1', createSession({
      sessionId: 'session-1',
      workspacePath: '/home/user/project',
    }));
    mocks.sessionBelongsToWorkspaceNavRow.mockReturnValue(true);
    mocks.openedWorkspaces.set('ws-1', { id: 'ws-1', rootPath: '/home/user/project' });
    mocks.activeWorkspaceId = 'ws-1';

    await openAgentCompanionSession('session-1');

    expect(mocks.openMainSession).toHaveBeenCalledWith('session-1', {
      workspaceId: 'ws-1',
      activateWorkspace: undefined,
    });
    expect(mocks.setActiveWorkspace).not.toHaveBeenCalled();
  });

  it('uses session.workspaceId fast path when workspace is still opened', async () => {
    mocks.sessions.set('session-1', createSession({
      sessionId: 'session-1',
      workspacePath: '/home/user/project',
      workspaceId: 'ws-fast',
    }));
    mocks.openedWorkspaces.set('ws-fast', { id: 'ws-fast', rootPath: '/home/user/project' });
    mocks.activeWorkspaceId = 'ws-other';

    await openAgentCompanionSession('session-1');

    // Fast path should match without calling sessionBelongsToWorkspaceNavRow.
    expect(mocks.sessionBelongsToWorkspaceNavRow).not.toHaveBeenCalled();
    expect(mocks.openMainSession).toHaveBeenCalledWith('session-1', {
      workspaceId: 'ws-fast',
      activateWorkspace: expect.any(Function),
    });
  });

  it('returns false when session does not exist', async () => {
    const opened = await openAgentCompanionSession('nonexistent');

    expect(opened).toBe(false);
    expect(mocks.openMainSession).not.toHaveBeenCalled();
  });
});

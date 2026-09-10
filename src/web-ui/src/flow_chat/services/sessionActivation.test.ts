import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  flowChatState: {
    activeSessionId: null as string | null,
    sessions: new Map<string, unknown>(),
  },
  sceneState: {
    openScene: vi.fn(),
  },
  switchChatSession: vi.fn(),
  workspaceState: {
    activeWorkspaceId: null as string | null,
    currentWorkspace: null as { id: string; rootPath: string } | null,
    openedWorkspaces: new Map(),
  },
  setActiveWorkspace: vi.fn(),
}));

vi.mock('@/infrastructure/services/business/workspaceManager', () => ({
  workspaceManager: { getState: () => mocks.workspaceState, setActiveWorkspace: mocks.setActiveWorkspace },
}));

vi.mock('@/app/services/AppManager', () => ({
  appManager: { updateLayout: vi.fn() },
}));

vi.mock('@/app/stores/sceneStore', () => ({
  useSceneStore: {
    getState: () => mocks.sceneState,
  },
}));

vi.mock('../store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => mocks.flowChatState,
  },
}));

vi.mock('./FlowChatManager', () => ({
  flowChatManager: { switchChatSession: mocks.switchChatSession },
}));

vi.mock('./storeSync', () => ({
  syncSessionToModernStore: vi.fn(),
}));

import { openMainSession } from './sessionActivation';
import { syncSessionToModernStore } from './storeSync';

describe('openMainSession resource activation', () => {
  beforeEach(() => {
    mocks.flowChatState.activeSessionId = null;
    mocks.flowChatState.sessions = new Map();
    vi.clearAllMocks();
    mocks.switchChatSession.mockReset();
    mocks.workspaceState.activeWorkspaceId = null;
    mocks.workspaceState.currentWorkspace = null;
    mocks.workspaceState.openedWorkspaces = new Map();
    mocks.setActiveWorkspace.mockReset();
  });

  it('does not open a scene for a missing or removed session', async () => {
    await openMainSession('missing');
    expect(mocks.sceneState.openScene).not.toHaveBeenCalled();
    expect(mocks.switchChatSession).not.toHaveBeenCalled();
  });

  it('opens an existing selected session and synchronizes its presentation', async () => {
    mocks.flowChatState.activeSessionId = 'active';
    mocks.flowChatState.sessions.set('active', { sessionId: 'active' });

    await openMainSession('active');

    expect(syncSessionToModernStore).toHaveBeenCalledWith('active');
    expect(mocks.sceneState.openScene).toHaveBeenCalledWith('session');
  });

  it('waits for successful activation before opening the scene', async () => {
    mocks.flowChatState.sessions.set('target', { sessionId: 'target' });
    mocks.switchChatSession.mockImplementation(async () => {
      expect(mocks.sceneState.openScene).not.toHaveBeenCalled();
      mocks.flowChatState.activeSessionId = 'target';
    });

    await openMainSession('target');

    expect(mocks.sceneState.openScene).toHaveBeenCalledWith('session');
  });

  it('does not reopen a session removed while activation was pending', async () => {
    mocks.flowChatState.sessions.set('target', { sessionId: 'target' });
    mocks.switchChatSession.mockImplementation(async () => {
      mocks.flowChatState.activeSessionId = 'target';
      mocks.flowChatState.sessions.delete('target');
    });

    await openMainSession('target');

    expect(mocks.sceneState.openScene).not.toHaveBeenCalled();
    expect(syncSessionToModernStore).not.toHaveBeenCalled();
  });

  it('does not open a scene when activation fails', async () => {
    mocks.flowChatState.sessions.set('target', { sessionId: 'target' });
    mocks.switchChatSession.mockRejectedValue(new Error('Host unavailable'));

    await expect(openMainSession('target')).rejects.toThrow('Host unavailable');
    expect(mocks.sceneState.openScene).not.toHaveBeenCalled();
  });

  it('rejects an unavailable remote workspace instead of using the current local directory', async () => {
    const local = { id: 'local', rootPath: '/project' };
    mocks.workspaceState.currentWorkspace = local;
    mocks.workspaceState.openedWorkspaces = new Map([['local', local]]);
    mocks.flowChatState.sessions.set('remote', {
      sessionId: 'remote', workspacePath: '/project', remoteSshHost: 'server',
    });
    await expect(openMainSession('remote')).rejects.toThrow();
    expect(mocks.switchChatSession).not.toHaveBeenCalled();
    expect(mocks.sceneState.openScene).not.toHaveBeenCalled();
  });

  it('orders workspace activation and only opens the latest requested session', async () => {
    const a = { id: 'a', rootPath: '/a' };
    const b = { id: 'b', rootPath: '/b' };
    mocks.workspaceState.openedWorkspaces = new Map([['a', a], ['b', b]]);
    mocks.workspaceState.activeWorkspaceId = 'b';
    mocks.flowChatState.sessions.set('a', { sessionId: 'a', workspaceId: 'a', workspacePath: '/a' });
    mocks.flowChatState.sessions.set('b', { sessionId: 'b', workspaceId: 'b', workspacePath: '/b' });
    let releaseA!: () => void;
    const pendingA = new Promise<void>(resolve => { releaseA = resolve; });
    mocks.setActiveWorkspace.mockImplementation(async (id: string) => {
      if (id === 'a') await pendingA;
      mocks.workspaceState.activeWorkspaceId = id;
    });
    mocks.switchChatSession.mockImplementation(async (id: string) => {
      mocks.flowChatState.activeSessionId = id;
    });
    const first = openMainSession('a');
    await Promise.resolve();
    const second = openMainSession('b');
    releaseA();
    await Promise.all([first, second]);

    expect(mocks.setActiveWorkspace.mock.calls.map(([id]) => id)).toEqual(['a', 'b']);
    expect(mocks.switchChatSession.mock.calls.map(([id]) => id)).toEqual(['b']);
    expect(mocks.workspaceState.activeWorkspaceId).toBe('b');
    expect(mocks.sceneState.openScene).toHaveBeenCalledTimes(1);
  });
});

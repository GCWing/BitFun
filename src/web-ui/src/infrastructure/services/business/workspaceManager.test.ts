import { beforeEach, describe, expect, it, vi } from 'vitest';

const globalStateMocks = vi.hoisted(() => ({
  initializeWorkspaceStartupState: vi.fn(),
  cleanupInvalidWorkspaces: vi.fn(),
  getRecentWorkspaces: vi.fn(),
  getOpenedWorkspaces: vi.fn(),
  getCurrentWorkspace: vi.fn(),
  updateWorkspaceInfo: vi.fn(),
}));

const listenMock = vi.hoisted(() => vi.fn());

vi.mock('../../../shared/types', () => ({
  WorkspaceKind: {
    Normal: 'normal',
    Assistant: 'assistant',
    Remote: 'remote',
  },
  globalStateAPI: globalStateMocks,
  isRemoteWorkspace: (workspace: { workspaceKind?: string } | null) =>
    workspace?.workspaceKind === 'remote',
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('@/shared/utils/startupTrace', () => ({
  startupTrace: {
    markPhase: vi.fn(),
  },
}));

function configureGlobalState(): void {
  globalStateMocks.initializeWorkspaceStartupState.mockResolvedValue({
    cleanupRemovedCount: 0,
    recentWorkspaces: [],
    openedWorkspaces: [],
    currentWorkspace: null,
    legacyRemoteWorkspace: null,
  });
  globalStateMocks.cleanupInvalidWorkspaces.mockResolvedValue(0);
  globalStateMocks.getRecentWorkspaces.mockResolvedValue([]);
  globalStateMocks.getOpenedWorkspaces.mockResolvedValue([]);
  globalStateMocks.getCurrentWorkspace.mockResolvedValue(null);
  globalStateMocks.updateWorkspaceInfo.mockReset();
}

async function getFreshWorkspaceManager() {
  vi.resetModules();
  const { WorkspaceManager } = await import('./workspaceManager');
  (WorkspaceManager as unknown as { instance: unknown }).instance = null;
  return WorkspaceManager.getInstance();
}

async function flushAsyncWork(): Promise<void> {
  await new Promise(resolve => setTimeout(resolve, 0));
}

describe('WorkspaceManager startup initialization', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    configureGlobalState();
  });

  it('does not block startup workspace state on identity listener registration', async () => {
    listenMock.mockReturnValue(new Promise(() => undefined));
    const manager = await getFreshWorkspaceManager();

    const initializePromise = manager.initialize();
    const initializeResult = await Promise.race([
      initializePromise.then(() => 'initialized'),
      new Promise(resolve => setTimeout(() => resolve('timeout'), 20)),
    ]);

    expect(listenMock).toHaveBeenCalledWith('workspace-identity-changed', expect.any(Function));
    expect(initializeResult).toBe('initialized');
    expect(globalStateMocks.initializeWorkspaceStartupState).toHaveBeenCalledTimes(1);
    expect(globalStateMocks.cleanupInvalidWorkspaces).not.toHaveBeenCalled();
    expect(globalStateMocks.getCurrentWorkspace).not.toHaveBeenCalled();
    expect(globalStateMocks.getRecentWorkspaces).not.toHaveBeenCalled();
    expect(globalStateMocks.getOpenedWorkspaces).not.toHaveBeenCalled();
  });

  it('applies identity updates after delayed listener registration completes', async () => {
    const workspace = {
      id: 'assistant-1',
      name: 'Assistant 1',
      rootPath: 'D:/workspace/assistant-1',
      workspaceKind: 'assistant',
      identity: null,
    };
    globalStateMocks.initializeWorkspaceStartupState.mockResolvedValue({
      cleanupRemovedCount: 0,
      recentWorkspaces: [workspace],
      openedWorkspaces: [workspace],
      currentWorkspace: workspace,
      legacyRemoteWorkspace: null,
    });
    globalStateMocks.getCurrentWorkspace.mockResolvedValue(workspace);
    globalStateMocks.getRecentWorkspaces.mockResolvedValue([workspace]);
    globalStateMocks.getOpenedWorkspaces.mockResolvedValue([workspace]);

    let identityHandler:
      | ((event: {
          payload: {
            workspaceId: string;
            workspacePath: string;
            name: string;
            identity: { name: string };
            changedFields: string[];
          };
        }) => void)
      | null = null;
    let resolveListener: ((unlisten: () => void) => void) | null = null;
    listenMock.mockImplementation((_eventName, handler) => {
      identityHandler = handler;
      return new Promise(resolve => {
        resolveListener = resolve;
      });
    });

    const manager = await getFreshWorkspaceManager();
    await manager.initialize();

    expect(manager.getState().currentWorkspace?.name).toBe('Assistant 1');

    resolveListener?.(() => undefined);
    await flushAsyncWork();

    identityHandler?.({
      payload: {
        workspaceId: 'assistant-1',
        workspacePath: 'D:/workspace/assistant-1',
        name: 'Assistant renamed',
        identity: { name: 'Assistant renamed' },
        changedFields: ['name'],
      },
    });

    expect(manager.getState().currentWorkspace?.name).toBe('Assistant renamed');
  });

  it('refreshes workspace identity once the delayed listener is ready after startup', async () => {
    const startupWorkspace = {
      id: 'assistant-1',
      name: 'Assistant 1',
      rootPath: 'D:/workspace/assistant-1',
      workspaceKind: 'assistant',
      identity: null,
    };
    const refreshedWorkspace = {
      ...startupWorkspace,
      name: 'Assistant renamed',
      identity: { name: 'Assistant renamed' },
    };
    globalStateMocks.initializeWorkspaceStartupState.mockResolvedValue({
      cleanupRemovedCount: 0,
      recentWorkspaces: [startupWorkspace],
      openedWorkspaces: [startupWorkspace],
      currentWorkspace: startupWorkspace,
      legacyRemoteWorkspace: null,
    });
    globalStateMocks.getCurrentWorkspace.mockResolvedValue(refreshedWorkspace);
    globalStateMocks.getRecentWorkspaces.mockResolvedValue([refreshedWorkspace]);
    globalStateMocks.getOpenedWorkspaces.mockResolvedValue([refreshedWorkspace]);

    let resolveListener: ((unlisten: () => void) => void) | null = null;
    listenMock.mockReturnValue(new Promise(resolve => {
      resolveListener = resolve;
    }));

    const manager = await getFreshWorkspaceManager();
    await manager.initialize();

    expect(manager.getState().currentWorkspace?.name).toBe('Assistant 1');

    resolveListener?.(() => undefined);
    await flushAsyncWork();

    expect(globalStateMocks.getCurrentWorkspace).toHaveBeenCalledTimes(1);
    expect(manager.getState().currentWorkspace?.name).toBe('Assistant renamed');
  });

  it('refreshes workspace identity when an identity event arrives before startup state is committed', async () => {
    const startupWorkspace = {
      id: 'assistant-1',
      name: 'Assistant 1',
      rootPath: 'D:/workspace/assistant-1',
      workspaceKind: 'assistant',
      identity: null,
    };
    const refreshedWorkspace = {
      ...startupWorkspace,
      name: 'Assistant renamed',
      identity: { name: 'Assistant renamed' },
    };
    let resolveStartupState: ((value: {
      cleanupRemovedCount: number;
      recentWorkspaces: typeof startupWorkspace[];
      openedWorkspaces: typeof startupWorkspace[];
      currentWorkspace: typeof startupWorkspace;
      legacyRemoteWorkspace: null;
    }) => void) | null = null;
    globalStateMocks.initializeWorkspaceStartupState.mockReturnValue(new Promise(resolve => {
      resolveStartupState = resolve;
    }));
    globalStateMocks.getCurrentWorkspace.mockResolvedValue(refreshedWorkspace);
    globalStateMocks.getRecentWorkspaces.mockResolvedValue([refreshedWorkspace]);
    globalStateMocks.getOpenedWorkspaces.mockResolvedValue([refreshedWorkspace]);

    let identityHandler:
      | ((event: {
          payload: {
            workspaceId: string;
            workspacePath: string;
            name: string;
            identity: { name: string };
            changedFields: string[];
          };
        }) => void)
      | null = null;
    listenMock.mockImplementation((_eventName, handler) => {
      identityHandler = handler;
      return Promise.resolve(() => undefined);
    });

    const manager = await getFreshWorkspaceManager();
    const initializePromise = manager.initialize();
    await flushAsyncWork();

    identityHandler?.({
      payload: {
        workspaceId: 'assistant-1',
        workspacePath: 'D:/workspace/assistant-1',
        name: 'Assistant renamed',
        identity: { name: 'Assistant renamed' },
        changedFields: ['name'],
      },
    });

    resolveStartupState?.({
      cleanupRemovedCount: 0,
      recentWorkspaces: [startupWorkspace],
      openedWorkspaces: [startupWorkspace],
      currentWorkspace: startupWorkspace,
      legacyRemoteWorkspace: null,
    });
    await initializePromise;
    await flushAsyncWork();

    expect(globalStateMocks.getCurrentWorkspace).toHaveBeenCalledTimes(1);
    expect(manager.getState().currentWorkspace?.name).toBe('Assistant renamed');
  });

  it('keeps startup workspace state available when identity listener registration fails', async () => {
    listenMock.mockRejectedValue(new Error('listener unavailable'));
    const manager = await getFreshWorkspaceManager();

    await expect(manager.initialize()).resolves.toBeUndefined();

    expect(globalStateMocks.initializeWorkspaceStartupState).toHaveBeenCalledTimes(1);
    expect(manager.getState().loading).toBe(false);
    expect(manager.getState().error).toBeNull();
  });

  it('keeps startup workspace state available when identity listener registration throws synchronously', async () => {
    listenMock.mockImplementation(() => {
      throw new Error('listener unavailable');
    });
    const manager = await getFreshWorkspaceManager();

    await expect(manager.initialize()).resolves.toBeUndefined();

    expect(globalStateMocks.initializeWorkspaceStartupState).toHaveBeenCalledTimes(1);
    expect(manager.getState().loading).toBe(false);
    expect(manager.getState().error).toBeNull();
  });

  it('stores the startup legacy remote workspace snapshot for one reconnect pass', async () => {
    const legacyRemoteWorkspace = {
      connectionId: 'conn-1',
      connectionName: 'Remote',
      remotePath: '/repo',
      sshHost: 'devbox',
    };
    globalStateMocks.initializeWorkspaceStartupState.mockResolvedValue({
      cleanupRemovedCount: 0,
      recentWorkspaces: [],
      openedWorkspaces: [],
      currentWorkspace: null,
      legacyRemoteWorkspace,
    });
    listenMock.mockResolvedValue(() => undefined);
    const manager = await getFreshWorkspaceManager();

    await manager.initialize();

    expect(manager.consumeStartupLegacyRemoteWorkspaceSnapshot()).toEqual({
      available: true,
      workspace: legacyRemoteWorkspace,
    });
    expect(manager.consumeStartupLegacyRemoteWorkspaceSnapshot()).toEqual({
      available: false,
      workspace: null,
    });
  });
});

describe('WorkspaceManager project rename', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    configureGlobalState();
  });

  it('updates current, opened, and recent workspace records together', async () => {
    const workspace = {
      id: 'project-1',
      name: 'Original project',
      rootPath: 'D:/workspace/project-1',
      workspaceKind: 'normal',
    };
    const renamedWorkspace = {
      ...workspace,
      name: 'Renamed project',
    };
    globalStateMocks.initializeWorkspaceStartupState.mockResolvedValue({
      cleanupRemovedCount: 0,
      recentWorkspaces: [workspace],
      openedWorkspaces: [workspace],
      currentWorkspace: workspace,
      legacyRemoteWorkspace: null,
    });
    globalStateMocks.updateWorkspaceInfo.mockResolvedValue(renamedWorkspace);
    listenMock.mockResolvedValue(() => undefined);

    const manager = await getFreshWorkspaceManager();
    await manager.initialize();

    await expect(manager.renameWorkspace('project-1', '  Renamed project  '))
      .resolves.toEqual(renamedWorkspace);

    expect(globalStateMocks.updateWorkspaceInfo).toHaveBeenCalledWith('project-1', {
      name: 'Renamed project',
    });
    const state = manager.getState();
    expect(state.currentWorkspace?.name).toBe('Renamed project');
    expect(state.openedWorkspaces.get('project-1')?.name).toBe('Renamed project');
    expect(state.recentWorkspaces[0]?.name).toBe('Renamed project');
  });
});

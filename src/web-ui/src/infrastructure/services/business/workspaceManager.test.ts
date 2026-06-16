import { beforeEach, describe, expect, it, vi } from 'vitest';

const globalStateMocks = vi.hoisted(() => ({
  initializeWorkspaceStartupState: vi.fn(),
  cleanupInvalidWorkspaces: vi.fn(),
  getRecentWorkspaces: vi.fn(),
  getOpenedWorkspaces: vi.fn(),
  getCurrentWorkspace: vi.fn(),
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
}

async function getFreshWorkspaceManager() {
  vi.resetModules();
  const { WorkspaceManager } = await import('./workspaceManager');
  (WorkspaceManager as unknown as { instance: unknown }).instance = null;
  return WorkspaceManager.getInstance();
}

describe('WorkspaceManager startup initialization', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    configureGlobalState();
  });

  it('waits for the identity listener before loading startup workspace state with one IPC', async () => {
    let resolveListener: ((unlisten: () => void) => void) | null = null;
    listenMock.mockReturnValue(new Promise(resolve => {
      resolveListener = resolve;
    }));
    const manager = await getFreshWorkspaceManager();

    const initializePromise = manager.initialize();
    await new Promise(resolve => setTimeout(resolve, 20));

    expect(listenMock).toHaveBeenCalledWith('workspace-identity-changed', expect.any(Function));
    expect(globalStateMocks.initializeWorkspaceStartupState).not.toHaveBeenCalled();

    resolveListener?.(() => undefined);
    await initializePromise;

    expect(globalStateMocks.initializeWorkspaceStartupState).toHaveBeenCalledTimes(1);
    expect(globalStateMocks.cleanupInvalidWorkspaces).not.toHaveBeenCalled();
    expect(globalStateMocks.getCurrentWorkspace).not.toHaveBeenCalled();
    expect(globalStateMocks.getRecentWorkspaces).not.toHaveBeenCalled();
    expect(globalStateMocks.getOpenedWorkspaces).not.toHaveBeenCalled();
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

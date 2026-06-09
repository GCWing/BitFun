import { beforeEach, describe, expect, it, vi } from 'vitest';

const registryMock = vi.hoisted(() => ({
  initialize: vi.fn(),
}));

const workspaceInitializerMock = vi.hoisted(() => ({
  start: vi.fn(),
  initializeWorkspace: vi.fn(),
}));

vi.mock('./services/LspDiagnostics', () => ({}));
vi.mock('./services/LspExtensionRegistry', () => ({
  lspExtensionRegistry: registryMock,
}));
vi.mock('./services/WorkspaceLspInitializer', () => ({
  workspaceLspInitializer: workspaceInitializerMock,
}));

describe('initializeLsp', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    registryMock.initialize.mockResolvedValue(undefined);
    workspaceInitializerMock.initializeWorkspace.mockResolvedValue(undefined);
  });

  it('initializes only lightweight LSP configuration by default', async () => {
    const { initializeLsp } = await import('./initializeLsp');

    await initializeLsp();

    expect(registryMock.initialize).toHaveBeenCalledTimes(1);
    expect(workspaceInitializerMock.start).not.toHaveBeenCalled();
    expect(workspaceInitializerMock.initializeWorkspace).not.toHaveBeenCalled();
  });

  it('initializes a workspace only when LSP is needed', async () => {
    const { ensureWorkspaceLspInitialized } = await import('./initializeLsp');

    await ensureWorkspaceLspInitialized('D:/workspace/BitFun');

    expect(registryMock.initialize).toHaveBeenCalledTimes(1);
    expect(workspaceInitializerMock.start).not.toHaveBeenCalled();
    expect(workspaceInitializerMock.initializeWorkspace).toHaveBeenCalledWith(
      'D:/workspace/BitFun',
      { prestartServers: false }
    );
  });
});

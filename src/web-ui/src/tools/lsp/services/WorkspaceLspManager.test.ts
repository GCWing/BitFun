import { afterEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn(() => () => {}));

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: {
    invoke: invokeMock,
    listen: listenMock,
  },
}));

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    progress: vi.fn(),
    error: vi.fn(),
    loading: vi.fn(() => ({
      updateMessage: vi.fn(),
      complete: vi.fn()
    }))
  }
}));

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: vi.fn((key: string) => key)
  }
}));

import { WorkspaceLspManager } from './WorkspaceLspManager';
import {
  activateSurface,
  isSurfaceChangedError,
  resetDeviceSurfaceForTest,
} from '@/infrastructure/peer-device/deviceSurface';

function serverState(status: 'stopped' | 'starting' | 'running' | 'failed' | 'restarting') {
  return {
    status,
    language: 'rust',
    startedAt: null,
    lastError: null,
    restartCount: 0,
    documentCount: 0
  };
}

describe('WorkspaceLspManager', () => {
  afterEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
    listenMock.mockReturnValue(() => {});
    (WorkspaceLspManager as unknown as {
      instances: Map<string, WorkspaceLspManager>;
    }).instances.clear();
    resetDeviceSurfaceForTest();
  });

  it('skips didOpen when the language server is stopped', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'lsp_open_workspace') {
        return undefined;
      }
      if (command === 'lsp_get_server_state') {
        return serverState('stopped');
      }
      if (command === 'lsp_open_document') {
        throw new Error('didOpen should not be sent for a stopped server');
      }
      return undefined;
    });

    const manager = WorkspaceLspManager.getOrCreate('D:\\workspace\\BitFun');

    const result = await manager.openDocument(
      'file:///D:/workspace/BitFun/src/main.rs',
      'rust',
      'fn main() {}'
    );

    expect(result).toEqual({
      language: 'rust',
      opened: false,
      skippedReason: 'server-not-running',
      serverStatus: 'stopped'
    });
    expect(invokeMock).not.toHaveBeenCalledWith('lsp_open_document', expect.anything());
  });

  it('sends didOpen when the language server is running', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'lsp_open_workspace') {
        return undefined;
      }
      if (command === 'lsp_get_server_state') {
        return serverState('running');
      }
      if (command === 'lsp_open_document') {
        return undefined;
      }
      return undefined;
    });

    const manager = WorkspaceLspManager.getOrCreate('D:\\workspace\\BitFun');

    const result = await manager.openDocument(
      'file:///D:/workspace/BitFun/src/main.rs',
      'rust',
      'fn main() {}'
    );

    expect(result).toEqual({ language: 'rust', opened: true });
    expect(invokeMock).toHaveBeenCalledWith('lsp_open_document', {
      request: {
        workspacePath: 'D:\\workspace\\BitFun',
        uri: 'file:///D:/workspace/BitFun/src/main.rs',
        language: 'rust',
        content: 'fn main() {}'
      }
    });
  });

  it('caches stopped server state to avoid repeated state queries during UI remounts', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'lsp_open_workspace') {
        return undefined;
      }
      if (command === 'lsp_get_server_state') {
        return serverState('stopped');
      }
      if (command === 'lsp_open_document') {
        throw new Error('didOpen should not be sent for a stopped server');
      }
      return undefined;
    });

    const manager = WorkspaceLspManager.getOrCreate('D:\\workspace\\BitFun');

    await manager.openDocument('file:///D:/workspace/BitFun/src/main.rs', 'rust', 'fn main() {}');
    await manager.openDocument('file:///D:/workspace/BitFun/src/lib.rs', 'rust', 'pub fn lib() {}');

    expect(invokeMock).toHaveBeenCalledTimes(2);
    expect(invokeMock).toHaveBeenCalledWith('lsp_open_workspace', {
      request: { workspacePath: 'D:\\workspace\\BitFun' }
    });
    expect(invokeMock).toHaveBeenCalledWith('lsp_get_server_state', {
      request: {
        workspacePath: 'D:\\workspace\\BitFun',
        language: 'rust'
      }
    });
    expect(invokeMock).not.toHaveBeenCalledWith('lsp_open_document', expect.anything());
  });
});

describe('WorkspaceLspManager device surface scoping', () => {
  afterEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
    listenMock.mockReturnValue(() => {});
    (WorkspaceLspManager as unknown as {
      instances: Map<string, WorkspaceLspManager>;
    }).instances.clear();
    resetDeviceSurfaceForTest();
  });

  it('does not share a manager for the same workspace path across devices', () => {
    const localManager = WorkspaceLspManager.getOrCreate('/Users/dev/BitFun');

    activateSurface('peer-device-b');
    const peerManager = WorkspaceLspManager.getOrCreate('/Users/dev/BitFun');

    expect(peerManager).not.toBe(localManager);
    expect(peerManager.getSurfaceId()).toBe('peer-device-b');
    expect(WorkspaceLspManager.get('/Users/dev/BitFun')).toBe(peerManager);
  });

  it('detaches for a surface switch without sending anything to the device being left', async () => {
    const unlisten = vi.fn();
    listenMock.mockReturnValue(unlisten);
    invokeMock.mockResolvedValue(undefined);

    const manager = WorkspaceLspManager.getOrCreate('/Users/dev/BitFun');
    await manager.initialize();
    invokeMock.mockClear();

    WorkspaceLspManager.detachAllForSurfaceSwitch();

    expect(invokeMock).not.toHaveBeenCalled();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('refuses to re-open a detached workspace on the device switched to', async () => {
    invokeMock.mockResolvedValue(undefined);

    const manager = WorkspaceLspManager.getOrCreate('/Users/dev/BitFun');
    await manager.initialize();
    WorkspaceLspManager.detachAllForSurfaceSwitch();
    activateSurface('peer-device-b');
    invokeMock.mockClear();

    const error = await manager.initialize().then(() => null, (reason: unknown) => reason);

    expect(isSurfaceChangedError(error)).toBe(true);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('prevents every detached manager operation from reaching the next device', async () => {
    invokeMock.mockResolvedValue(undefined);

    const manager = WorkspaceLspManager.getOrCreate('/Users/dev/BitFun');
    await manager.initialize();
    WorkspaceLspManager.detachAllForSurfaceSwitch();
    activateSurface('peer-device-b');
    invokeMock.mockClear();

    await manager.changeDocument('file:///Users/dev/BitFun/src/main.rs', 'fn main() {}');

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('drops events that reach a manager after its surface was left', async () => {
    let emitLspEvent: ((payload: unknown) => void) | null = null;
    listenMock.mockImplementation((_event: string, callback: (payload: unknown) => void) => {
      emitLspEvent = callback;
      return () => {};
    });
    invokeMock.mockResolvedValue(undefined);

    const manager = WorkspaceLspManager.getOrCreate('/Users/dev/BitFun');
    await manager.initialize();

    const diagnostics = vi.fn();
    manager.onDiagnostics('file:///Users/dev/BitFun/src/main.rs', diagnostics);
    manager.detachForSurfaceSwitch();

    emitLspEvent?.({
      type: 'Diagnostics',
      data: {
        workspace_path: '/Users/dev/BitFun',
        uri: 'file:///Users/dev/BitFun/src/main.rs',
        diagnostics: [{ message: 'from the device we left' }],
      },
    });

    expect(diagnostics).not.toHaveBeenCalled();
  });
});

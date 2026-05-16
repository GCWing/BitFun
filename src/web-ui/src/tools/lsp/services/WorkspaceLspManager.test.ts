import { afterEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn(async () => () => {}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock
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
    listenMock.mockResolvedValue(() => {});
    (WorkspaceLspManager as unknown as {
      instances: Map<string, WorkspaceLspManager>;
    }).instances.clear();
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

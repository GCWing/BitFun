import { beforeEach, describe, expect, it, vi } from 'vitest';
import { workspaceAPI } from './WorkspaceAPI';

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn(() => vi.fn()));
const waitForListenerRegistrationsMock = vi.hoisted(() => vi.fn());
const streamCapabilityMock = vi.hoisted(() => ({ supported: false }));

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
    listen: listenMock,
    waitForListenerRegistrations: waitForListenerRegistrationsMock,
    getAdapter: () => ({
      supportsSearchStreamEvents: () => streamCapabilityMock.supported,
    }),
  },
}));

describe('WorkspaceAPI', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue('file content');
    listenMock.mockReset();
    listenMock.mockImplementation(() => vi.fn());
    waitForListenerRegistrationsMock.mockReset();
    waitForListenerRegistrationsMock.mockResolvedValue(undefined);
    streamCapabilityMock.supported = false;
  });

  it('reads text through the registered command with remote routing context', async () => {
    await workspaceAPI.readFileContent(
      '/workspace/src/new.ts',
      undefined,
      'remote-connection-1',
    );

    expect(invokeMock).toHaveBeenCalledWith('read_file_content', {
      request: {
        filePath: '/workspace/src/new.ts',
        encoding: undefined,
        remoteConnectionId: 'remote-connection-1',
      },
    });
  });

  it('writes text through the registered command with remote routing context', async () => {
    await workspaceAPI.writeFileContent(
      '/workspace',
      '/workspace/.openbitfun/plans/refactor.plan.md',
      '# Plan',
      'remote-connection-1',
    );

    expect(invokeMock).toHaveBeenCalledWith('write_file_content', {
      request: {
        workspacePath: '/workspace',
        filePath: '/workspace/.openbitfun/plans/refactor.plan.md',
        content: '# Plan',
        remoteConnectionId: 'remote-connection-1',
      },
    });
  });

  it('browses a directory through the explicit remote workspace scope', async () => {
    invokeMock.mockResolvedValueOnce([]);

    await workspaceAPI.getDirectoryChildren('/workspace', 'remote-connection-1');

    expect(invokeMock).toHaveBeenCalledWith('get_directory_children', {
      request: {
        path: '/workspace',
        remoteConnectionId: 'remote-connection-1',
      },
    });
  });

  it('searches filenames through the same explicit remote workspace scope', async () => {
    invokeMock.mockResolvedValueOnce({ results: [], limit: 30, truncated: false });

    await workspaceAPI.searchFilenamesOnlyDetailed(
      '/workspace',
      '手写',
      false,
      false,
      false,
      undefined,
      30,
      true,
      undefined,
      'remote-connection-1',
    );

    expect(invokeMock).toHaveBeenCalledWith('search_filenames', {
      request: expect.objectContaining({
        rootPath: '/workspace',
        pattern: '手写',
        maxResults: 30,
        includeDirectories: true,
        remoteConnectionId: 'remote-connection-1',
      }),
    });
  });

  it('uses response-based filename search when transport events are unavailable', async () => {
    invokeMock.mockResolvedValueOnce({
      results: [{
        path: '/workspace/手写笔画标注项目',
        name: '手写笔画标注项目',
        isDirectory: true,
        matchType: 'fileName',
      }],
      limit: 30,
      truncated: false,
    });
    const onProgress = vi.fn();

    await workspaceAPI.searchFilenamesOnlyStreamDetailed(
      '/workspace',
      '手写',
      false,
      false,
      false,
      undefined,
      30,
      true,
      { onProgress },
      undefined,
      'remote-connection-1',
    );

    expect(invokeMock).toHaveBeenCalledWith('search_filenames', {
      request: expect.objectContaining({
        remoteConnectionId: 'remote-connection-1',
      }),
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      'start_search_filenames_stream',
      expect.anything(),
    );
    expect(onProgress).toHaveBeenCalledWith(expect.objectContaining({
      results: [expect.objectContaining({ name: '手写笔画标注项目' })],
    }));
  });

  it('starts scoped streaming only after transport listeners are registered', async () => {
    streamCapabilityMock.supported = true;
    const listeners = new Map<string, (event: any) => void>();
    listenMock.mockImplementation((event: string, callback: (payload: any) => void) => {
      listeners.set(event, callback);
      return vi.fn();
    });
    invokeMock.mockImplementationOnce(async (command: string, args: {
      request: { searchId: string };
    }) => {
      expect(command).toBe('start_search_filenames_stream');
      listeners.get('file-search://progress')?.({
        searchId: args.request.searchId,
        searchKind: 'filenames',
        results: [],
      });
      listeners.get('file-search://complete')?.({
        searchId: args.request.searchId,
        searchKind: 'filenames',
        limit: 30,
        truncated: false,
        totalResults: 0,
      });
      return { searchId: args.request.searchId, limit: 30 };
    });

    await workspaceAPI.searchFilenamesOnlyStreamDetailed(
      '/workspace',
      '手写',
      false,
      false,
      false,
      undefined,
      30,
      true,
      {},
      undefined,
      'remote-connection-1',
    );

    expect(invokeMock).toHaveBeenCalledWith('start_search_filenames_stream', {
      request: expect.objectContaining({
        rootPath: '/workspace',
        remoteConnectionId: 'remote-connection-1',
      }),
    });
    expect(waitForListenerRegistrationsMock).toHaveBeenCalledOnce();
  });

  it('does not start an orphaned stream when aborted during listener registration', async () => {
    streamCapabilityMock.supported = true;
    let finishListenerRegistration: (() => void) | undefined;
    waitForListenerRegistrationsMock.mockReturnValueOnce(new Promise<void>(resolve => {
      finishListenerRegistration = resolve;
    }));
    const controller = new AbortController();

    const search = workspaceAPI.searchFilenamesOnlyStreamDetailed(
      '/workspace',
      '手写',
      false,
      false,
      false,
      undefined,
      30,
      true,
      {},
      controller.signal,
      'remote-connection-1',
    );
    await vi.waitFor(() => {
      expect(waitForListenerRegistrationsMock).toHaveBeenCalledOnce();
    });

    controller.abort();
    finishListenerRegistration?.();

    await expect(search).rejects.toMatchObject({ name: 'AbortError' });
    await Promise.resolve();
    expect(invokeMock.mock.calls.some(([command]) => (
      command === 'start_search_filenames_stream'
    ))).toBe(false);
  });

  it('resolves browser-dropped file paths through a structured host request', async () => {
    invokeMock.mockResolvedValueOnce(['C:\\drop\\report.pdf']);

    await expect(workspaceAPI.resolveBrowserDroppedFilePaths('drop-token', 1)).resolves.toEqual([
      'C:\\drop\\report.pdf',
    ]);
    expect(invokeMock).toHaveBeenCalledWith('resolve_browser_dropped_file_paths', {
      request: {
        token: 'drop-token',
        fileCount: 1,
      },
    });
  });
});

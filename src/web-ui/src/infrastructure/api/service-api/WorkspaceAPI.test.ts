import { beforeEach, describe, expect, it, vi } from 'vitest';
import { workspaceAPI } from './WorkspaceAPI';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
    listen: vi.fn(),
  },
}));

describe('WorkspaceAPI', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue('file content');
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
});

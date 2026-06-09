import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SessionAPI } from './SessionAPI';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
  },
}));

describe('SessionAPI paged metadata reads', () => {
  let sessionAPI: SessionAPI;

  beforeEach(() => {
    sessionAPI = new SessionAPI();
    invokeMock.mockReset();
  });

  it('requests a top-level session metadata page with cursor and remote identity', async () => {
    const page = {
      sessions: [],
      totalTopLevelCount: 12,
      loadedTopLevelCount: 5,
      nextCursor: '5',
      hasMore: true,
    };
    invokeMock.mockResolvedValueOnce(page);

    await expect(
      sessionAPI.listSessionsPage({
        workspacePath: '/repo',
        limit: 5,
        cursor: '0',
        remoteConnectionId: 'remote-1',
        remoteSshHost: 'host',
      })
    ).resolves.toBe(page);

    expect(invokeMock).toHaveBeenCalledWith('list_persisted_sessions_page', {
      request: {
        workspace_path: '/repo',
        limit: 5,
        cursor: '0',
        remote_connection_id: 'remote-1',
        remote_ssh_host: 'host',
      },
    });
  });
});

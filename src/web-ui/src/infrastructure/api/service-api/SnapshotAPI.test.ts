import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SnapshotAPI } from './SnapshotAPI';

const invokeMock = vi.hoisted(() => vi.fn());
const sessionsMock = vi.hoisted(() => new Map<string, any>());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
  },
}));

vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  flowChatStore: {
    getState: () => ({ sessions: sessionsMock }),
  },
}));

describe('SnapshotAPI request dedupe', () => {
  let snapshotAPI: SnapshotAPI;

  beforeEach(() => {
    snapshotAPI = new SnapshotAPI();
    invokeMock.mockReset();
    sessionsMock.clear();
  });

  it('deduplicates concurrent session stats requests for the same session and workspace', async () => {
    const stats = {
      session_id: 'session-1',
      total_files: 2,
      total_turns: 3,
      total_changes: 4,
    };
    invokeMock.mockResolvedValueOnce(stats);

    const first = snapshotAPI.getSessionStats('session-1', 'D:/workspace/BitFun');
    const second = snapshotAPI.getSessionStats('session-1', 'D:/workspace/BitFun');

    await expect(Promise.all([first, second])).resolves.toEqual([stats, stats]);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('get_session_stats', {
      request: {
        session_id: 'session-1',
        workspacePath: 'D:/workspace/BitFun',
      },
    });
  });

  it('allows a new session stats request after the in-flight request settles', async () => {
    invokeMock
      .mockResolvedValueOnce({
        session_id: 'session-1',
        total_files: 1,
        total_turns: 1,
        total_changes: 1,
      })
      .mockResolvedValueOnce({
        session_id: 'session-1',
        total_files: 2,
        total_turns: 2,
        total_changes: 2,
      });

    await snapshotAPI.getSessionStats('session-1', 'D:/workspace/BitFun');
    await snapshotAPI.getSessionStats('session-1', 'D:/workspace/BitFun');

    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it('preserves the session remote binding on snapshot mutations', async () => {
    sessionsMock.set('remote-session', {
      workspacePath: '/srv/project',
      remoteConnectionId: 'ssh:user@example.com:22',
      remoteSshHost: 'example.com',
      config: {},
    });
    invokeMock.mockResolvedValue(undefined);

    await snapshotAPI.rejectFileModifications(
      'remote-session',
      'src/main.rs',
      '/srv/project',
    );

    expect(invokeMock).toHaveBeenCalledWith('reject_file', {
      request: {
        sessionId: 'remote-session',
        filePath: 'src/main.rs',
        workspacePath: '/srv/project',
        remoteConnectionId: 'ssh:user@example.com:22',
        remoteSshHost: 'example.com',
      },
    });
  });

  it('preserves the session remote binding on snapshot reads after disconnect', async () => {
    sessionsMock.set('remote-session', {
      workspacePath: 'D:/workspace/project',
      remoteConnectionId: 'ssh:user@example.com:22',
      remoteSshHost: 'example.com',
      config: {},
    });
    invokeMock.mockResolvedValue({
      session_id: 'remote-session',
      total_files: 0,
      total_turns: 0,
      total_changes: 0,
    });

    await snapshotAPI.getSessionStats('remote-session', 'D:/workspace/project');

    expect(invokeMock).toHaveBeenCalledWith('get_session_stats', {
      request: {
        session_id: 'remote-session',
        workspacePath: 'D:/workspace/project',
        remoteConnectionId: 'ssh:user@example.com:22',
        remoteSshHost: 'example.com',
      },
    });
  });

  it('does not treat a persisted localhost hostname as a remote session binding', async () => {
    sessionsMock.set('local-history-session', {
      workspacePath: 'D:/workspace/project',
      remoteSshHost: 'localhost',
      config: {},
    });
    invokeMock.mockResolvedValue({
      session_id: 'local-history-session',
      total_files: 0,
      total_turns: 0,
      total_changes: 0,
    });

    await snapshotAPI.getSessionStats('local-history-session', 'D:/workspace/project');

    expect(invokeMock).toHaveBeenCalledWith('get_session_stats', {
      request: {
        session_id: 'local-history-session',
        workspacePath: 'D:/workspace/project',
      },
    });
  });
});

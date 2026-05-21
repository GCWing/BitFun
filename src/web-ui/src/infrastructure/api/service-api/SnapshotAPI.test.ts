import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SnapshotAPI } from './SnapshotAPI';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
  },
}));

describe('SnapshotAPI request dedupe', () => {
  let snapshotAPI: SnapshotAPI;

  beforeEach(() => {
    snapshotAPI = new SnapshotAPI();
    invokeMock.mockReset();
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
});

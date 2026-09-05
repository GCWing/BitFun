import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SnapshotAPI } from './SnapshotAPI';
import { activateSurface } from '@/infrastructure/peer-device/deviceSurface';

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
    activateSurface('local');
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

    const first = snapshotAPI.getSessionStats('session-1', 'D:/workspace/OpenBitFun');
    const second = snapshotAPI.getSessionStats('session-1', 'D:/workspace/OpenBitFun');

    await expect(Promise.all([first, second])).resolves.toEqual([stats, stats]);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('get_session_stats', {
      request: {
        session_id: 'session-1',
        workspacePath: 'D:/workspace/OpenBitFun',
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

    await snapshotAPI.getSessionStats('session-1', 'D:/workspace/OpenBitFun');
    await snapshotAPI.getSessionStats('session-1', 'D:/workspace/OpenBitFun');

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

  it('never reuses a pending snapshot response across device surface activations', async () => {
    let resolveFirst!: (value: unknown) => void;
    invokeMock.mockImplementationOnce(() => new Promise(resolve => { resolveFirst = resolve; }));
    invokeMock.mockResolvedValueOnce({ operationId: 'operation-1', linesAdded: 2 });
    const first = snapshotAPI.getOperationSummary('same-session', 'operation-1', '/same/path');
    activateSurface('peer-b');
    const second = snapshotAPI.getOperationSummary('same-session', 'operation-1', '/same/path');
    expect(invokeMock).toHaveBeenCalledTimes(2);
    await expect(second).resolves.toMatchObject({ linesAdded: 2 });
    resolveFirst({ operationId: 'operation-1', linesAdded: 1 });
    await expect(first).resolves.toMatchObject({ linesAdded: 1 });
    activateSurface('local');
  });

  it('keeps host-only legacy remote identity instead of querying colliding local history', async () => {
    sessionsMock.set('legacy-remote', {
      workspacePath: '/srv/shared',
      config: { remoteSshHost: 'legacy.example' },
    });
    invokeMock.mockResolvedValue({});
    await snapshotAPI.getOperationSummary('legacy-remote', 'operation-1');
    expect(invokeMock).toHaveBeenCalledWith('get_operation_summary', {
      request: {
        sessionId: 'legacy-remote', operationId: 'operation-1',
        workspacePath: '/srv/shared', remoteSshHost: 'legacy.example',
      },
    });
  });

  it('scopes persisted operation diff and summary to the Session connection', async () => {
    sessionsMock.set('remote-operation', {
      workspacePath: '/srv/shared',
      remoteConnectionId: 'ssh:user-a@host:22', remoteSshHost: 'host', config: {},
    });
    invokeMock.mockResolvedValue({});
    await snapshotAPI.getOperationDiff('remote-operation', '/srv/shared/file.txt', 'operation-1');
    await snapshotAPI.getOperationSummary('remote-operation', 'operation-1');
    for (const [command, args] of invokeMock.mock.calls) {
      expect(['get_operation_diff', 'get_operation_summary']).toContain(command);
      expect(args.request).toMatchObject({
        workspacePath: '/srv/shared', remoteConnectionId: 'ssh:user-a@host:22', remoteSshHost: 'host',
        sessionId: 'remote-operation', operationId: 'operation-1',
      });
    }
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

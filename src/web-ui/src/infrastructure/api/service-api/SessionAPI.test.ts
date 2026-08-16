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

  it('omits a local workspace host sentinel without a remote connection id', async () => {
    const page = {
      sessions: [],
      totalTopLevelCount: 0,
      loadedTopLevelCount: 0,
      hasMore: false,
    };
    invokeMock.mockResolvedValueOnce(page);

    await sessionAPI.listSessionsPage({
      workspacePath: 'D:/repo',
      limit: 5,
      remoteSshHost: 'localhost',
    });

    expect(invokeMock).toHaveBeenCalledWith('list_persisted_sessions_page', {
      request: {
        workspace_path: 'D:/repo',
        limit: 5,
      },
    });
  });

  it('preserves localhost when a remote connection id disambiguates the scope', async () => {
    const page = {
      sessions: [],
      totalTopLevelCount: 0,
      loadedTopLevelCount: 0,
      hasMore: false,
    };
    invokeMock.mockResolvedValueOnce(page);

    await sessionAPI.listSessionsPage({
      workspacePath: '/srv/repo',
      limit: 5,
      remoteConnectionId: 'connection-1',
      remoteSshHost: 'localhost',
    });

    expect(invokeMock).toHaveBeenCalledWith('list_persisted_sessions_page', {
      request: {
        workspace_path: '/srv/repo',
        limit: 5,
        remote_connection_id: 'connection-1',
        remote_ssh_host: 'localhost',
      },
    });
  });

  it('preserves a legacy non-local host without a remote connection id', async () => {
    const page = {
      sessions: [],
      totalTopLevelCount: 0,
      loadedTopLevelCount: 0,
      hasMore: false,
    };
    invokeMock.mockResolvedValueOnce(page);

    await sessionAPI.listSessionsPage({
      workspacePath: '/srv/repo',
      limit: 5,
      remoteSshHost: 'legacy.example',
    });

    expect(invokeMock).toHaveBeenCalledWith('list_persisted_sessions_page', {
      request: {
        workspace_path: '/srv/repo',
        limit: 5,
        remote_ssh_host: 'legacy.example',
      },
    });
  });

  it('loads the scoped hidden Session lineage without listing all internal Sessions', async () => {
    const snapshot = { rootSessionId: 'root', sessions: [] };
    invokeMock.mockResolvedValueOnce(snapshot);

    await expect(sessionAPI.getSessionLineage({
      sessionId: 'child',
      workspacePath: '/repo',
      remoteConnectionId: 'remote-1',
      remoteSshHost: 'host',
    })).resolves.toBe(snapshot);

    expect(invokeMock).toHaveBeenCalledWith('get_session_lineage', {
      request: {
        session_id: 'child',
        workspace_path: '/repo',
        remote_connection_id: 'remote-1',
        remote_ssh_host: 'host',
      },
    });
  });

  it('requests usage reports with explicit hidden subagent scope', async () => {
    const report = {
      reportId: 'usage-report-1',
      schemaVersion: 1,
      generatedAt: 1_778_347_200_000,
      sessionId: 'session-1',
      workspace: { kind: 'local' },
      scope: { kind: 'full_session', turnCount: 0 },
      coverage: { level: 'complete', available: [], missing: [], notes: [] },
      time: { accounting: 'unavailable', denominator: 'session_wall_time' },
      tokens: { source: 'unavailable', cacheCoverage: 'unavailable' },
      models: [],
      tools: [],
      files: { scope: 'unavailable', files: [] },
      compression: { compactionCount: 0, manualCompactionCount: 0, automaticCompactionCount: 0 },
      errors: { totalErrors: 0, toolErrors: 0, modelErrors: 0, examples: [] },
      slowest: [],
      privacy: {
        promptContentIncluded: false,
        toolInputsIncluded: false,
        commandOutputsIncluded: false,
        fileContentsIncluded: false,
        redactedFields: [],
      },
    };
    invokeMock.mockResolvedValueOnce(report);

    await expect(
      sessionAPI.getSessionUsageReport({
        sessionId: 'session-1',
        workspacePath: '/repo',
        includeHiddenSubagents: false,
      })
    ).resolves.toBe(report);

    expect(invokeMock).toHaveBeenCalledWith('get_session_usage_report', {
      request: {
        session_id: 'session-1',
        workspace_path: '/repo',
        include_hidden_subagents: false,
      },
    });
  });

  it('redacts workspace paths and search text from command-error diagnostics', async () => {
    invokeMock.mockRejectedValueOnce(new Error('search unavailable'));

    const error = await sessionAPI.searchSessionContent({
      workspacePath: '/private/customer/repository',
      remoteConnectionId: 'remote-1',
      query: 'confidential roadmap',
      includeArchived: true,
    }).catch((caught: unknown) => caught);

    expect(error).toMatchObject({
      context: {
        request: {
          queryLength: 20,
          remote: true,
          includeArchived: true,
        },
      },
    });
    expect(error.getFormattedMessage()).not.toContain('/private/customer/repository');
    expect(error.getFormattedMessage()).not.toContain('confidential roadmap');
  });

  it('preserves AbortError values without wrapping them as command failures', async () => {
    const abortError = Object.assign(new Error('cancelled'), { name: 'AbortError' });
    invokeMock.mockRejectedValueOnce(abortError);

    await expect(sessionAPI.searchSessionContent({
      workspacePath: '/repo',
      query: 'cancelled search',
    })).rejects.toBe(abortError);
  });
});

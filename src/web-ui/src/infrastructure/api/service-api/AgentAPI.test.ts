import { beforeEach, describe, expect, it, vi } from 'vitest';
import { agentAPI } from './AgentAPI';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
    listen: vi.fn(),
  },
}));

describe('AgentAPI', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it('sends subagent timeout controls with the desktop command request shape', async () => {
    await agentAPI.setSubagentTimeout('subagent-session', { type: 'disable' });

    expect(invokeMock).toHaveBeenCalledWith('set_subagent_timeout', {
      request: {
        sessionId: 'subagent-session',
        action: { type: 'Disable', payload: null },
      },
    });
  });

  it('reloads one closed session context target through a structured request', async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await expect(agentAPI.reloadSessionContext({
      sessionId: 'session-1',
      target: 'instructions',
    })).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith('reload_session_context', {
      request: {
        sessionId: 'session-1',
        target: 'instructions',
      },
    });
  });

  it('returns whether session cancellation was accepted for an active turn', async () => {
    invokeMock.mockResolvedValueOnce({
      cancelled: true,
      dialogTurnId: 'turn-1',
    });

    await expect(agentAPI.cancelSession('subagent-session')).resolves.toEqual({
      cancelled: true,
      dialogTurnId: 'turn-1',
    });
    expect(invokeMock).toHaveBeenCalledWith('cancel_session', {
      request: { sessionId: 'subagent-session' },
    });
  });

  it('preserves a no-active-turn cancellation response', async () => {
    invokeMock.mockResolvedValueOnce({
      cancelled: false,
      dialogTurnId: null,
    });

    await expect(agentAPI.cancelSession('idle-session')).resolves.toEqual({
      cancelled: false,
      dialogTurnId: null,
    });
  });

  it('sends subagent timeout extensions with seconds in the action payload', async () => {
    await agentAPI.setSubagentTimeout('subagent-session', { type: 'extend', seconds: 300 });

    expect(invokeMock).toHaveBeenCalledWith('set_subagent_timeout', {
      request: {
        sessionId: 'subagent-session',
        action: { type: 'Extend', payload: { seconds: 300 } },
      },
    });
  });

  it('responds to permission requests by request id', async () => {
    await agentAPI.respondPermission('permission-1', 'reject', 'Use a read-only path');

    expect(invokeMock).toHaveBeenCalledWith('respond_permission', {
      request: {
        requestId: 'permission-1',
        reply: 'reject',
        feedback: 'Use a read-only path',
      },
    });
  });

  it('responds to the current and following permission requests atomically', async () => {
    invokeMock.mockResolvedValue(['permission-1', 'permission-2']);

    await expect(
      agentAPI.respondPermissionBatch('permission-1', 'always'),
    ).resolves.toEqual(['permission-1', 'permission-2']);

    expect(invokeMock).toHaveBeenCalledWith('respond_permission_batch', {
      request: {
        requestId: 'permission-1',
        reply: 'always',
      },
    });
  });

  it('preserves structured worktree errors during atomic session creation', async () => {
    invokeMock.mockRejectedValueOnce(JSON.stringify({
      code: 'copy_conflict',
      message: 'A selected local file already exists in the target worktree',
      recoveryPath: '/tmp/recover-worktree',
    }));

    await expect(agentAPI.createSession({
      sessionName: 'Isolated task',
      agentType: 'agentic',
      workspacePath: '/repo',
      projectWorkspacePath: '/repo',
      requestId: 'request-worktree-1',
      executionTarget: {
        kind: 'newManagedWorktree',
        baseRef: 'HEAD',
        copyLocalChanges: true,
      },
    })).rejects.toMatchObject({
      name: 'WorktreeCommandError',
      code: 'copy_conflict',
      recoveryPath: '/tmp/recover-worktree',
    });
  });

});

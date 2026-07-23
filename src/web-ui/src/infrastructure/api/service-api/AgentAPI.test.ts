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

});

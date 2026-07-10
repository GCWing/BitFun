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

  it('requests the platform-neutral Review quality decision with raw target facts', async () => {
    await agentAPI.decideReviewQuality({
      intent: 'review',
      target: {
        resolution: 'resolved',
        fileCount: 2,
        totalLinesChanged: 40,
        securitySensitiveFileCount: 1,
        workspaceAreaCount: 1,
        contractSurfaceChanged: false,
      },
    });

    expect(invokeMock).toHaveBeenCalledWith('decide_review_quality', {
      request: {
        intent: 'review',
        target: {
          resolution: 'resolved',
          fileCount: 2,
          totalLinesChanged: 40,
          securitySensitiveFileCount: 1,
          workspaceAreaCount: 1,
          contractSurfaceChanged: false,
        },
      },
    });
  });
});

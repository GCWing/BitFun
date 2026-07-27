import { beforeEach, describe, expect, it, vi } from 'vitest';
import { WorktreeAPI, WorktreeCommandError } from './WorktreeAPI';

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
    listen: listenMock,
  },
}));

describe('WorktreeAPI', () => {
  let api: WorktreeAPI;

  beforeEach(() => {
    api = new WorktreeAPI();
    invokeMock.mockReset();
    listenMock.mockReset();
  });

  it('uses project-scoped commands and never enables force by default', async () => {
    invokeMock.mockResolvedValue({ worktreeId: 'wt-1', removed: true });

    await api.remove('/repo', 'wt-1', 'request-1');

    expect(invokeMock).toHaveBeenCalledWith('worktree_remove', {
      request: {
        projectWorkspacePath: '/repo',
        worktreeId: 'wt-1',
        requestId: 'request-1',
        force: false,
      },
    });
  });

  it('preserves stable structured error codes', async () => {
    const transportError = Object.assign(new Error('command failed'), {
      data: {
        code: 'dirty_worktree',
        message: 'The worktree contains local changes',
      },
    });
    invokeMock.mockRejectedValue(transportError);

    await expect(api.remove('/repo', 'wt-1', 'request-2')).rejects.toMatchObject({
      name: 'WorktreeCommandError',
      code: 'dirty_worktree',
      message: 'The worktree contains local changes',
    } satisfies Partial<WorktreeCommandError>);
  });

  it('subscribes to event-driven worktree updates', () => {
    const unsubscribe = vi.fn();
    const callback = vi.fn();
    listenMock.mockReturnValue(unsubscribe);

    expect(api.onChanged(callback)).toBe(unsubscribe);
    expect(listenMock).toHaveBeenCalledWith('worktree://changed', callback);
  });
});

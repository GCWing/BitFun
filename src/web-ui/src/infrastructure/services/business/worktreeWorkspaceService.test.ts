import { beforeEach, describe, expect, it, vi } from 'vitest';

const addWorktree = vi.hoisted(() => vi.fn());

vi.mock('@/infrastructure/api/service-api/GitAPI', () => ({
  gitAPI: { addWorktree },
}));

import {
  createWorktreeWorkspace,
  WorktreeWorkspaceCreationError,
} from './worktreeWorkspaceService';

describe('createWorktreeWorkspace', () => {
  beforeEach(() => {
    addWorktree.mockReset();
  });

  it('reports Git creation failures as create-stage errors', async () => {
    addWorktree.mockRejectedValue(new Error('initial commit required'));
    const openWorkspace = vi.fn();

    const error = await createWorktreeWorkspace({
      repositoryPath: 'C:/repo',
      branch: 'dev',
      isNew: true,
      openAfterCreate: true,
      openWorkspace,
    }).catch(error => error);

    expect(error).toBeInstanceOf(WorktreeWorkspaceCreationError);
    expect(error.stage).toBe('create');
    expect(error.message).toBe('initial commit required');
    expect(openWorkspace).not.toHaveBeenCalled();
  });

  it('reports workspace opening failures only after creation succeeds', async () => {
    addWorktree.mockResolvedValue({
      path: 'C:/repo/.worktrees/dev',
      branch: 'dev',
      head: '1'.repeat(40),
      isMain: false,
      isLocked: false,
      isPrunable: false,
    });
    const openWorkspace = vi.fn().mockRejectedValue(new Error('open failed'));

    const error = await createWorktreeWorkspace({
      repositoryPath: 'C:/repo',
      branch: 'dev',
      isNew: true,
      openAfterCreate: true,
      openWorkspace,
    }).catch(error => error);

    expect(error).toBeInstanceOf(WorktreeWorkspaceCreationError);
    expect(error.stage).toBe('open');
    expect(error.message).toBe('open failed');
    expect(openWorkspace).toHaveBeenCalledWith('C:/repo/.worktrees/dev');
  });
});

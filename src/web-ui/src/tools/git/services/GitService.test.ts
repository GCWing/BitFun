import { describe, expect, it, vi, beforeEach } from 'vitest';
import { gitService } from './GitService';

const gitApiMocks = vi.hoisted(() => ({
  commit: vi.fn(),
  push: vi.fn(),
  resetFiles: vi.fn(),
}));

const gitStateManagerMock = vi.hoisted(() => ({
  refresh: vi.fn(),
}));

vi.mock('@/infrastructure/api', () => ({
  gitAPI: gitApiMocks,
}));

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: (key: string) => key,
  },
}));

vi.mock('../state/GitStateManager', () => ({
  gitStateManager: gitStateManagerMock,
}));

const repositoryPath = 'D:/workspace/BitFun';

describe('GitService dangerous operation refresh guard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    gitStateManagerMock.refresh.mockResolvedValue(undefined);
    gitApiMocks.commit.mockResolvedValue({ success: true });
    gitApiMocks.push.mockResolvedValue({ success: true });
    gitApiMocks.resetFiles.mockResolvedValue({ success: true });
  });

  it('forces a fresh basic/status refresh before committing', async () => {
    const order: string[] = [];
    gitStateManagerMock.refresh.mockImplementation(async () => {
      order.push('refresh');
    });
    gitApiMocks.commit.mockImplementation(async () => {
      order.push('commit');
      return { success: true };
    });

    await gitService.commit(repositoryPath, { message: 'test' });

    expect(order).toEqual(['refresh', 'commit']);
    expect(gitStateManagerMock.refresh).toHaveBeenCalledWith(repositoryPath, {
      force: true,
      layers: ['basic', 'status'],
      reason: 'operation',
      silent: true,
    });
  });

  it('forces a fresh basic/status refresh before push and reset operations', async () => {
    await gitService.push(repositoryPath);
    await gitService.resetFiles(repositoryPath, ['src/app.ts'], false);

    expect(gitStateManagerMock.refresh).toHaveBeenCalledTimes(2);
    expect(gitApiMocks.push).toHaveBeenCalledTimes(1);
    expect(gitApiMocks.resetFiles).toHaveBeenCalledTimes(1);
  });
});

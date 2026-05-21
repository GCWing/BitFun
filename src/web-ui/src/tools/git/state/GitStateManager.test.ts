import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GitStateManager } from './GitStateManager';

const gitApiMocks = vi.hoisted(() => ({
  isGitRepository: vi.fn(),
  getRepositoryBasic: vi.fn(),
  getRepository: vi.fn(),
  getStatus: vi.fn(),
  getBranches: vi.fn(),
  getCommits: vi.fn(),
}));

const gitEventServiceMock = vi.hoisted(() => ({
  on: vi.fn(),
  emit: vi.fn(),
}));

vi.mock('@/infrastructure/api', () => ({
  gitAPI: gitApiMocks,
}));

vi.mock('../services/GitEventService', () => ({
  gitEventService: gitEventServiceMock,
}));

vi.mock('@/infrastructure/event-bus', () => ({
  globalEventBus: {
    emit: vi.fn(),
  },
}));

vi.mock('@/shared/utils/debugProbe', () => ({
  sendDebugProbe: vi.fn(),
}));

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: (key: string) => key,
  },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const repositoryPath = 'D:/workspace/BitFun';

describe('GitStateManager refresh performance guards', () => {
  let manager: GitStateManager;

  beforeEach(() => {
    vi.useFakeTimers();
    GitStateManager.resetInstance();
    manager = GitStateManager.getInstance();
    manager.setCacheConfig({ basic: 0, status: 0, detailed: 0 });

    gitApiMocks.isGitRepository.mockResolvedValue(true);
    gitApiMocks.getRepositoryBasic.mockResolvedValue({
      path: repositoryPath,
      name: 'BitFun',
      current_branch: 'main',
      is_bare: false,
      has_changes: false,
      remotes: [],
    });
    gitApiMocks.getRepository.mockResolvedValue({
      path: repositoryPath,
      name: 'BitFun',
      current_branch: 'main',
      is_bare: false,
      has_changes: true,
      remotes: ['origin'],
    });
    gitApiMocks.getStatus.mockResolvedValue({
      staged: [],
      unstaged: [],
      untracked: [],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });
  });

  afterEach(() => {
    manager.dispose();
    GitStateManager.resetInstance();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('refreshes the basic layer without fetching full status', async () => {
    const refresh = manager.refresh(repositoryPath, {
      layers: ['basic'],
      force: true,
      reason: 'mount',
    });

    await vi.advanceTimersByTimeAsync(100);
    await refresh;

    expect(gitApiMocks.isGitRepository).toHaveBeenCalledTimes(1);
    expect(gitApiMocks.getRepositoryBasic).toHaveBeenCalledTimes(1);
    expect(gitApiMocks.getRepository).not.toHaveBeenCalled();
    expect(gitApiMocks.getStatus).not.toHaveBeenCalled();
    expect(manager.getState(repositoryPath)).toMatchObject({
      isRepository: true,
      currentBranch: 'main',
      hasChanges: false,
    });
  });

  it('merges duplicate mount refreshes for the same repository and layer', async () => {
    const first = manager.refresh(repositoryPath, {
      layers: ['basic', 'status'],
      reason: 'mount',
    });
    const second = manager.refresh(repositoryPath, {
      layers: ['basic', 'status'],
      reason: 'mount',
    });

    await vi.advanceTimersByTimeAsync(100);
    await Promise.all([first, second]);

    expect(gitApiMocks.getStatus).toHaveBeenCalledTimes(1);
  });

  it('does not run force refresh concurrently with an in-flight refresh', async () => {
    const firstStatus = deferred<Awaited<ReturnType<typeof gitApiMocks.getStatus>>>();
    gitApiMocks.getStatus
      .mockReturnValueOnce(firstStatus.promise)
      .mockResolvedValueOnce({
        staged: [],
        unstaged: [{ path: 'changed.ts', status: 'modified' }],
        untracked: [],
        conflicts: [],
        current_branch: 'main',
        ahead: 0,
        behind: 0,
      });

    const first = manager.refresh(repositoryPath, {
      layers: ['basic', 'status'],
      force: true,
      reason: 'mount',
    });
    await vi.advanceTimersByTimeAsync(100);
    await Promise.resolve();
    expect(gitApiMocks.getStatus).toHaveBeenCalledTimes(1);

    const forced = manager.refresh(repositoryPath, {
      layers: ['basic', 'status'],
      force: true,
      reason: 'operation',
    });
    await vi.advanceTimersByTimeAsync(100);
    await Promise.resolve();

    expect(gitApiMocks.getStatus).toHaveBeenCalledTimes(1);

    firstStatus.resolve({
      staged: [],
      unstaged: [],
      untracked: [],
      conflicts: [],
      current_branch: 'main',
      ahead: 0,
      behind: 0,
    });

    await first;
    await forced;
    expect(gitApiMocks.getStatus).toHaveBeenCalledTimes(2);
  });

  it('propagates an in-flight refresh failure to non-force joiners', async () => {
    const firstStatus = deferred<Awaited<ReturnType<typeof gitApiMocks.getStatus>>>();
    const failure = new Error('status failed');
    gitApiMocks.getStatus.mockReturnValueOnce(firstStatus.promise);

    const first = manager.refresh(repositoryPath, {
      layers: ['basic', 'status'],
      force: true,
      reason: 'mount',
    });
    await vi.advanceTimersByTimeAsync(100);
    await Promise.resolve();

    const second = manager.refresh(repositoryPath, {
      layers: ['basic', 'status'],
      reason: 'mount',
    });
    await vi.advanceTimersByTimeAsync(100);
    await Promise.resolve();

    firstStatus.reject(failure);

    await expect(first).rejects.toThrow('status failed');
    await expect(second).rejects.toThrow('status failed');
    expect(gitApiMocks.getStatus).toHaveBeenCalledTimes(1);
  });
});

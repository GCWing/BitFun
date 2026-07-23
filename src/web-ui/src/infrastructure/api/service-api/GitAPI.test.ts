import { beforeEach, describe, expect, it, vi } from 'vitest';
import { GitAPI } from './GitAPI';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('./ApiClient', () => ({
  api: {
    invoke: invokeMock,
  },
}));

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('GitAPI repository probe cache', () => {
  let gitAPI: GitAPI;

  beforeEach(() => {
    gitAPI = new GitAPI();
    invokeMock.mockReset();
  });

  it('deduplicates concurrent repository probes for the same path', async () => {
    const deferred = createDeferred<boolean>();
    invokeMock.mockReturnValueOnce(deferred.promise);

    const first = gitAPI.isGitRepository('D:/workspace/BitFun');
    const second = gitAPI.isGitRepository('D:/workspace/BitFun');

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('git_is_repository', {
      request: { repositoryPath: 'D:/workspace/BitFun' },
    });

    deferred.resolve(true);
    await expect(Promise.all([first, second])).resolves.toEqual([true, true]);
  });

  it('reuses a recent repository probe result for the same path', async () => {
    invokeMock.mockResolvedValueOnce(true);

    await expect(gitAPI.isGitRepository('D:/workspace/BitFun')).resolves.toBe(true);
    await expect(gitAPI.isGitRepository('D:/workspace/BitFun')).resolves.toBe(true);

    expect(invokeMock).toHaveBeenCalledTimes(1);
  });
});

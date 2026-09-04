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

    const first = gitAPI.isGitRepository('D:/workspace/OpenBitFun');
    const second = gitAPI.isGitRepository('D:/workspace/OpenBitFun');

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith('git_is_repository', {
      request: { repositoryPath: 'D:/workspace/OpenBitFun' },
    });

    deferred.resolve(true);
    await expect(Promise.all([first, second])).resolves.toEqual([true, true]);
  });

  it('reuses a recent repository probe result for the same path', async () => {
    invokeMock.mockResolvedValueOnce(true);

    await expect(gitAPI.isGitRepository('D:/workspace/OpenBitFun')).resolves.toBe(true);
    await expect(gitAPI.isGitRepository('D:/workspace/OpenBitFun')).resolves.toBe(true);

    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it('drops a cached probe result once ownership trust is granted', async () => {
    invokeMock.mockResolvedValueOnce(false);
    await expect(gitAPI.isGitRepository('D:/workspace/OpenBitFun')).resolves.toBe(false);

    invokeMock.mockResolvedValueOnce({
      state: 'trusted',
      repositoryPath: 'D:/workspace/OpenBitFun',
      alreadyTrusted: false,
      addedEntries: ['D:/workspace/OpenBitFun'],
      detail: null,
      manualCommand: null,
    });
    await gitAPI.trustRepository('D:/workspace/OpenBitFun');

    invokeMock.mockResolvedValueOnce(true);
    await expect(gitAPI.isGitRepository('D:/workspace/OpenBitFun')).resolves.toBe(true);
  });

  // One folder reaches this API under several spellings — a tree node hands in
  // `D:\workspace\OpenBitFun`, the backend hands back `D:/workspace/OpenBitFun`. The
  // backend treats them as one repository, so a cache keyed on the raw string
  // probes twice and, worse, leaves a stale entry a granted decision misses.
  it('treats the spellings of one repository as one cache entry', async () => {
    invokeMock.mockResolvedValueOnce(false);
    await expect(gitAPI.isGitRepository('D:/workspace/OpenBitFun')).resolves.toBe(false);
    await expect(gitAPI.isGitRepository('d:\\workspace\\openbitfun\\')).resolves.toBe(false);
    expect(invokeMock).toHaveBeenCalledTimes(1);

    invokeMock.mockResolvedValueOnce({
      state: 'trusted',
      repositoryPath: 'D:/workspace/OpenBitFun',
      alreadyTrusted: false,
      addedEntries: ['D:/workspace/OpenBitFun'],
      detail: null,
      manualCommand: null,
    });
    await gitAPI.trustRepository('d:\\workspace\\openbitfun');

    invokeMock.mockResolvedValueOnce(true);
    await expect(gitAPI.isGitRepository('D:/workspace/OpenBitFun')).resolves.toBe(true);
  });

  // The user runs the manual command in a terminal, or the repository's owner
  // fixes ownership. Nothing in this process granted anything, so the read-only
  // probe is the only place that learns — and the stale `false` it leaves behind
  // fails the very retry the recovery just decided was worth making.
  it('drops a cached probe result once the read-only probe reports trust', async () => {
    invokeMock.mockResolvedValueOnce(false);
    await expect(gitAPI.isGitRepository('D:/workspace/OpenBitFun')).resolves.toBe(false);

    invokeMock.mockResolvedValueOnce({
      state: 'trusted',
      repositoryPath: 'D:/workspace/OpenBitFun',
      detail: null,
      manualCommand: null,
    });
    await gitAPI.getRepositoryTrust('d:\\workspace\\openbitfun');

    invokeMock.mockResolvedValueOnce(true);
    await expect(gitAPI.isGitRepository('D:/workspace/OpenBitFun')).resolves.toBe(true);
  });

  it('keeps the cached probe result while the probe still reports the wall', async () => {
    invokeMock.mockResolvedValueOnce(false);
    await expect(gitAPI.isGitRepository('D:/workspace/OpenBitFun')).resolves.toBe(false);

    invokeMock.mockResolvedValueOnce({
      state: 'trust_required',
      repositoryPath: 'D:/workspace/OpenBitFun',
      detail: 'detected dubious ownership',
      manualCommand: "git config --global --add safe.directory 'D:/workspace/OpenBitFun'",
    });
    await gitAPI.getRepositoryTrust('D:/workspace/OpenBitFun');

    await expect(gitAPI.isGitRepository('D:/workspace/OpenBitFun')).resolves.toBe(false);
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it('keeps the cached probe result when trust was not granted', async () => {
    invokeMock.mockResolvedValueOnce(false);
    await expect(gitAPI.isGitRepository('D:/workspace/OpenBitFun')).resolves.toBe(false);

    invokeMock.mockResolvedValueOnce({
      state: 'trust_required',
      repositoryPath: 'D:/workspace/OpenBitFun',
      alreadyTrusted: false,
      addedEntries: [],
      detail: 'detected dubious ownership',
      manualCommand: 'git config --global --add safe.directory "D:/workspace/OpenBitFun"',
    });
    await gitAPI.trustRepository('D:/workspace/OpenBitFun');

    await expect(gitAPI.isGitRepository('D:/workspace/OpenBitFun')).resolves.toBe(false);
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });
});

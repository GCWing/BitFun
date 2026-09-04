import { afterEach, describe, expect, it, vi } from 'vitest';
import { resolveBrowserDroppedFilePaths } from './resolveBrowserDroppedFilePaths';

const REGISTRY_KEY = '__OPENBITFUN_BROWSER_DROP_FILES__';

type BrowserDropRegistryHost = typeof globalThis & {
  [REGISTRY_KEY]?: Map<string, readonly File[]>;
};

function registry(): Map<string, readonly File[]> | undefined {
  return (globalThis as BrowserDropRegistryHost)[REGISTRY_KEY];
}

describe('resolveBrowserDroppedFilePaths', () => {
  afterEach(() => {
    delete (globalThis as BrowserDropRegistryHost)[REGISTRY_KEY];
  });

  it('exposes the exact File wrappers while the host resolves their paths', async () => {
    const files = [
      new File(['alpha'], 'alpha.txt', { type: 'text/plain' }),
      new File(['beta'], 'beta.pdf', { type: 'application/pdf' }),
    ];
    const resolver = vi.fn(async (token: string, fileCount: number) => {
      expect(fileCount).toBe(2);
      expect(registry()?.get(token)).toBe(files);
      return ['C:\\drop\\alpha.txt', 'C:\\drop\\beta.pdf'];
    });

    await expect(resolveBrowserDroppedFilePaths(files, resolver)).resolves.toEqual([
      'C:\\drop\\alpha.txt',
      'C:\\drop\\beta.pdf',
    ]);
    expect(registry()).toBeUndefined();
  });

  it('removes the transient File wrappers when host resolution fails', async () => {
    const files = [new File(['alpha'], 'alpha.txt', { type: 'text/plain' })];

    await expect(resolveBrowserDroppedFilePaths(files, async () => {
      throw new Error('host unavailable');
    })).rejects.toThrow('host unavailable');
    expect(registry()).toBeUndefined();
  });

  it('does not invoke the host for an empty drop', async () => {
    const resolver = vi.fn();

    await expect(resolveBrowserDroppedFilePaths([], resolver)).resolves.toEqual([]);
    expect(resolver).not.toHaveBeenCalled();
  });
});

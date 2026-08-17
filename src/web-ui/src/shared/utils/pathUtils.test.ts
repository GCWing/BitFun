import { describe, expect, it } from 'vitest';
import { repositoryPathKey } from './pathUtils';

describe('repositoryPathKey', () => {
  it('collapses the spellings one Windows repository arrives under', () => {
    const expected = 'c:/work/repo';
    expect(repositoryPathKey('C:\\work\\repo')).toBe(expected);
    expect(repositoryPathKey('c:/work/repo')).toBe(expected);
    expect(repositoryPathKey('C:/work/repo/')).toBe(expected);
  });

  it('keys a UNC share the same way whichever separator it arrives with', () => {
    expect(repositoryPathKey('\\\\build01\\shared\\repo')).toBe('//build01/shared/repo');
    expect(repositoryPathKey('//Build01/Shared/repo/')).toBe('//build01/shared/repo');
  });

  // Backslash is an ordinary filename character on POSIX, so rewriting it would
  // collapse two different repositories onto one key — and this key gates the
  // trust prompt and the probe cache, so one dismissal would answer for both.
  it('keeps a POSIX backslash distinct from a separator', () => {
    expect(repositoryPathKey('/srv/we\\ird/repo')).toBe('/srv/we\\ird/repo');
    expect(repositoryPathKey('/srv/we\\ird/repo')).not.toBe(repositoryPathKey('/srv/we/ird/repo'));
  });

  it('leaves POSIX case alone and drops only a trailing separator', () => {
    expect(repositoryPathKey('/srv/Shared/Repo/')).toBe('/srv/Shared/Repo');
    expect(repositoryPathKey('/')).toBe('/');
  });
});

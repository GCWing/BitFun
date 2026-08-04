import { describe, expect, it } from 'vitest';
import { BITFUN_HOME_URL, BITFUN_RELEASES_URL } from './links';

describe('Skin Market external links', () => {
  it('uses the official BitFun website and GitHub release pages', () => {
    expect(BITFUN_HOME_URL).toBe('https://openbitfun.com/');
    expect(BITFUN_RELEASES_URL).toBe('https://github.com/GCWing/BitFun/releases');
    expect(new URL(BITFUN_HOME_URL).protocol).toBe('https:');
    expect(new URL(BITFUN_RELEASES_URL).protocol).toBe('https:');
  });
});

import { describe, expect, it } from 'vitest';
import { BITFUN_DOWNLOAD_URL, BITFUN_HOME_URL } from './links';

describe('MiniApp Market external links', () => {
  it('uses the official BitFun website and download pages', () => {
    expect(BITFUN_HOME_URL).toBe('https://openbitfun.com/');
    expect(BITFUN_DOWNLOAD_URL).toBe('https://openbitfun.com/download');
    expect(new URL(BITFUN_HOME_URL).protocol).toBe('https:');
    expect(new URL(BITFUN_DOWNLOAD_URL).protocol).toBe('https:');
  });
});

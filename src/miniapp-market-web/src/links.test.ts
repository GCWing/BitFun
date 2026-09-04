import { describe, expect, it } from 'vitest';
import { OPENBITFUN_DOWNLOAD_URL, OPENBITFUN_HOME_URL } from './links';

describe('MiniApp Market external links', () => {
  it('uses the official OpenBitFun website and download pages', () => {
    expect(OPENBITFUN_HOME_URL).toBe('https://openbitfun.com/');
    expect(OPENBITFUN_DOWNLOAD_URL).toBe('https://openbitfun.com/download');
    expect(new URL(OPENBITFUN_HOME_URL).protocol).toBe('https:');
    expect(new URL(OPENBITFUN_DOWNLOAD_URL).protocol).toBe('https:');
  });
});

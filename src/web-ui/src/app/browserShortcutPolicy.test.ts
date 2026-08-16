import { describe, expect, it } from 'vitest';

import { shouldBlockBrowserShortcut } from './browserShortcutPolicy';

describe('browser shortcut policy', () => {
  it('allows page reload shortcuts in development', () => {
    expect(shouldBlockBrowserShortcut('r', true)).toBe(false);
    expect(shouldBlockBrowserShortcut('R', true)).toBe(false);
  });

  it('blocks page reload shortcuts in release builds', () => {
    expect(shouldBlockBrowserShortcut('r', false)).toBe(true);
    expect(shouldBlockBrowserShortcut('R', false)).toBe(true);
  });

  it('continues to block browser find and ignores unrelated shortcuts', () => {
    expect(shouldBlockBrowserShortcut('f', true)).toBe(true);
    expect(shouldBlockBrowserShortcut('F', false)).toBe(true);
    expect(shouldBlockBrowserShortcut('p', false)).toBe(false);
  });
});

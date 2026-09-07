// @vitest-environment jsdom

import { describe, expect, it, vi } from 'vitest';

import { handleBrowserShortcut, shouldBlockBrowserShortcut } from './browserShortcutPolicy';

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
    expect(shouldBlockBrowserShortcut('s', false)).toBe(false);
  });

  it('blocks printing in development and release builds', () => {
    expect(shouldBlockBrowserShortcut('p', true)).toBe(true);
    expect(shouldBlockBrowserShortcut('P', false)).toBe(true);
  });
});

describe.each([
  ['Win32', { ctrlKey: true }],
  ['MacIntel', { metaKey: true }],
] as const)('browser shortcuts on %s', (platform, modifier) => {
  it.each([
    [{ key: '=', code: 'Equal' }, 1],
    [{ key: '+', code: 'Equal', shiftKey: true }, 1],
    [{ key: '+', code: 'NumpadAdd' }, 1],
    [{ key: '-', code: 'Minus' }, -1],
    [{ key: '_', code: 'Minus', shiftKey: true }, -1],
    [{ key: '-', code: 'NumpadSubtract' }, -1],
  ] as const)('changes typography for %j before focused controls receive it', (keys, delta) => {
    const adjust = vi.fn();
    const input = document.createElement('textarea');
    document.body.append(input);
    input.focus();
    const editorHandler = vi.fn();
    input.addEventListener('keydown', editorHandler);
    const capture = (event: KeyboardEvent) => handleBrowserShortcut(event, false, adjust, platform);
    window.addEventListener('keydown', capture, true);
    try {
      const event = new KeyboardEvent('keydown', { ...keys, ...modifier, bubbles: true, cancelable: true });
      input.dispatchEvent(event);
      expect(adjust).toHaveBeenCalledExactlyOnceWith(delta);
      expect(event.defaultPrevented).toBe(true);
      expect(editorHandler).not.toHaveBeenCalled();
    } finally {
      window.removeEventListener('keydown', capture, true);
      input.remove();
    }
  });

  it.each(['p', 'P'])('prevents printing for %s', key => {
    const adjust = vi.fn();
    const event = new KeyboardEvent('keydown', { key, ...modifier, cancelable: true });
    handleBrowserShortcut(event, true, adjust, platform);
    expect(event.defaultPrevented).toBe(true);
    expect(adjust).not.toHaveBeenCalled();
  });

  it.each([
    { key: '+' },
    { key: 'p' },
    { key: '+', ...modifier, altKey: true },
    { key: '+', ctrlKey: true, metaKey: true },
    { key: '+', ...modifier, isComposing: true },
    { key: '+', ...modifier, keyCode: 229 },
    { key: 's', ...modifier },
    { key: '+', ...(platform === 'MacIntel' ? { ctrlKey: true } : { metaKey: true }) },
  ])('leaves unrelated keys and IME input untouched: %j', keys => {
    const adjust = vi.fn();
    const event = new KeyboardEvent('keydown', { ...keys, cancelable: true });
    handleBrowserShortcut(event, false, adjust, platform);
    expect(event.defaultPrevented).toBe(false);
    expect(adjust).not.toHaveBeenCalled();
  });
});

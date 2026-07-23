// @vitest-environment jsdom

import { describe, expect, it, vi } from 'vitest';

import {
  restoreWindowKeyboardFocus,
  type FocusSnapshot,
  type WindowKeyboardFocusTarget,
} from './windowKeyboardFocus';

describe('restoreWindowKeyboardFocus', () => {
  it('restores native window focus and webview focus after a hidden fullscreen relayout', async () => {
    vi.useFakeTimers();

    const setWindowFocus = vi.fn().mockResolvedValue(undefined);
    const setWebviewFocus = vi.fn().mockResolvedValue(undefined);
    const windowFocus = vi.spyOn(window, 'focus').mockImplementation(() => {});
    const appRoot = document.createElement('div');
    appRoot.className = 'bitfun-app-layout';
    document.body.appendChild(appRoot);

    const focusTarget: WindowKeyboardFocusTarget = {
      setFocus: setWindowFocus,
      setWebviewFocus,
    };
    const focusSnapshot: FocusSnapshot = {
      activeElement: null,
      wasInputFocused: false,
    };

    restoreWindowKeyboardFocus(focusTarget, focusSnapshot, 80);

    await vi.advanceTimersByTimeAsync(80);
    await vi.waitFor(() => {
      expect(setWindowFocus).toHaveBeenCalledTimes(1);
      expect(setWebviewFocus).toHaveBeenCalledTimes(1);
    });

    expect(document.activeElement).toBe(appRoot);

    document.body.removeChild(appRoot);
    windowFocus.mockRestore();
    vi.useRealTimers();
  });
});

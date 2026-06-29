// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  getStartupOverlayElapsedMs,
  hideStartupOverlay,
  isStartupOverlayPresent,
} from './startupOverlay';

afterEach(() => {
  document.body.innerHTML = '';
  delete window.__BITFUN_STARTUP_OVERLAY_STARTED_AT__;
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe('startupOverlay', () => {
  it('removes the existing startup overlay when its exit animation completes', async () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="bitfun-startup-overlay"></div>';

    const hidden = hideStartupOverlay();

    const overlay = document.getElementById('bitfun-startup-overlay');
    expect(overlay?.classList.contains('bitfun-startup-overlay--exiting')).toBe(true);
    expect(overlay?.getAttribute('aria-hidden')).toBe('true');

    overlay?.dispatchEvent(new Event('animationend'));
    await hidden;

    expect(isStartupOverlayPresent()).toBe(false);
  });

  it('falls back to a timer when the exit animation event is not delivered', async () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="bitfun-startup-overlay"></div>';

    const hidden = hideStartupOverlay();

    await vi.advanceTimersByTimeAsync(349);
    expect(isStartupOverlayPresent()).toBe(true);

    await vi.advanceTimersByTimeAsync(1);
    await hidden;

    expect(isStartupOverlayPresent()).toBe(false);
  });

  it('tracks elapsed time from the static overlay first paint', () => {
    vi.spyOn(performance, 'now').mockReturnValue(1500);
    window.__BITFUN_STARTUP_OVERLAY_STARTED_AT__ = 1200;

    expect(getStartupOverlayElapsedMs()).toBe(300);
  });
});

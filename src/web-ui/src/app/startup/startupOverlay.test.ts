// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  getStartupOverlayElapsedMs,
  hideStartupOverlay,
  isStartupOverlayPresent,
} from './startupOverlay';

afterEach(() => {
  document.body.innerHTML = '';
  delete window.__OPENBITFUN_STARTUP_OVERLAY_STARTED_AT__;
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe('startupOverlay', () => {
  it('removes the existing startup overlay when its exit animation completes', async () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="openbitfun-startup-overlay"></div>';

    const hidden = hideStartupOverlay();

    const overlay = document.getElementById('openbitfun-startup-overlay');
    expect(overlay?.classList.contains('openbitfun-startup-overlay--exiting')).toBe(true);
    expect(overlay?.getAttribute('aria-hidden')).toBe('true');

    overlay?.dispatchEvent(new Event('animationend'));
    await hidden;

    expect(isStartupOverlayPresent()).toBe(false);
  });

  it('releases startup-control focus before hiding the exiting overlay', async () => {
    vi.useFakeTimers();
    document.body.innerHTML = `
      <div id="openbitfun-startup-overlay">
        <button type="button" data-startup-window-action="close">Close</button>
      </div>
    `;
    const closeButton = document.querySelector<HTMLButtonElement>(
      '[data-startup-window-action="close"]',
    );
    closeButton?.focus();
    expect(document.activeElement).toBe(closeButton);

    const hidden = hideStartupOverlay();
    const overlay = document.getElementById('openbitfun-startup-overlay');

    expect(document.activeElement).not.toBe(closeButton);
    expect(overlay?.hasAttribute('inert')).toBe(true);
    expect(overlay?.getAttribute('aria-hidden')).toBe('true');

    overlay?.dispatchEvent(new Event('animationend'));
    await hidden;
  });

  it('falls back to a timer when the exit animation event is not delivered', async () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="openbitfun-startup-overlay"></div>';

    const hidden = hideStartupOverlay();

    await vi.advanceTimersByTimeAsync(349);
    expect(isStartupOverlayPresent()).toBe(true);

    await vi.advanceTimersByTimeAsync(1);
    await hidden;

    expect(isStartupOverlayPresent()).toBe(false);
  });

  it('tracks elapsed time from the static overlay first paint', () => {
    vi.spyOn(performance, 'now').mockReturnValue(1500);
    window.__OPENBITFUN_STARTUP_OVERLAY_STARTED_AT__ = 1200;

    expect(getStartupOverlayElapsedMs()).toBe(300);
  });
});

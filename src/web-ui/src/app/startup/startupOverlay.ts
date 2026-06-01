const STARTUP_OVERLAY_ID = 'bitfun-startup-overlay';
const EXIT_CLASS = 'bitfun-startup-overlay--exiting';
const DEFAULT_EXIT_MS = 650;

declare global {
  interface Window {
    __BITFUN_STARTUP_OVERLAY_STARTED_AT__?: number;
  }
}

function getOverlay(): HTMLElement | null {
  return document.getElementById(STARTUP_OVERLAY_ID);
}

export function getStartupOverlayElapsedMs(): number {
  const startedAt = window.__BITFUN_STARTUP_OVERLAY_STARTED_AT__;
  if (typeof startedAt !== 'number') {
    return 0;
  }
  return Math.max(0, performance.now() - startedAt);
}

export function isStartupOverlayPresent(): boolean {
  return getOverlay() !== null;
}

export function hideStartupOverlay(): Promise<void> {
  const overlay = getOverlay();
  if (!overlay) {
    return Promise.resolve();
  }

  if (overlay.classList.contains(EXIT_CLASS)) {
    return new Promise(resolve => {
      window.setTimeout(resolve, DEFAULT_EXIT_MS);
    });
  }

  overlay.classList.add(EXIT_CLASS);
  overlay.setAttribute('aria-hidden', 'true');
  return new Promise(resolve => {
    window.setTimeout(() => {
      overlay.remove();
      resolve();
    }, DEFAULT_EXIT_MS);
  });
}

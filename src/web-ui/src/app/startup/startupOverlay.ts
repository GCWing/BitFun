const STARTUP_OVERLAY_ID = 'bitfun-startup-overlay';
const EXIT_CLASS = 'bitfun-startup-overlay--exiting';
const EXIT_FALLBACK_MS = 350;

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

function waitForOverlayExitAnimation(overlay: HTMLElement): Promise<void> {
  return new Promise(resolve => {
    let done = false;
    const state: { fallbackTimer: number | undefined } = {
      fallbackTimer: undefined,
    };

    function finish() {
      if (done) {
        return;
      }
      done = true;
      overlay.removeEventListener('animationend', handleAnimationEnd);
      if (state.fallbackTimer !== undefined) {
        window.clearTimeout(state.fallbackTimer);
      }
      resolve();
    }

    function handleAnimationEnd(event: AnimationEvent) {
      if (event.target !== overlay) {
        return;
      }
      finish();
    }

    overlay.addEventListener('animationend', handleAnimationEnd);
    state.fallbackTimer = window.setTimeout(finish, EXIT_FALLBACK_MS);
  });
}

export async function hideStartupOverlay(): Promise<void> {
  const overlay = getOverlay();
  if (!overlay) {
    return;
  }

  if (overlay.classList.contains(EXIT_CLASS)) {
    await waitForOverlayExitAnimation(overlay);
    return;
  }

  overlay.classList.add(EXIT_CLASS);
  overlay.setAttribute('aria-hidden', 'true');
  await waitForOverlayExitAnimation(overlay);
  overlay.remove();
}

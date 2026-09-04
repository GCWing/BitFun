const STARTUP_OVERLAY_ID = 'openbitfun-startup-overlay';
const EXIT_CLASS = 'openbitfun-startup-overlay--exiting';
const EXIT_FALLBACK_MS = 350;

declare global {
  interface Window {
    __OPENBITFUN_STARTUP_OVERLAY_STARTED_AT__?: number;
  }
}

function getOverlay(): HTMLElement | null {
  return document.getElementById(STARTUP_OVERLAY_ID);
}

export function getStartupOverlayElapsedMs(): number {
  const startedAt = window.__OPENBITFUN_STARTUP_OVERLAY_STARTED_AT__;
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
  // Static startup window controls can still own focus when the React shell
  // becomes ready. Release that focus before hiding the overlay from the
  // accessibility tree, then make the exiting surface non-interactive so it
  // cannot reclaim focus during its final animation.
  const activeElement = document.activeElement;
  if (activeElement instanceof HTMLElement && overlay.contains(activeElement)) {
    activeElement.blur();
  }
  overlay.setAttribute('inert', '');
  overlay.setAttribute('aria-hidden', 'true');
  await waitForOverlayExitAnimation(overlay);
  overlay.remove();
}

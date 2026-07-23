export type FocusSnapshot = {
  activeElement: HTMLElement | null;
  wasInputFocused: boolean;
};

export type WindowKeyboardFocusTarget = {
  setFocus?: () => Promise<void> | void;
  setWebviewFocus?: () => Promise<void> | void;
} | null;

export const captureFocusedEditable = (): FocusSnapshot => {
  const activeElement =
    document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const wasInputFocused = !!activeElement && (
    activeElement.classList.contains('rich-text-input') ||
    activeElement.closest('.rich-text-input') !== null ||
    activeElement.isContentEditable
  );

  return { activeElement, wasInputFocused };
};

const restoreFocusedEditable = (snapshot: FocusSnapshot) => {
  const chatInputs = document.querySelectorAll('.rich-text-input[contenteditable]');
  chatInputs.forEach((input) => {
    const element = input as HTMLElement;
    if (element.getAttribute('contenteditable') !== 'true') {
      element.setAttribute('contenteditable', 'true');
    }
  });

  if (snapshot.wasInputFocused && snapshot.activeElement?.isConnected) {
    try {
      const rect = snapshot.activeElement.getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        snapshot.activeElement.focus();
      }
    } catch (_error) {
      // Ignore focus restore failures.
    }
  }
};

const focusAppRootForKeyboardShortcuts = () => {
  const appRoot = document.querySelector('.bitfun-app-layout');
  if (!(appRoot instanceof HTMLElement)) {
    window.focus();
    return;
  }

  const previousTabIndex = appRoot.getAttribute('tabindex');
  appRoot.setAttribute('tabindex', '-1');
  appRoot.focus({ preventScroll: true });

  if (previousTabIndex === null) {
    appRoot.removeAttribute('tabindex');
  } else {
    appRoot.setAttribute('tabindex', previousTabIndex);
  }
};

export const restoreWindowKeyboardFocus = (
  focusTarget: WindowKeyboardFocusTarget,
  snapshot: FocusSnapshot,
  delayMs: number
) => {
  setTimeout(() => {
    void (async () => {
      try {
        await focusTarget?.setFocus?.();
      } catch {
        // Ignore focus failures during native window transitions.
      }

      try {
        // Hidden fullscreen relayouts show the native window before the WebView
        // reliably owns keyboard focus. The DOM F11 listener only receives the
        // next keydown after the WebView, not just the native window, is focused.
        await focusTarget?.setWebviewFocus?.();
      } catch {
        // Ignore focus failures during native webview transitions.
      }

      window.focus();

      if (snapshot.wasInputFocused) {
        restoreFocusedEditable(snapshot);
      } else {
        focusAppRootForKeyboardShortcuts();
      }
    })();
  }, delayMs);
};

interface KeyboardEventWithNativeSignals {
  isComposing?: boolean;
  keyCode?: number;
  nativeEvent?: {
    isComposing?: boolean;
    keyCode?: number;
  };
}

/**
 * Whether an IME still owns this keyboard event.
 *
 * `keyCode === 229` covers WebKit/older Chromium paths that do not reliably
 * expose `isComposing` on the React synthetic event.
 */
export function isImeOwnedKeyboardEvent(
  event: KeyboardEventWithNativeSignals,
  compositionActive = false,
): boolean {
  return compositionActive
    || event.isComposing === true
    || event.keyCode === 229
    || event.nativeEvent?.isComposing === true
    || event.nativeEvent?.keyCode === 229;
}

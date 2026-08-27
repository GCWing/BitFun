interface KeyboardEventWithNativeSignals {
  isComposing?: boolean;
  keyCode?: number;
  nativeEvent?: {
    isComposing?: boolean;
    keyCode?: number;
  };
}

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

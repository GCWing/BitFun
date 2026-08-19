/**
 * IME ownership detection for keyboard shortcuts on text inputs.
 *
 * Composition lifecycle tracking covers browsers where the native keyboard
 * event is incomplete. The native signals cover event-ordering races where
 * composition has ended locally but the IME still owns the key.
 */

import { useCallback, useRef } from 'react';

export interface ImeOwnedKeyGuard {
  isImeOwnedKey: (event: React.KeyboardEvent) => boolean;
  handleCompositionStart: () => void;
  handleCompositionEnd: () => void;
}

export function useImeOwnedKeyGuard(): ImeOwnedKeyGuard {
  const isImeComposingRef = useRef(false);

  const handleCompositionStart = useCallback(() => {
    isImeComposingRef.current = true;
  }, []);

  const handleCompositionEnd = useCallback(() => {
    isImeComposingRef.current = false;
  }, []);

  const isImeOwnedKey = useCallback((event: React.KeyboardEvent) => {
    const nativeEvent = event.nativeEvent as KeyboardEvent | undefined;
    return isImeComposingRef.current
      || nativeEvent?.isComposing === true
      || nativeEvent?.keyCode === 229;
  }, []);

  return { isImeOwnedKey, handleCompositionStart, handleCompositionEnd };
}

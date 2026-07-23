/**
 * useImeEnterGuard — IME-safe Enter detection for chat-style inputs.
 *
 * Problem: with Chinese / Japanese / Korean IMEs, the Enter key that
 * confirms a candidate must NOT be treated as "send message", but the
 * Enter key that actually submits the input MUST trigger send — and
 * fast typists may chain "confirm candidate → send" within a few
 * milliseconds.
 *
 * Strategy (no time-based heuristics):
 *   1. Track our own "is composing" flag via composition events. This
 *      handles browsers/IMEs where `KeyboardEvent.isComposing` is
 *      occasionally unreliable (notably some Safari / Linux paths).
 *   2. Treat any Enter `keydown` whose `keyCode === 229` as IME-owned.
 *      `keyCode 229` is the W3C-defined "composition keyCode" that
 *      every major browser still emits while an IME is processing the
 *      key, even when `isComposing` has already flipped back to false.
 *
 * The combination removes the need for a fragile time window guard
 * (which would otherwise swallow legitimate fast Enter presses) while
 * still rejecting the IME-confirmation Enter on every platform we
 * tested.
 *
 * Reference behaviour mirrors how Slack / Discord / Lark handle the
 * same race condition.
 */

import { useCallback, useRef } from 'react';

export interface ImeEnterGuard {
  isImeEnter: (e: React.KeyboardEvent) => boolean;
  handleCompositionStart: () => void;
  handleCompositionEnd: () => void;
}

export function useImeEnterGuard(): ImeEnterGuard {
  const isImeComposingRef = useRef(false);

  const handleCompositionStart = useCallback(() => {
    isImeComposingRef.current = true;
  }, []);

  const handleCompositionEnd = useCallback(() => {
    isImeComposingRef.current = false;
  }, []);

  const isImeEnter = useCallback((e: React.KeyboardEvent) => {
    const native = e.nativeEvent as KeyboardEvent | undefined;
    if (isImeComposingRef.current) return true;
    if (native?.isComposing) return true;
    // `keyCode === 229` is the canonical IME "in-flight" signal and is
    // still emitted by every evergreen browser even though the field is
    // marked legacy. It catches the race where the IME swallows Enter
    // to confirm a candidate but `isComposing` has already cleared.
    if (native?.keyCode === 229) return true;
    return false;
  }, []);

  return { isImeEnter, handleCompositionStart, handleCompositionEnd };
}

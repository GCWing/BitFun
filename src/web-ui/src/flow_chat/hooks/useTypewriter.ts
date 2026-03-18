/**
 * Typewriter hook for smoothing batched streaming updates.
 *
 * The EventBatcher flushes content every ~100ms, which makes text appear
 * in jarring chunks. This hook interpolates between batched updates to
 * produce a smooth character-by-character reveal.
 *
 * When `animate` is false the full text is returned immediately —
 * suitable for completed / history items.
 */

import { useState, useEffect, useRef } from 'react';

const FRAME_INTERVAL = 50; // ms per tick – aligned with MutationObserver throttle
const REVEAL_DURATION = 800; // ms to reveal a new batch
const MIN_CHARS_PER_TICK = 3;

export function useTypewriter(targetText: string, animate: boolean): string {
  const [displayText, setDisplayText] = useState(animate ? '' : targetText);
  const revealedRef = useRef(animate ? 0 : targetText.length);
  const targetRef = useRef(targetText);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const speedRef = useRef(MIN_CHARS_PER_TICK);

  useEffect(() => {
    if (!animate) {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
      revealedRef.current = targetText.length;
      targetRef.current = targetText;
      setDisplayText(targetText);
      return;
    }

    targetRef.current = targetText;

    // Reset when target shrinks (e.g. new round).
    if (targetText.length < revealedRef.current) {
      revealedRef.current = 0;
    }

    const delta = targetText.length - revealedRef.current;
    if (delta > 0) {
      const totalFrames = REVEAL_DURATION / FRAME_INTERVAL;
      speedRef.current = Math.max(Math.ceil(delta / totalFrames), MIN_CHARS_PER_TICK);

      if (!timerRef.current) {
        timerRef.current = setInterval(() => {
          const target = targetRef.current;
          const cur = revealedRef.current;
          if (cur >= target.length) {
            if (timerRef.current) {
              clearInterval(timerRef.current);
              timerRef.current = null;
            }
            return;
          }
          const next = Math.min(cur + speedRef.current, target.length);
          revealedRef.current = next;
          setDisplayText(target.slice(0, next));
        }, FRAME_INTERVAL);
      }
    }
  }, [targetText, animate]);

  useEffect(() => {
    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
      }
    };
  }, []);

  return displayText;
}

import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';

/** Keep a newly completed tail card visible briefly before compacting it. */
export const TOOL_CARD_COMPLETION_PREVIEW_GRACE_MS = 800;

interface UseToolCardCompletionGracePeriodOptions {
  eligible: boolean;
  isRevealing?: boolean;
  durationMs?: number;
}

/**
 * Starts at most one completion grace period for the current tail state.
 * The caller explicitly starts it from its existing auto-expansion decision so
 * the first completed render cannot collapse before the active state is set.
 */
export function useToolCardCompletionGracePeriod({
  eligible,
  isRevealing = false,
  durationMs = TOOL_CARD_COMPLETION_PREVIEW_GRACE_MS,
}: UseToolCardCompletionGracePeriodOptions) {
  const [isActive, setIsActive] = useState(false);
  const isActiveRef = useRef(false);
  const hasStartedRef = useRef(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const reset = useCallback(() => {
    clearTimer();
    hasStartedRef.current = false;
    isActiveRef.current = false;
    setIsActive(previous => (previous ? false : previous));
  }, [clearTimer]);

  const expire = useCallback(() => {
    timerRef.current = null;
    isActiveRef.current = false;
    setIsActive(false);
  }, []);

  const schedule = useCallback(() => {
    if (!isActiveRef.current || isRevealing || timerRef.current !== null) {
      return;
    }

    timerRef.current = setTimeout(expire, Math.max(0, durationMs));
  }, [durationMs, expire, isRevealing]);

  const begin = useCallback(() => {
    if (!eligible) {
      reset();
      return false;
    }

    if (hasStartedRef.current) {
      if (isActiveRef.current && !isRevealing) {
        schedule();
      }
      return isActiveRef.current;
    }

    hasStartedRef.current = true;
    isActiveRef.current = true;
    setIsActive(true);
    if (!isRevealing) {
      schedule();
    }
    return true;
  }, [eligible, isRevealing, reset, schedule]);

  useLayoutEffect(() => {
    if (!eligible) {
      reset();
      return;
    }

    if (isActiveRef.current) {
      if (isRevealing) {
        clearTimer();
      } else {
        schedule();
      }
    }
  }, [clearTimer, eligible, isRevealing, reset, schedule]);

  useEffect(() => () => {
    clearTimer();
    isActiveRef.current = false;
    hasStartedRef.current = false;
  }, [clearTimer]);

  return { begin, isActive };
}

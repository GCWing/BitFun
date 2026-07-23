import { useState, useEffect, useRef, useCallback } from 'react';

export interface UseLiveElapsedTimeResult {
  elapsedMs: number;
  remainingMs: number | null;
}

/**
 * Live elapsed time tracker for running subagent/tool cards.
 *
 * @param startTime - Tool start timestamp (ms). If undefined, returns 0.
 * @param isRunning - Whether the tool is currently running.
 * @param timeoutMs - Current effective timeout in ms. 0 or undefined = no timeout.
 * @param isTimeoutDisabled - Whether the timeout has been disabled by user.
 */
export function useLiveElapsedTime(
  startTime: number | undefined,
  isRunning: boolean,
  timeoutMs: number | undefined,
  isTimeoutDisabled: boolean,
): UseLiveElapsedTimeResult {
  const [elapsedMs, setElapsedMs] = useState(0);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const startTimeRef = useRef(startTime);
  const isRunningRef = useRef(isRunning);
  const timeoutMsRef = useRef(timeoutMs);
  const isTimeoutDisabledRef = useRef(isTimeoutDisabled);

  const computeElapsed = useCallback(() => {
    const start = startTimeRef.current;
    if (!start) return 0;
    return Math.max(0, Date.now() - start);
  }, []);

  const computeRemaining = useCallback((elapsed: number) => {
    if (isTimeoutDisabledRef.current) return null;
    const timeout = timeoutMsRef.current;
    if (!timeout || timeout <= 0) return null;
    return Math.max(0, timeout - elapsed);
  }, []);

  useEffect(() => {
    startTimeRef.current = startTime;
    timeoutMsRef.current = timeoutMs;
    isTimeoutDisabledRef.current = isTimeoutDisabled;
  });

  useEffect(() => {
    isRunningRef.current = isRunning;
    if (!isRunning) {
      // Final update when stopping, then clear interval.
      const finalElapsed = computeElapsed();
      setElapsedMs(finalElapsed);
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
      return;
    }

    // Running: update immediately then start interval.
    const update = () => {
      const elapsed = computeElapsed();
      setElapsedMs(elapsed);
    };
    update();
    intervalRef.current = setInterval(update, 1000);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [isRunning, computeElapsed]);

  const remainingMs = computeRemaining(elapsedMs);

  return { elapsedMs, remainingMs };
}

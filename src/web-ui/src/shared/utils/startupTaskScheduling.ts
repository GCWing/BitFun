export interface StartupPaintSchedulingOptions {
  frameCount?: number;
  requestAnimationFrame?: (callback: FrameRequestCallback) => number;
  cancelAnimationFrame?: (handle: number) => void;
  setTimeout?: (callback: () => void, timeout: number) => number;
  clearTimeout?: (handle: number) => void;
  onError?: (error: unknown) => void;
}

export interface StartupSignalSchedulingOptions extends StartupPaintSchedulingOptions {
  signalName?: string;
  signalTarget?: Pick<EventTarget, 'addEventListener' | 'removeEventListener'>;
  fallbackTimeoutMs?: number;
}

export function scheduleAfterStartupPaint(
  task: () => void | Promise<void>,
  options: StartupPaintSchedulingOptions = {}
): () => void {
  const frameCount = Math.max(1, Math.floor(options.frameCount ?? 2));
  const requestFrame =
    options.requestAnimationFrame ?? globalThis.requestAnimationFrame?.bind(globalThis);
  const cancelFrame =
    options.cancelAnimationFrame ?? globalThis.cancelAnimationFrame?.bind(globalThis);
  const setTimer =
    options.setTimeout ?? ((callback, timeout) => globalThis.setTimeout(callback, timeout) as unknown as number);
  const clearTimer =
    options.clearTimeout ?? ((handle) => globalThis.clearTimeout(handle));

  let cancelled = false;
  let activeFrameHandle: number | null = null;
  let activeTimerHandle: number | null = null;

  const runTask = () => {
    if (cancelled) {
      return;
    }
    activeFrameHandle = null;
    activeTimerHandle = null;
    try {
      void Promise.resolve(task()).catch(error => {
        options.onError?.(error);
      });
    } catch (error) {
      options.onError?.(error);
    }
  };

  if (!requestFrame) {
    activeTimerHandle = setTimer(runTask, 0);
    return () => {
      cancelled = true;
      if (activeTimerHandle !== null) {
        clearTimer(activeTimerHandle);
        activeTimerHandle = null;
      }
    };
  }

  let remainingFrames = frameCount;
  const scheduleNextFrame = () => {
    if (cancelled) {
      return;
    }
    activeFrameHandle = requestFrame(() => {
      remainingFrames -= 1;
      if (remainingFrames <= 0) {
        runTask();
        return;
      }
      scheduleNextFrame();
    });
  };

  scheduleNextFrame();

  return () => {
    cancelled = true;
    if (activeFrameHandle !== null && cancelFrame) {
      cancelFrame(activeFrameHandle);
      activeFrameHandle = null;
    }
  };
}

export function scheduleAfterStartupSignal(
  task: () => void | Promise<void>,
  options: StartupSignalSchedulingOptions = {}
): () => void {
  const signalName = options.signalName ?? 'bitfun:main-window-shown';
  const signalTarget =
    options.signalTarget ?? (typeof window !== 'undefined' ? window : undefined);
  const fallbackTimeoutMs = Math.max(0, options.fallbackTimeoutMs ?? 2000);
  const setTimer =
    options.setTimeout ?? ((callback, timeout) => globalThis.setTimeout(callback, timeout) as unknown as number);
  const clearTimer =
    options.clearTimeout ?? ((handle) => globalThis.clearTimeout(handle));

  let cancelled = false;
  let started = false;
  let fallbackTimer: number | null = null;
  let cancelPaintTask: (() => void) | null = null;

  const cleanupSignal = () => {
    signalTarget?.removeEventListener(signalName, startAfterSignal);
  };

  const clearFallback = () => {
    if (fallbackTimer !== null) {
      clearTimer(fallbackTimer);
      fallbackTimer = null;
    }
  };

  function startAfterSignal() {
    if (cancelled || started) {
      return;
    }
    started = true;
    cleanupSignal();
    clearFallback();
    cancelPaintTask = scheduleAfterStartupPaint(task, options);
  }

  if (signalTarget) {
    signalTarget.addEventListener(signalName, startAfterSignal);
    fallbackTimer = setTimer(startAfterSignal, fallbackTimeoutMs);
  } else {
    fallbackTimer = setTimer(startAfterSignal, 0);
  }

  return () => {
    cancelled = true;
    cleanupSignal();
    clearFallback();
    cancelPaintTask?.();
  };
}

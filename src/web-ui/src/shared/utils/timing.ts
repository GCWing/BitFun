export interface MeasuredValue<T> {
  value: T;
  durationMs: number;
}

export type LogLevelName = 'trace' | 'debug' | 'info' | 'warn' | 'error';

export interface LoggerLike {
  trace: (message: string, data?: unknown) => void;
  debug: (message: string, data?: unknown) => void;
  info: (message: string, data?: unknown) => void;
  warn: (message: string, data?: unknown) => void;
  error: (message: string, data?: unknown) => void;
}

export interface TimedLogOptions {
  level?: LogLevelName;
  data?: Record<string, unknown>;
}

export function nowMs(): number {
  return typeof performance !== 'undefined' ? performance.now() : Date.now();
}

export function roundDurationMs(durationMs: number): number {
  return Math.round(durationMs * 10) / 10;
}

export function elapsedMs(startedAt: number): number {
  return roundDurationMs(nowMs() - startedAt);
}

export function measureSync<T>(task: () => T): MeasuredValue<T> {
  const startedAt = nowMs();
  return {
    value: task(),
    durationMs: elapsedMs(startedAt),
  };
}

export async function measureAsync<T>(task: () => Promise<T> | T): Promise<MeasuredValue<T>> {
  const startedAt = nowMs();
  return {
    value: await task(),
    durationMs: elapsedMs(startedAt),
  };
}

export function logWithLevel(
  logger: LoggerLike,
  level: LogLevelName,
  message: string,
  data?: Record<string, unknown>
): void {
  logger[level](message, data);
}

export function logDuration(
  logger: LoggerLike,
  message: string,
  durationMs: number,
  options: TimedLogOptions = {}
): number {
  const roundedDurationMs = roundDurationMs(durationMs);
  logWithLevel(logger, options.level ?? 'debug', message, {
    ...(options.data ?? {}),
    durationMs: roundedDurationMs,
  });
  return roundedDurationMs;
}

export function logElapsed(
  logger: LoggerLike,
  message: string,
  startedAt: number,
  options: TimedLogOptions = {}
): number {
  return logDuration(logger, message, elapsedMs(startedAt), options);
}

export function measureSyncAndLog<T>(
  logger: LoggerLike,
  message: string,
  task: () => T,
  options: TimedLogOptions = {}
): MeasuredValue<T> {
  const result = measureSync(task);
  logDuration(logger, message, result.durationMs, options);
  return result;
}

export async function measureAsyncAndLog<T>(
  logger: LoggerLike,
  message: string,
  task: () => Promise<T> | T,
  options: TimedLogOptions = {}
): Promise<MeasuredValue<T>> {
  const result = await measureAsync(task);
  logDuration(logger, message, result.durationMs, options);
  return result;
}

/**
 * Wrap a promise with a timeout that rejects after `ms` milliseconds.
 * The optional `cleanup` callback runs when the timeout fires, allowing
 * callers to abort in-flight work (e.g. remove DOM nodes, abort requests).
 *
 * @param promise - The promise to wrap
 * @param ms - Timeout in milliseconds
 * @param label - Label for error messages (e.g. 'domToPng capture')
 * @param cleanup - Optional callback invoked when timeout fires
 * @returns Promise that resolves/rejects with the original promise, or rejects on timeout
 */
export function withTimeout<T>(
  promise: Promise<T>,
  ms: number,
  label: string,
  cleanup?: () => void,
): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  return Promise.race([
    promise,
    new Promise<never>((_, reject) => {
      timer = setTimeout(() => {
        cleanup?.();
        reject(new Error(`${label} timed out after ${ms}ms`));
      }, ms);
    }),
  ]).finally(() => clearTimeout(timer));
}

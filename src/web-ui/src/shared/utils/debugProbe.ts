import { createLogger } from './logger';
import { logElapsed, logWithLevel, type LogLevelName } from './timing';

type DebugProbeData = Record<string, unknown>;
type DebugProbeOptions = {
  level?: LogLevelName;
  startedAt?: number;
};

const log = createLogger('DebugProbe');

function serializeProbeValue(value: unknown): unknown {
  if (value instanceof Error) {
    return {
      name: value.name,
      message: value.message,
    };
  }

  if (value instanceof Set) {
    return Array.from(value);
  }

  if (value instanceof Map) {
    return Object.fromEntries(value);
  }

  return value;
}

export function sendDebugProbe(
  location: string,
  message: string,
  data?: DebugProbeData,
  options: DebugProbeOptions = {}
): void {
  const serializedData = data
    ? Object.fromEntries(
        Object.entries(data).map(([key, value]) => [key, serializeProbeValue(value)])
      )
    : undefined;

  const payload: Record<string, unknown> = {
    location,
    ...(serializedData ?? {}),
  };

  if (
    options.startedAt !== undefined &&
    typeof payload.durationMs !== 'number'
  ) {
    logElapsed(log, message, options.startedAt, {
      level: options.level ?? 'debug',
      data: payload,
    });
    return;
  }

  logWithLevel(log, options.level ?? 'debug', message, payload);
}

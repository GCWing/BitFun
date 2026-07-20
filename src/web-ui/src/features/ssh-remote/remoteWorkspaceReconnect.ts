/**
 * Remote workspace auto-reconnect timing and deadline-driven retry.
 *
 * The reconnect budget is a wall-clock window: keep attempting until the
 * deadline, then give up and allow callers to remove the workspace.
 * A single fast-failing connect must not exhaust the budget.
 */

export const REMOTE_WORKSPACE_RECONNECT_TIMEOUT_MS = 180_000;
export const REMOTE_WORKSPACE_RECONNECT_RETRY_WAIT_MS = 10_000;
/** Cap for one connect attempt so fast failures can still retry within the budget. */
export const REMOTE_WORKSPACE_RECONNECT_ATTEMPT_TIMEOUT_MS = 30_000;

export function remoteReconnectTimeoutSeconds(
  timeoutMs: number = REMOTE_WORKSPACE_RECONNECT_TIMEOUT_MS
): number {
  return Math.round(timeoutMs / 1000);
}

export type ReconnectUntilDeadlineOptions<T> = {
  totalTimeoutMs: number;
  attemptTimeoutMs?: number;
  retryWaitMs?: number;
  now?: () => number;
  sleep?: (ms: number) => Promise<void>;
  attempt: (attemptTimeoutMs: number, attempt: number) => Promise<T>;
};

export async function reconnectUntilDeadline<T>(
  options: ReconnectUntilDeadlineOptions<T>
): Promise<T | false> {
  const now = options.now ?? Date.now;
  const sleep =
    options.sleep ??
    ((ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms)));
  const attemptTimeoutMs =
    options.attemptTimeoutMs ?? REMOTE_WORKSPACE_RECONNECT_ATTEMPT_TIMEOUT_MS;
  const retryWaitMs = options.retryWaitMs ?? REMOTE_WORKSPACE_RECONNECT_RETRY_WAIT_MS;
  const deadline = now() + options.totalTimeoutMs;

  let attempt = 0;
  while (now() < deadline) {
    attempt += 1;
    const remaining = deadline - now();
    if (remaining <= 0) {
      break;
    }
    const timeoutForAttempt = Math.min(attemptTimeoutMs, remaining);
    try {
      return await options.attempt(timeoutForAttempt, attempt);
    } catch {
      const waitBudget = deadline - now();
      if (waitBudget <= 0) {
        break;
      }
      await sleep(Math.min(retryWaitMs, waitBudget));
    }
  }

  return false;
}

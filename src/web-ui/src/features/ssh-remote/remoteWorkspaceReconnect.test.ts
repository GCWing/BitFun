import { describe, expect, it, vi } from 'vitest';

import {
  REMOTE_WORKSPACE_RECONNECT_TIMEOUT_MS,
  reconnectUntilDeadline,
  remoteReconnectTimeoutSeconds,
} from './remoteWorkspaceReconnect';

describe('remoteWorkspaceReconnect', () => {
  it('exposes a 180-second reconnect budget', () => {
    expect(REMOTE_WORKSPACE_RECONNECT_TIMEOUT_MS).toBe(180_000);
    expect(remoteReconnectTimeoutSeconds()).toBe(180);
  });

  it('keeps retrying after fast failures until the deadline', async () => {
    let clock = 0;
    const attempts: number[] = [];
    const sleepCalls: number[] = [];

    const result = await reconnectUntilDeadline({
      totalTimeoutMs: 180_000,
      attemptTimeoutMs: 30_000,
      retryWaitMs: 10_000,
      now: () => clock,
      sleep: async ms => {
        sleepCalls.push(ms);
        clock += ms;
      },
      attempt: async (_timeoutMs, attempt) => {
        attempts.push(attempt);
        // Instant failure — must not exhaust the budget on the first try.
        throw new Error('connection refused');
      },
    });

    expect(result).toBe(false);
    expect(attempts.length).toBeGreaterThan(1);
    expect(attempts.length).toBeGreaterThanOrEqual(10);
    expect(sleepCalls.length).toBe(attempts.length);
    expect(clock).toBeGreaterThanOrEqual(180_000);
  });

  it('returns success as soon as an attempt succeeds within the budget', async () => {
    let clock = 0;
    let attemptCount = 0;

    const result = await reconnectUntilDeadline({
      totalTimeoutMs: 180_000,
      attemptTimeoutMs: 30_000,
      retryWaitMs: 10_000,
      now: () => clock,
      sleep: async ms => {
        clock += ms;
      },
      attempt: async () => {
        attemptCount += 1;
        if (attemptCount < 3) {
          throw new Error('temporary failure');
        }
        return { connectionId: 'conn-ok' };
      },
    });

    expect(result).toEqual({ connectionId: 'conn-ok' });
    expect(attemptCount).toBe(3);
    expect(clock).toBeLessThan(180_000);
  });

  it('does not wait past the deadline after the final failed attempt', async () => {
    let clock = 0;
    const sleepCalls: number[] = [];

    const result = await reconnectUntilDeadline({
      totalTimeoutMs: 12_000,
      attemptTimeoutMs: 30_000,
      retryWaitMs: 10_000,
      now: () => clock,
      sleep: async ms => {
        sleepCalls.push(ms);
        clock += ms;
      },
      attempt: async () => {
        throw new Error('still down');
      },
    });

    expect(result).toBe(false);
    expect(sleepCalls).toEqual([10_000, 2_000]);
    expect(clock).toBe(12_000);
  });
});

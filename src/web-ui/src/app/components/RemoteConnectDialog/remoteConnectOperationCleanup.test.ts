import { describe, expect, it, vi } from 'vitest';
import {
  stopAfterPendingStart,
  updateIfOperationCurrent,
} from './remoteConnectOperationCleanup';

describe('stopAfterPendingStart', () => {
  it('does not stop until a late start has settled', async () => {
    let resolveStart!: () => void;
    const start = new Promise<void>((resolve) => { resolveStart = resolve; });
    const stop = vi.fn(async () => undefined);

    const cleanup = stopAfterPendingStart(start, stop);
    await Promise.resolve();
    expect(stop).not.toHaveBeenCalled();

    resolveStart();
    await cleanup;
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it('still performs cleanup when start rejects', async () => {
    const stop = vi.fn(async () => undefined);
    await stopAfterPendingStart(Promise.reject(new Error('start failed')), stop);
    expect(stop).toHaveBeenCalledTimes(1);
  });
});

describe('updateIfOperationCurrent', () => {
  it('drops a stale operation cleanup instead of clearing replacement UI', () => {
    const update = vi.fn();

    expect(updateIfOperationCurrent(() => false, update)).toBe(false);
    expect(update).not.toHaveBeenCalled();
  });

  it('applies cleanup while the operation still owns the UI', () => {
    const update = vi.fn();

    expect(updateIfOperationCurrent(() => true, update)).toBe(true);
    expect(update).toHaveBeenCalledTimes(1);
  });
});

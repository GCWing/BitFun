import { describe, expect, it, vi, afterEach } from 'vitest';
import {
  BackgroundTaskCancelledError,
  BackgroundTaskScheduler,
} from './backgroundTaskScheduler';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('BackgroundTaskScheduler', () => {
  afterEach(() => {
    vi.useRealTimers();
    delete (globalThis as any).requestIdleCallback;
    delete (globalThis as any).cancelIdleCallback;
  });

  it('respects concurrency limit and runs higher priority queued tasks first', async () => {
    const scheduler = new BackgroundTaskScheduler({ concurrency: 1 });
    const first = deferred<string>();
    const order: string[] = [];

    const firstTask = scheduler.schedule(async () => {
      order.push('first:start');
      await first.promise;
      order.push('first:end');
      return 'first';
    }, { priority: 'low' });

    const lowTask = scheduler.schedule(async () => {
      order.push('low');
      return 'low';
    }, { priority: 'low' });

    const highTask = scheduler.schedule(async () => {
      order.push('high');
      return 'high';
    }, { priority: 'high' });

    await Promise.resolve();
    expect(order).toEqual(['first:start']);

    first.resolve('first');
    await expect(firstTask.promise).resolves.toBe('first');
    await expect(highTask.promise).resolves.toBe('high');
    await expect(lowTask.promise).resolves.toBe('low');
    expect(order).toEqual(['first:start', 'first:end', 'high', 'low']);
  });

  it('deduplicates queued or running tasks with the same in-flight key', async () => {
    const scheduler = new BackgroundTaskScheduler({ concurrency: 1 });
    const task = vi.fn(async () => 'shared');

    const first = scheduler.schedule(task, { inFlightKey: 'startup:monaco' });
    const second = scheduler.schedule(task, { inFlightKey: 'startup:monaco' });

    await expect(first.promise).resolves.toBe('shared');
    await expect(second.promise).resolves.toBe('shared');
    expect(first.promise).toBe(second.promise);
    expect(task).toHaveBeenCalledTimes(1);
  });

  it('cancels queued work without running it', async () => {
    const scheduler = new BackgroundTaskScheduler({ concurrency: 1 });
    const blocker = deferred<void>();
    const queuedTask = vi.fn(async () => 'queued');

    scheduler.schedule(async () => {
      await blocker.promise;
    });
    const queued = scheduler.schedule(queuedTask);

    queued.cancel();
    blocker.resolve();

    await expect(queued.promise).rejects.toBeInstanceOf(BackgroundTaskCancelledError);
    expect(queuedTask).not.toHaveBeenCalled();
  });

  it('uses requestIdleCallback for idle tasks', async () => {
    const scheduler = new BackgroundTaskScheduler({ concurrency: 1 });
    const idleCallbacks: Array<() => void> = [];
    const task = vi.fn(async () => 'idle');
    (globalThis as any).requestIdleCallback = vi.fn((callback: () => void) => {
      idleCallbacks.push(callback);
      return idleCallbacks.length;
    });
    (globalThis as any).cancelIdleCallback = vi.fn();

    const scheduled = scheduler.schedule(task, { idle: true });
    await Promise.resolve();

    expect(task).not.toHaveBeenCalled();
    expect((globalThis as any).requestIdleCallback).toHaveBeenCalledTimes(1);

    idleCallbacks[0]();
    await expect(scheduled.promise).resolves.toBe('idle');
    expect(task).toHaveBeenCalledTimes(1);
  });

  it('releases the concurrency slot when an idle task is cancelled', async () => {
    const scheduler = new BackgroundTaskScheduler({ concurrency: 1 });
    const task = vi.fn(async () => 'next');
    (globalThis as any).requestIdleCallback = vi.fn(() => 1);
    (globalThis as any).cancelIdleCallback = vi.fn();

    const idle = scheduler.schedule(async () => 'idle', { idle: true });
    idle.cancel();

    const next = scheduler.schedule(task);

    await expect(idle.promise).rejects.toBeInstanceOf(BackgroundTaskCancelledError);
    await expect(next.promise).resolves.toBe('next');
    expect(task).toHaveBeenCalledTimes(1);
  });
});

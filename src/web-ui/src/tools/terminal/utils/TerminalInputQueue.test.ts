import { describe, expect, it, vi } from 'vitest';

import { TerminalInputQueue } from './TerminalInputQueue';

/** Flush all pending microtasks by waiting for the next macrotask. */
function flushMicrotasks(): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, 0));
}

describe('TerminalInputQueue', () => {
  it('batches multiple synchronous enqueue calls into a single write', async () => {
    const write = vi.fn((_data: string): Promise<void> => Promise.resolve());
    const onError = vi.fn();
    const queue = new TerminalInputQueue(write, onError);

    queue.enqueue('1');
    queue.enqueue('2');
    queue.enqueue('3');

    // Microtask hasn't run yet — no writes should have occurred.
    expect(write).not.toHaveBeenCalled();

    await flushMicrotasks();

    expect(write).toHaveBeenCalledTimes(1);
    expect(write).toHaveBeenCalledWith('123');
    expect(onError).not.toHaveBeenCalled();
  });

  it('does not start a second write while the first is in flight', async () => {
    let resolveFirst: () => void = () => {};
    const write = vi.fn((data: string): Promise<void> => {
      if (data === 'first') {
        return new Promise<void>(resolve => {
          resolveFirst = resolve;
        });
      }
      return Promise.resolve();
    });
    const onError = vi.fn();
    const queue = new TerminalInputQueue(write, onError);

    queue.enqueue('first');
    await flushMicrotasks();

    expect(write).toHaveBeenCalledTimes(1);
    expect(write).toHaveBeenLastCalledWith('first');

    // Enqueue more data while the first write is still in flight.
    queue.enqueue('A');
    queue.enqueue('B');

    expect(write).toHaveBeenCalledTimes(1); // Still only one call.

    // Resolve the first write — the queued data should flush as a single batch.
    resolveFirst();
    await flushMicrotasks();

    expect(write).toHaveBeenCalledTimes(2);
    expect(write).toHaveBeenLastCalledWith('AB');
  });

  it('preserves ordering across multiple flush cycles', async () => {
    const writtenData: string[] = [];
    const write = vi.fn((data: string): Promise<void> => {
      writtenData.push(data);
      return Promise.resolve();
    });
    const onError = vi.fn();
    const queue = new TerminalInputQueue(write, onError);

    // First batch
    queue.enqueue('a');
    queue.enqueue('b');
    await flushMicrotasks();

    // Second batch
    queue.enqueue('c');
    queue.enqueue('d');
    await flushMicrotasks();

    expect(writtenData).toEqual(['ab', 'cd']);
  });

  it('reports write errors to onError and continues processing', async () => {
    const write = vi.fn((_data: string): Promise<void> => Promise.resolve());
    write.mockRejectedValueOnce(new Error('boom'));
    const onError = vi.fn();
    const queue = new TerminalInputQueue(write, onError);

    queue.enqueue('data');
    await flushMicrotasks();

    expect(onError).toHaveBeenCalledTimes(1);
    expect((onError.mock.calls[0][0] as Error).message).toBe('boom');

    // Queue should still accept new data after an error.
    queue.enqueue('more');
    await flushMicrotasks();

    expect(write).toHaveBeenCalledTimes(2);
    expect(write).toHaveBeenLastCalledWith('more');
    expect(onError).toHaveBeenCalledTimes(1);
  });

  it('clear() discards buffered data that has not been flushed', async () => {
    const write = vi.fn((_data: string): Promise<void> => Promise.resolve());
    const onError = vi.fn();
    const queue = new TerminalInputQueue(write, onError);

    queue.enqueue('pending');
    queue.clear();

    await flushMicrotasks();

    // The microtask still runs drain(), but the buffer was cleared,
    // so data is empty and write should not be called.
    expect(write).not.toHaveBeenCalled();
  });

  it('clear() does not affect data already mid-flush', async () => {
    let resolveWrite: () => void = () => {};
    const write = vi.fn((_data: string): Promise<void> =>
      new Promise<void>(resolve => {
        resolveWrite = resolve;
      }),
    );
    const onError = vi.fn();
    const queue = new TerminalInputQueue(write, onError);

    queue.enqueue('in-flight');
    await flushMicrotasks();

    expect(write).toHaveBeenCalledWith('in-flight');

    // Data that arrives after the flush started but before it completes.
    queue.enqueue('queued');
    // Clear should discard 'queued' but not affect 'in-flight'.
    queue.clear();

    resolveWrite();
    await flushMicrotasks();

    // Only 'in-flight' was written; 'queued' was discarded.
    expect(write).toHaveBeenCalledTimes(1);
    expect(write).toHaveBeenLastCalledWith('in-flight');
  });
});

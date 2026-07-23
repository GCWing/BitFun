import { createLogger } from './logger';

const log = createLogger('BackgroundTaskScheduler');

export type BackgroundTaskPriority = 'high' | 'normal' | 'low';

export interface BackgroundTaskOptions {
  priority?: BackgroundTaskPriority;
  idle?: boolean;
  inFlightKey?: string;
}

export interface BackgroundTaskHandle<T> {
  promise: Promise<T>;
  cancel: () => void;
}

export interface BackgroundTaskSchedulerOptions {
  concurrency?: number;
}

type TaskStatus = 'queued' | 'idle' | 'running' | 'settled' | 'cancelled';

interface TaskEntry<T> {
  id: number;
  priority: BackgroundTaskPriority;
  idle: boolean;
  inFlightKey?: string;
  status: TaskStatus;
  task: (signal: AbortSignal) => Promise<T> | T;
  controller: AbortController;
  promise: Promise<T>;
  resolve: (value: T | PromiseLike<T>) => void;
  reject: (reason?: unknown) => void;
  idleHandle?: number;
}

export class BackgroundTaskCancelledError extends Error {
  constructor() {
    super('Background task cancelled');
    this.name = 'BackgroundTaskCancelledError';
  }
}

const PRIORITY_RANK: Record<BackgroundTaskPriority, number> = {
  high: 0,
  normal: 1,
  low: 2,
};

function requestIdle(callback: () => void): number {
  const requestIdleCallback = (globalThis as {
    requestIdleCallback?: (callback: () => void, options?: { timeout?: number }) => number;
  }).requestIdleCallback;

  if (typeof requestIdleCallback === 'function') {
    return requestIdleCallback(callback, { timeout: 2000 });
  }

  return globalThis.setTimeout(callback, 0) as unknown as number;
}

function cancelIdle(handle: number): void {
  const cancelIdleCallback = (globalThis as {
    cancelIdleCallback?: (handle: number) => void;
  }).cancelIdleCallback;

  if (typeof cancelIdleCallback === 'function') {
    cancelIdleCallback(handle);
    return;
  }

  globalThis.clearTimeout(handle);
}

export class BackgroundTaskScheduler {
  private readonly concurrency: number;
  private nextId = 1;
  private runningCount = 0;
  private queue: Array<TaskEntry<unknown>> = [];
  private keyedTasks = new Map<string, TaskEntry<unknown>>();

  constructor(options: BackgroundTaskSchedulerOptions = {}) {
    this.concurrency = Math.max(1, options.concurrency ?? 2);
  }

  schedule<T>(
    task: (signal: AbortSignal) => Promise<T> | T,
    options: BackgroundTaskOptions = {}
  ): BackgroundTaskHandle<T> {
    const inFlightKey = options.inFlightKey;
    if (inFlightKey) {
      const existing = this.keyedTasks.get(inFlightKey) as TaskEntry<T> | undefined;
      if (existing && existing.status !== 'cancelled' && existing.status !== 'settled') {
        return this.toHandle(existing);
      }
    }

    let resolve!: (value: T | PromiseLike<T>) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<T>((res, rej) => {
      resolve = res;
      reject = rej;
    });

    const entry: TaskEntry<T> = {
      id: this.nextId++,
      priority: options.priority ?? 'normal',
      idle: options.idle ?? false,
      inFlightKey,
      status: 'queued',
      task,
      controller: new AbortController(),
      promise,
      resolve,
      reject,
    };

    this.queue.push(entry as TaskEntry<unknown>);
    if (inFlightKey) {
      this.keyedTasks.set(inFlightKey, entry as TaskEntry<unknown>);
    }
    this.drain();

    return this.toHandle(entry);
  }

  private toHandle<T>(entry: TaskEntry<T>): BackgroundTaskHandle<T> {
    return {
      promise: entry.promise,
      cancel: () => this.cancel(entry),
    };
  }

  private cancel<T>(entry: TaskEntry<T>): void {
    if (entry.status === 'settled' || entry.status === 'cancelled') {
      return;
    }

    if (entry.status === 'running') {
      entry.controller.abort();
      return;
    }

    const shouldReleaseSlot = entry.status === 'idle';
    entry.status = 'cancelled';
    entry.controller.abort();
    if (entry.idleHandle !== undefined) {
      cancelIdle(entry.idleHandle);
      entry.idleHandle = undefined;
    }
    this.queue = this.queue.filter((item) => item !== entry);
    this.deleteKeyIfCurrent(entry);
    entry.reject(new BackgroundTaskCancelledError());
    if (shouldReleaseSlot) {
      this.runningCount -= 1;
    }
    this.drain();
  }

  private drain(): void {
    while (this.runningCount < this.concurrency) {
      const next = this.shiftNextRunnable();
      if (!next) {
        return;
      }

      this.runningCount += 1;
      if (next.idle) {
        next.status = 'idle';
        next.idleHandle = requestIdle(() => {
          next.idleHandle = undefined;
          void this.run(next);
        });
      } else {
        void this.run(next);
      }
    }
  }

  private shiftNextRunnable(): TaskEntry<unknown> | null {
    this.queue = this.queue.filter((entry) => entry.status === 'queued');
    if (this.queue.length === 0) {
      return null;
    }

    this.queue.sort((a, b) => {
      const priorityDiff = PRIORITY_RANK[a.priority] - PRIORITY_RANK[b.priority];
      return priorityDiff !== 0 ? priorityDiff : a.id - b.id;
    });

    return this.queue.shift() ?? null;
  }

  private async run<T>(entry: TaskEntry<T>): Promise<void> {
    if (entry.status === 'cancelled') {
      this.runningCount -= 1;
      this.drain();
      return;
    }

    entry.status = 'running';
    try {
      if (entry.controller.signal.aborted) {
        throw new BackgroundTaskCancelledError();
      }
      entry.resolve(await entry.task(entry.controller.signal));
    } catch (error) {
      if (entry.controller.signal.aborted && !(error instanceof BackgroundTaskCancelledError)) {
        entry.reject(new BackgroundTaskCancelledError());
      } else {
        entry.reject(error);
      }
      if (!(error instanceof BackgroundTaskCancelledError)) {
        log.warn('Background task failed', { inFlightKey: entry.inFlightKey, error });
      }
    } finally {
      entry.status = 'settled';
      this.deleteKeyIfCurrent(entry);
      this.runningCount -= 1;
      this.drain();
    }
  }

  private deleteKeyIfCurrent<T>(entry: TaskEntry<T>): void {
    if (!entry.inFlightKey) {
      return;
    }
    if (this.keyedTasks.get(entry.inFlightKey) === entry) {
      this.keyedTasks.delete(entry.inFlightKey);
    }
  }
}

export const backgroundTaskScheduler = new BackgroundTaskScheduler();

import { describe, expect, it, vi } from 'vitest';
import { scheduleMonacoStartupWarmup } from './MonacoStartupWarmup';

describe('scheduleMonacoStartupWarmup', () => {
  it('schedules Monaco initialization as low-priority idle background work', async () => {
    const initializeMonaco = vi.fn(async () => undefined);
    const initializeThemeSync = vi.fn(async () => undefined);
    const signal = { aborted: false } as AbortSignal;
    const schedule = vi.fn((task: (signal: AbortSignal) => Promise<void>, options: unknown) => ({
      promise: task(signal),
      cancel: vi.fn(),
      options,
    }));

    const handle = scheduleMonacoStartupWarmup({
      scheduler: { schedule },
      initializeMonaco,
      initializeThemeSync,
    });

    expect(schedule).toHaveBeenCalledWith(expect.any(Function), {
      idle: true,
      inFlightKey: 'startup:monaco-warmup',
      priority: 'low',
    });
    await expect(handle.promise).resolves.toBeUndefined();
    expect(initializeMonaco).toHaveBeenCalledTimes(1);
    expect(initializeThemeSync).toHaveBeenCalledTimes(1);
  });

  it('skips theme sync when the warmup task is cancelled before Monaco resolves', async () => {
    const initializeMonaco = vi.fn(async () => undefined);
    const initializeThemeSync = vi.fn(async () => undefined);
    const signal = { aborted: true } as AbortSignal;
    const schedule = vi.fn((task: (signal: AbortSignal) => Promise<void>, options: unknown) => ({
      promise: task(signal),
      cancel: vi.fn(),
      options,
    }));

    const handle = scheduleMonacoStartupWarmup({
      scheduler: { schedule },
      initializeMonaco,
      initializeThemeSync,
    });

    await expect(handle.promise).resolves.toBeUndefined();
    expect(initializeMonaco).toHaveBeenCalledTimes(1);
    expect(initializeThemeSync).not.toHaveBeenCalled();
  });
});

import { describe, expect, it, vi } from 'vitest';
import { scheduleMonacoStartupWarmup } from './MonacoStartupWarmup';

describe('scheduleMonacoStartupWarmup', () => {
  it('schedules editor runtime warmup as low-priority idle background work', async () => {
    const order: string[] = [];
    const initializeMonaco = vi.fn(async () => {
      order.push('monaco');
    });
    const initializeThemeSync = vi.fn(async () => {
      order.push('theme');
    });
    const preloadEditorSurfaceStages = [
      { name: 'code_editor', run: vi.fn(async () => { order.push('code'); }) },
      { name: 'diff_editor', run: vi.fn(async () => { order.push('diff'); }) },
      { name: 'git_diff_editor', run: vi.fn(async () => { order.push('git'); }) },
    ];
    const waitForIdle = vi.fn(async () => {
      order.push('idle');
    });
    const trace = { markPhase: vi.fn() };
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
      preloadEditorSurfaceStages,
      waitForIdle,
      trace,
    });

    expect(trace.markPhase).toHaveBeenCalledWith('editor_startup_warmup_scheduled', {
      idle: true,
      priority: 'low',
    });
    expect(schedule).toHaveBeenCalledWith(expect.any(Function), {
      idle: true,
      inFlightKey: 'startup:monaco-warmup',
      priority: 'low',
    });
    await expect(handle.promise).resolves.toBeUndefined();
    expect(preloadEditorSurfaceStages[0].run).toHaveBeenCalledTimes(1);
    expect(preloadEditorSurfaceStages[1].run).toHaveBeenCalledTimes(1);
    expect(preloadEditorSurfaceStages[2].run).toHaveBeenCalledTimes(1);
    expect(waitForIdle).toHaveBeenCalledTimes(4);
    expect(initializeMonaco).toHaveBeenCalledTimes(1);
    expect(initializeThemeSync).toHaveBeenCalledTimes(1);
    expect(order).toEqual([
      'code',
      'idle',
      'diff',
      'idle',
      'git',
      'idle',
      'monaco',
      'idle',
      'theme',
    ]);
    expect(trace.markPhase).toHaveBeenCalledWith('editor_startup_warmup_end');
  });

  it('skips editor warmup work when cancelled before execution', async () => {
    const initializeMonaco = vi.fn(async () => undefined);
    const initializeThemeSync = vi.fn(async () => undefined);
    const preloadEditorSurfaceStages = [
      { name: 'code_editor', run: vi.fn(async () => undefined) },
    ];
    const waitForIdle = vi.fn(async () => undefined);
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
      preloadEditorSurfaceStages,
      waitForIdle,
      trace: { markPhase: vi.fn() },
    });

    await expect(handle.promise).resolves.toBeUndefined();
    expect(preloadEditorSurfaceStages[0].run).not.toHaveBeenCalled();
    expect(waitForIdle).not.toHaveBeenCalled();
    expect(initializeMonaco).not.toHaveBeenCalled();
    expect(initializeThemeSync).not.toHaveBeenCalled();
  });
});

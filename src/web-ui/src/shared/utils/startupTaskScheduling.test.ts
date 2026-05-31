import { describe, expect, it, vi } from 'vitest';

import {
  scheduleAfterStartupPaint,
  scheduleAfterStartupSignal,
} from './startupTaskScheduling';

describe('scheduleAfterStartupPaint', () => {
  it('runs work only after the requested animation frames', () => {
    const callbacks: Array<(time: number) => void> = [];
    const task = vi.fn();

    scheduleAfterStartupPaint(task, {
      frameCount: 2,
      requestAnimationFrame: callback => {
        callbacks.push(callback);
        return callbacks.length;
      },
      cancelAnimationFrame: vi.fn(),
      setTimeout: vi.fn(),
      clearTimeout: vi.fn(),
    });

    expect(task).not.toHaveBeenCalled();
    expect(callbacks).toHaveLength(1);

    callbacks.shift()?.(16);
    expect(task).not.toHaveBeenCalled();
    expect(callbacks).toHaveLength(1);

    callbacks.shift()?.(32);
    expect(task).toHaveBeenCalledTimes(1);
  });

  it('cancels queued animation-frame work', () => {
    const callbacks: Array<(time: number) => void> = [];
    const cancelAnimationFrame = vi.fn();
    const task = vi.fn();

    const cancel = scheduleAfterStartupPaint(task, {
      frameCount: 2,
      requestAnimationFrame: callback => {
        callbacks.push(callback);
        return callbacks.length;
      },
      cancelAnimationFrame,
      setTimeout: vi.fn(),
      clearTimeout: vi.fn(),
    });

    cancel();
    callbacks.shift()?.(16);

    expect(cancelAnimationFrame).toHaveBeenCalledWith(1);
    expect(task).not.toHaveBeenCalled();
  });

  it('waits for the startup signal before scheduling paint-delayed work', () => {
    const callbacks: Array<(time: number) => void> = [];
    let signalHandler: (() => void) | null = null;
    const addEventListener = vi.fn((_name: string, handler: EventListenerOrEventListenerObject) => {
      signalHandler = handler as () => void;
    });
    const removeEventListener = vi.fn();
    const setTimeout = vi.fn();
    const task = vi.fn();

    scheduleAfterStartupSignal(task, {
      frameCount: 1,
      signalName: 'bitfun:main-window-shown',
      signalTarget: { addEventListener, removeEventListener },
      fallbackTimeoutMs: 2000,
      requestAnimationFrame: callback => {
        callbacks.push(callback);
        return callbacks.length;
      },
      cancelAnimationFrame: vi.fn(),
      setTimeout,
      clearTimeout: vi.fn(),
    });

    expect(addEventListener).toHaveBeenCalledWith('bitfun:main-window-shown', expect.any(Function));
    expect(setTimeout).toHaveBeenCalledWith(expect.any(Function), 2000);
    expect(task).not.toHaveBeenCalled();

    signalHandler?.();
    expect(task).not.toHaveBeenCalled();
    callbacks.shift()?.(16);

    expect(task).toHaveBeenCalledTimes(1);
    expect(removeEventListener).toHaveBeenCalledWith('bitfun:main-window-shown', expect.any(Function));
  });

  it('falls back when the startup signal is not observed', () => {
    const callbacks: Array<(time: number) => void> = [];
    let fallback: (() => void) | null = null;
    const task = vi.fn();

    scheduleAfterStartupSignal(task, {
      frameCount: 1,
      signalTarget: {
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      },
      fallbackTimeoutMs: 2000,
      requestAnimationFrame: callback => {
        callbacks.push(callback);
        return callbacks.length;
      },
      cancelAnimationFrame: vi.fn(),
      setTimeout: callback => {
        fallback = callback;
        return 1;
      },
      clearTimeout: vi.fn(),
    });

    fallback?.();
    callbacks.shift()?.(16);

    expect(task).toHaveBeenCalledTimes(1);
  });
});

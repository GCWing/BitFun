import { describe, expect, it, vi } from 'vitest';

import {
  useCanvasAction,
  useHostAppearance,
} from './hooks';

function withWindow<T>(run: () => T): T {
  const previous = (globalThis as typeof globalThis & { window?: Window }).window;
  (globalThis as typeof globalThis & { window?: Partial<Window> }).window = {};
  try {
    return run();
  } finally {
    (globalThis as typeof globalThis & { window?: Window }).window = previous;
  }
}

describe('OpenBitFun Canvas hook adapters', () => {
  it('delegates host appearance access to runtime hooks', () => {
    withWindow(() => {
      window.OpenBitFunCanvasRuntimeHooks = {
        useHostAppearance: () => ({ type: 'dark' }),
      };

      expect(useHostAppearance()).toEqual({ type: 'dark' });
    });
  });

  it('delegates canvas actions to runtime hooks', async () => {
    await withWindow(async () => {
      const action = vi.fn(async () => 'done');
      window.OpenBitFunCanvasRuntimeHooks = {
        useCanvasAction: () => action,
      };

      await expect(useCanvasAction()({ type: 'ping' })).resolves.toBe('done');
      expect(action).toHaveBeenCalledWith({ type: 'ping' });
    });
  });
});

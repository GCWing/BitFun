import { describe, expect, it, vi } from 'vitest';

import {
  clearStartupModuleReloadAttempt,
  isRecoverableStartupModuleLoadError,
  retryStartupAfterModuleLoadFailure,
} from './startupModuleRecovery';

function createStorage() {
  const values = new Map<string, string>();
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
    removeItem: (key: string) => values.delete(key),
  };
}

describe('startup module recovery', () => {
  it('recognizes WebKit and Chromium dynamic module load failures', () => {
    expect(isRecoverableStartupModuleLoadError(
      new TypeError('Importing a module script failed.'),
    )).toBe(true);
    expect(isRecoverableStartupModuleLoadError(
      new TypeError('Failed to fetch dynamically imported module'),
    )).toBe(true);
    expect(isRecoverableStartupModuleLoadError(new Error('render failed'))).toBe(false);
  });

  it('reloads at most once until a module import succeeds', () => {
    const storage = createStorage();
    const reload = vi.fn();
    const runtime = { isDevelopment: true, storage, reload };
    const error = new TypeError('Importing a module script failed.');

    expect(retryStartupAfterModuleLoadFailure(error, runtime)).toBe(true);
    expect(reload).toHaveBeenCalledOnce();
    expect(retryStartupAfterModuleLoadFailure(error, runtime)).toBe(false);
    expect(reload).toHaveBeenCalledOnce();

    clearStartupModuleReloadAttempt(storage);
    expect(retryStartupAfterModuleLoadFailure(error, runtime)).toBe(true);
    expect(reload).toHaveBeenCalledTimes(2);
  });

  it('does not reload production or non-module failures', () => {
    const storage = createStorage();
    const reload = vi.fn();

    expect(retryStartupAfterModuleLoadFailure(
      new TypeError('Importing a module script failed.'),
      { isDevelopment: false, storage, reload },
    )).toBe(false);
    expect(retryStartupAfterModuleLoadFailure(
      new Error('application initialization failed'),
      { isDevelopment: true, storage, reload },
    )).toBe(false);
    expect(reload).not.toHaveBeenCalled();
  });

  it('falls through to the error boundary if reload itself fails', () => {
    const storage = createStorage();
    const reload = vi.fn(() => {
      throw new Error('reload unavailable');
    });

    expect(retryStartupAfterModuleLoadFailure(
      new TypeError('Importing a module script failed.'),
      { isDevelopment: true, storage, reload },
    )).toBe(false);

    expect(retryStartupAfterModuleLoadFailure(
      new TypeError('Importing a module script failed.'),
      { isDevelopment: true, storage, reload: vi.fn() },
    )).toBe(true);
  });
});

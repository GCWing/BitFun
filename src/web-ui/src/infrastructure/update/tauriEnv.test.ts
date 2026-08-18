import { afterEach, describe, expect, it, vi } from 'vitest';

import { canCheckForAppUpdates } from './tauriEnv';

describe('app update runtime availability', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  it('disables update checks in a Tauri development runtime', () => {
    vi.stubEnv('DEV', true);
    vi.stubGlobal('window', { __TAURI__: {} });

    expect(canCheckForAppUpdates()).toBe(false);
  });

  it('enables update checks in a packaged Tauri runtime', () => {
    vi.stubEnv('DEV', false);
    vi.stubGlobal('window', { __TAURI__: {} });

    expect(canCheckForAppUpdates()).toBe(true);
  });
});

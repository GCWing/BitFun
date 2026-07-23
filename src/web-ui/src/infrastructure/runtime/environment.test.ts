import { afterEach, describe, expect, it, vi } from 'vitest';

import { isTauriRuntime, supportsNativeWindowControls } from './environment';

const setTauriInternals = (value: unknown) => {
  vi.stubGlobal('window', {
    __TAURI_INTERNALS__: value,
  });
};

describe('runtime environment', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('treats a plain browser as non-Tauri without native window controls', () => {
    vi.stubGlobal('window', {});

    expect(isTauriRuntime()).toBe(false);
    expect(supportsNativeWindowControls()).toBe(false);
  });

  it('requires current window metadata before enabling native window controls', () => {
    setTauriInternals({ invoke: vi.fn() });

    expect(isTauriRuntime()).toBe(true);
    expect(supportsNativeWindowControls()).toBe(false);
  });

  it('enables native window controls for a complete Tauri window runtime', () => {
    setTauriInternals({
      invoke: vi.fn(),
      metadata: {
        currentWindow: {
          label: 'main',
        },
      },
    });

    expect(isTauriRuntime()).toBe(true);
    expect(supportsNativeWindowControls()).toBe(true);
  });
});

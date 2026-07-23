import { describe, expect, it, vi } from 'vitest';

import { bitfunDarkTheme } from '../presets/dark-theme';
import { MonacoThemeSync } from './MonacoThemeSync';

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

function createMonacoStub() {
  const defineTheme = vi.fn();
  const setTheme = vi.fn();
  const getEditors = vi.fn(() => []);

  return {
    monaco: {
      editor: {
        defineTheme,
        setTheme,
        getEditors,
      },
    },
    defineTheme,
    setTheme,
  };
}

describe('MonacoThemeSync deferred runtime behavior', () => {
  it('keeps custom theme registrations until Monaco runtime is attached', () => {
    const sync = new MonacoThemeSync();
    const queuedTheme = {
      ...bitfunDarkTheme,
      id: 'queued-theme',
      name: 'Queued theme',
    };
    const { monaco, defineTheme } = createMonacoStub();

    sync.registerTheme(queuedTheme.id, queuedTheme);
    expect(defineTheme).not.toHaveBeenCalled();

    sync.attachMonaco(monaco as never);

    expect(defineTheme).toHaveBeenCalledWith(
      queuedTheme.id,
      expect.objectContaining({
        base: queuedTheme.monaco?.base,
        inherit: queuedTheme.monaco?.inherit,
      }),
    );
  });
});

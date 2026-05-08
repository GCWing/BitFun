import { JSDOM } from 'jsdom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ThemeService } from './ThemeService';

vi.mock('@/infrastructure/api', () => ({
  configAPI: {
    getConfig: vi.fn(),
    setConfig: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../integrations/MonacoThemeSync', () => ({
  monacoThemeSync: {
    syncTheme: vi.fn(),
  },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

describe('ThemeService flow chat link tokens', () => {
  let dom: JSDOM;

  beforeEach(() => {
    dom = new JSDOM('<!doctype html><html><body></body></html>');
    vi.stubGlobal('window', dom.window);
    vi.stubGlobal('document', dom.window.document);
    Object.defineProperty(dom.window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockReturnValue({
        matches: false,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      }),
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it('keeps light theme Flow Chat markdown links browser-blue even with a neutral app accent', async () => {
    const service = new ThemeService();

    await service.applyTheme('bitfun-light');

    const rootStyle = document.documentElement.style;
    expect(rootStyle.getPropertyValue('--color-accent-500')).toBe('#64748b');
    expect(rootStyle.getPropertyValue('--flowchat-link-color')).toBe('#0969da');
    expect(rootStyle.getPropertyValue('--flowchat-link-hover-color')).toBe('#0550ae');
  });

  it('keeps dark neutral-accent themes on an obvious blue link color', async () => {
    const service = new ThemeService();

    await service.applyTheme('bitfun-slate');

    const rootStyle = document.documentElement.style;
    expect(rootStyle.getPropertyValue('--color-accent-500')).toBe('#94a3b8');
    expect(rootStyle.getPropertyValue('--flowchat-link-color')).toBe('#60a5fa');
    expect(rootStyle.getPropertyValue('--flowchat-link-hover-color')).toBe('#93c5fd');
  });
});

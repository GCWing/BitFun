import { JSDOM } from 'jsdom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { configAPI } from '@/infrastructure/api';
import { bitfunDarkTheme, bitfunLightTheme } from '../presets';
import { SYSTEM_THEME_ID, type ThemeConfig } from '../types';
import { ThemeService } from './ThemeService';

function expectThemeError(
  result: ReturnType<ThemeService['validateTheme']>,
  path: string,
  code: string,
) {
  expect(result.errors).toEqual(expect.arrayContaining([expect.objectContaining({ path, code })]));
}

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

describe('ThemeService runtime theme tokens', () => {
  let dom: JSDOM;
  const bootstrapGlobals = globalThis as typeof globalThis & {
    __BITFUN_BOOTSTRAP_THEME_ID__?: string;
    __BITFUN_BOOTSTRAP_THEME_SELECTION__?: string;
  };

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
    delete bootstrapGlobals.__BITFUN_BOOTSTRAP_THEME_ID__;
    delete bootstrapGlobals.__BITFUN_BOOTSTRAP_THEME_SELECTION__;
    vi.mocked(configAPI.getConfig).mockResolvedValue(undefined);
    vi.mocked(configAPI.setConfig).mockResolvedValue(undefined);
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

  it('uses canonical light overlay stops for scrollbar fallback hover', async () => {
    const service = new ThemeService();

    await service.applyTheme('bitfun-light');

    expect(document.documentElement.style.getPropertyValue('--scrollbar-thumb-hover')).toBe('rgba(0, 0, 0, 0.3)');
  });

  it('keeps card subtle variants visually below the default card background', async () => {
    const service = new ThemeService();

    await service.applyTheme('bitfun-dark');

    expect(document.documentElement.style.getPropertyValue('--card-bg-default')).toBe('rgba(255, 255, 255, 0.04)');
    expect(document.documentElement.style.getPropertyValue('--card-bg-subtle')).toBe('rgba(255, 255, 255, 0.02)');

    await service.applyTheme('bitfun-light');

    expect(document.documentElement.style.getPropertyValue('--card-bg-default')).toBe('rgba(0, 0, 0, 0.06)');
    expect(document.documentElement.style.getPropertyValue('--card-bg-subtle')).toBe('transparent');
  });

  it('keeps dark info border aligned with the canonical medium overlay stop', async () => {
    const service = new ThemeService();

    await service.applyTheme('bitfun-dark');

    expect(document.documentElement.style.getPropertyValue('--color-info-border')).toBe('rgba(255, 255, 255, 0.24)');
  });

  it('uses canonical dark overlay stops when a theme omits scrollbar values', () => {
    const service = new ThemeService();
    const fallbackTheme: ThemeConfig = {
      ...bitfunDarkTheme,
      id: 'fallback-dark',
      colors: {
        ...bitfunDarkTheme.colors,
        scrollbar: undefined,
      },
    } as unknown as ThemeConfig;

    (service as unknown as { injectCSSVariables(theme: ThemeConfig): void }).injectCSSVariables(fallbackTheme);

    expect(document.documentElement.style.getPropertyValue('--scrollbar-thumb-hover')).toBe('rgba(255, 255, 255, 0.24)');
  });

  it('initializes from bootstrap theme selection without reading or writing themes.current', async () => {
    bootstrapGlobals.__BITFUN_BOOTSTRAP_THEME_ID__ = 'bitfun-slate';
    bootstrapGlobals.__BITFUN_BOOTSTRAP_THEME_SELECTION__ = 'bitfun-slate';
    const service = new ThemeService();

    await service.initialize();

    expect(service.getCurrentThemeId()).toBe('bitfun-slate');
    expect(document.documentElement.getAttribute('data-theme')).toBe('bitfun-slate');
    expect(configAPI.getConfig).not.toHaveBeenCalled();
    expect(configAPI.getConfig).not.toHaveBeenCalledWith(
      'themes.current',
      expect.anything(),
    );
    expect(configAPI.setConfig).not.toHaveBeenCalledWith(
      'themes.current',
      expect.anything(),
    );
  });

  it('loads custom themes on demand after initialization and deduplicates repeated loads', async () => {
    bootstrapGlobals.__BITFUN_BOOTSTRAP_THEME_ID__ = 'bitfun-slate';
    bootstrapGlobals.__BITFUN_BOOTSTRAP_THEME_SELECTION__ = 'bitfun-slate';
    const service = new ThemeService();
    await service.initialize();

    await service.ensureUserThemesLoaded();
    await service.ensureUserThemesLoaded();

    expect(configAPI.getConfig).toHaveBeenCalledTimes(1);
    expect(configAPI.getConfig).toHaveBeenCalledWith(
      'themes',
      expect.objectContaining({ skipRetryOnNotFound: true }),
    );
  });

  it('falls back to config lookup when bootstrap theme selection is unavailable', async () => {
    bootstrapGlobals.__BITFUN_BOOTSTRAP_THEME_ID__ = 'bitfun-light';
    vi.mocked(configAPI.getConfig).mockImplementation(async (key: string) => {
      if (key === 'themes.current') {
        return 'bitfun-slate';
      }
      return undefined;
    });
    const service = new ThemeService();

    await service.initialize();

    expect(service.getCurrentThemeId()).toBe('bitfun-slate');
    expect(configAPI.getConfig).toHaveBeenCalledWith(
      'themes.current',
      expect.objectContaining({ skipRetryOnNotFound: true }),
    );
  });

  it('applies saved custom theme during initialization when bootstrap cannot provide it', async () => {
    const customTheme: ThemeConfig = {
      ...bitfunLightTheme,
      id: 'custom-ocean',
      name: 'Custom Ocean',
      colors: {
        ...bitfunLightTheme.colors,
        background: {
          ...bitfunLightTheme.colors.background,
          primary: '#001122',
        },
      },
    };
    vi.mocked(configAPI.getConfig).mockImplementation(async (key: string) => {
      if (key === 'themes.current') {
        return 'custom-ocean';
      }
      if (key === 'themes') {
        return { custom: [customTheme] };
      }
      return undefined;
    });
    const service = new ThemeService();

    await service.initialize();
    await service.ensureUserThemesLoaded();

    expect(service.getCurrentThemeId()).toBe('custom-ocean');
    expect(service.getResolvedThemeId()).toBe('custom-ocean');
    expect(document.documentElement.getAttribute('data-theme')).toBe('custom-ocean');
    expect(document.documentElement.style.getPropertyValue('--color-bg-primary')).toBe('#001122');
    expect(configAPI.getConfig).toHaveBeenCalledWith(
      'themes',
      expect.objectContaining({ skipRetryOnNotFound: true }),
    );
    expect(vi.mocked(configAPI.getConfig).mock.calls.filter(([key]) => key === 'themes')).toHaveLength(1);
    expect(configAPI.setConfig).not.toHaveBeenCalledWith('themes.current', 'custom-ocean');
  });

  it('does not persist the theme selection again during initialization', async () => {
    vi.mocked(configAPI.getConfig).mockImplementation(async (key: string) => {
      if (key === 'themes.current') {
        return 'bitfun-slate';
      }
      return undefined;
    });
    const service = new ThemeService();

    await service.initialize();

    expect(configAPI.setConfig).not.toHaveBeenCalledWith(
      'themes.current',
      expect.anything(),
    );
  });

  it('validates the core theme schema instead of only root fields', () => {
    const service = new ThemeService();
    const invalidTheme: ThemeConfig = {
      ...bitfunLightTheme,
      id: 'custom-invalid-semantic',
      name: 'Invalid Semantic',
      colors: {
        ...bitfunLightTheme.colors,
        semantic: {
          ...bitfunLightTheme.colors.semantic,
          success: 'not-a-color',
        },
      },
    };

    const result = service.validateTheme(invalidTheme);

    expect(result.valid).toBe(false);
    expectThemeError(result, 'colors.semantic.success', 'INVALID_COLOR_FORMAT');

    const incompleteTheme = {
      ...bitfunLightTheme,
      id: 'custom-incomplete',
      name: 'Incomplete Custom',
      effects: undefined,
      motion: undefined,
      typography: undefined,
    } as unknown as ThemeConfig;
    const incompleteResult = service.validateTheme(incompleteTheme);

    expect(incompleteResult.valid).toBe(false);
    expectThemeError(incompleteResult, 'effects', 'MISSING_THEME_FIELD_GROUP');
    expectThemeError(incompleteResult, 'motion', 'MISSING_THEME_FIELD_GROUP');
    expectThemeError(incompleteResult, 'typography', 'MISSING_THEME_FIELD_GROUP');

    const invalidOptionalTheme = {
      ...bitfunLightTheme,
      id: 'custom-invalid-optional-scrollbar',
      name: 'Invalid Optional Scrollbar',
      colors: {
        ...bitfunLightTheme.colors,
        scrollbar: {
          thumb: 'invalid',
          thumbHover: '#ffffff',
        },
      },
    } as unknown as ThemeConfig;
    const invalidOptionalResult = service.validateTheme(invalidOptionalTheme);

    expect(invalidOptionalResult.valid).toBe(false);
    expectThemeError(invalidOptionalResult, 'colors.scrollbar.thumb', 'INVALID_COLOR_FORMAT');
  });

  it('normalizes older partial custom themes before applying them', async () => {
    const partialCustomTheme = {
      id: 'custom-partial',
      name: 'Partial Custom',
      type: 'light',
      colors: {
        background: {
          primary: '#101820',
        },
        text: {
          primary: '#f8fafc',
        },
        accent: {
          500: '#2f80ed',
        },
      },
    } as unknown as ThemeConfig;
    vi.mocked(configAPI.getConfig).mockImplementation(async (key: string) => {
      if (key === 'themes.current') {
        return 'custom-partial';
      }
      if (key === 'themes') {
        return { custom: [partialCustomTheme] };
      }
      return undefined;
    });
    const service = new ThemeService();

    await service.initialize();

    const normalized = service.getTheme('custom-partial');
    expect(service.getCurrentThemeId()).toBe('custom-partial');
    expect(service.getResolvedThemeId()).toBe('custom-partial');
    expect(normalized?.colors.background.primary).toBe('#101820');
    expect(normalized?.colors.background.secondary).toBe(bitfunLightTheme.colors.background.secondary);
    expect(normalized?.colors.text.primary).toBe('#f8fafc');
    expect(normalized?.colors.text.secondary).toBe(bitfunLightTheme.colors.text.secondary);
    expect(normalized?.effects.spacing[4]).toBe(bitfunLightTheme.effects.spacing[4]);
    expect(document.documentElement.style.getPropertyValue('--color-bg-primary')).toBe('#101820');
    expect(document.documentElement.style.getPropertyValue('--color-bg-secondary')).toBe(
      bitfunLightTheme.colors.background.secondary,
    );
    expect(configAPI.setConfig).not.toHaveBeenCalledWith('themes.custom', expect.anything());
  });

  it('does not export non-contract dynamic keys from custom themes', () => {
    const service = new ThemeService();
    const customTheme = {
      ...bitfunLightTheme,
      id: 'custom-extra-keys',
      colors: {
        ...bitfunLightTheme.colors,
        accent: {
          ...bitfunLightTheme.colors.accent,
          950: '#111111',
        },
        purple: {
          ...bitfunLightTheme.colors.purple,
          300: '#222222',
          700: '#333333',
        },
      },
      effects: {
        ...bitfunLightTheme.effects,
        shadow: {
          ...bitfunLightTheme.effects.shadow,
          '2xl': '0 0 0 #111111',
        },
        blur: {
          ...bitfunLightTheme.effects.blur,
          intense: 'blur(99px)',
        },
        radius: {
          ...bitfunLightTheme.effects.radius,
          huge: '99px',
        },
        spacing: {
          ...bitfunLightTheme.effects.spacing,
          99: '99px',
        },
      },
      motion: {
        ...bitfunLightTheme.motion,
        duration: {
          ...bitfunLightTheme.motion.duration,
          lazy: '99s',
        },
        easing: {
          ...bitfunLightTheme.motion.easing,
          bounce: 'cubic-bezier(0.68, -0.55, 0.265, 1.55)',
        },
      },
      typography: {
        ...bitfunLightTheme.typography,
        weight: {
          ...bitfunLightTheme.typography.weight,
          black: 900,
        },
        size: {
          ...bitfunLightTheme.typography.size,
          '5xl': '99px',
        },
        lineHeight: {
          ...bitfunLightTheme.typography.lineHeight,
          loose: 2,
        },
      },
    } as unknown as ThemeConfig;

    (service as unknown as { injectCSSVariables(theme: ThemeConfig): void }).injectCSSVariables(customTheme);

    const rootStyle = document.documentElement.style;
    expect(rootStyle.getPropertyValue('--color-accent-950')).toBe('');
    expect(rootStyle.getPropertyValue('--color-purple-300')).toBe('');
    expect(rootStyle.getPropertyValue('--color-purple-700')).toBe('');
    expect(rootStyle.getPropertyValue('--shadow-2xl')).toBe('');
    expect(rootStyle.getPropertyValue('--blur-intense')).toBe('');
    expect(rootStyle.getPropertyValue('--size-radius-huge')).toBe('');
    expect(rootStyle.getPropertyValue('--size-gap-99')).toBe('');
    expect(rootStyle.getPropertyValue('--motion-lazy')).toBe('');
    expect(rootStyle.getPropertyValue('--easing-bounce')).toBe('');
    expect(rootStyle.getPropertyValue('--font-weight-black')).toBe('');
    expect(rootStyle.getPropertyValue('--font-size-5xl')).toBe('');
    expect(rootStyle.getPropertyValue('--line-height-loose')).toBe('');
  });

  it('skips invalid persisted custom themes before they reach preview or runtime injection', async () => {
    const invalidCustomTheme = {
      ...bitfunLightTheme,
      id: 'custom-broken',
      name: 'Broken Custom',
      colors: {
        ...bitfunLightTheme.colors,
        background: {
          ...bitfunLightTheme.colors.background,
          primary: 'definitely-not-a-color',
        },
      },
    };
    vi.mocked(configAPI.getConfig).mockImplementation(async (key: string) => {
      if (key === 'themes.current') {
        return 'custom-broken';
      }
      if (key === 'themes') {
        return { custom: [invalidCustomTheme] };
      }
      return undefined;
    });
    const service = new ThemeService();

    await service.initialize();

    expect(service.getTheme('custom-broken')).toBeUndefined();
    expect(service.getCurrentThemeId()).toBe(SYSTEM_THEME_ID);
    expect(document.documentElement.getAttribute('data-theme')).not.toBe('custom-broken');
    expect(configAPI.setConfig).not.toHaveBeenCalledWith('themes.custom', expect.anything());
  });

  it('persists registered custom themes only after schema normalization succeeds', async () => {
    const service = new ThemeService();
    const partialCustomTheme = {
      id: 'custom-registered',
      name: 'Registered Custom',
      type: 'dark',
      colors: {
        background: {
          primary: '#04080f',
        },
        text: {
          primary: '#f8fafc',
        },
        accent: {
          500: '#7c3aed',
        },
      },
    } as unknown as ThemeConfig;

    await service.registerTheme(partialCustomTheme);

    const normalized = service.getTheme('custom-registered');
    expect(normalized?.colors.background.primary).toBe('#04080f');
    expect(normalized?.colors.background.secondary).toBe(bitfunDarkTheme.colors.background.secondary);
    expect(normalized?.effects.radius.base).toBe(bitfunDarkTheme.effects.radius.base);
    expect(configAPI.setConfig).toHaveBeenCalledWith(
      'themes.custom',
      expect.arrayContaining([
        expect.objectContaining({
          id: 'custom-registered',
          colors: expect.objectContaining({
            background: expect.objectContaining({
              primary: '#04080f',
              secondary: bitfunDarkTheme.colors.background.secondary,
            }),
          }),
        }),
      ]),
    );

    await expect(
      service.registerTheme({
        ...bitfunLightTheme,
        id: 'custom-invalid-register',
        colors: {
          ...bitfunLightTheme.colors,
          text: {
            ...bitfunLightTheme.colors.text,
            primary: 'invalid',
          },
        },
      }),
    ).rejects.toThrow(/Invalid theme/);
    expect(service.getTheme('custom-invalid-register')).toBeUndefined();

    await expect(
      service.registerTheme({
        ...bitfunLightTheme,
        id: '',
        name: '',
      }),
    ).rejects.toThrow(/Theme id cannot be empty/);

    await expect(
      service.registerTheme({
        ...bitfunLightTheme,
        name: 'Builtin Override',
      }),
    ).rejects.toThrow(/reserved for a built-in theme/);
  });
});

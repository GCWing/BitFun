import { createHash } from 'node:crypto';
import { describe, expect, it } from 'vitest';

import { builtinAppearancePalettes } from './palettes';
import {
  getBuiltinAppearance,
  getBuiltinAppearanceThemeTokens,
} from './catalog';
import {
  PLUGIN_APPEARANCE_COLOR_KEYS,
  createPluginAppearanceColorProjection,
} from '../adapters/PluginAppearanceProjection';
import {
  createAccentScale,
  createGitColors,
  createSemanticColors,
  createSecondaryAccentScale,
  overlayBlack,
  overlayWhite,
  rgbFromHex,
  rgbaFromHex,
} from './paletteHelpers';

function hashAppearance(appearance: unknown): string {
  return createHash('sha256')
    .update(JSON.stringify(appearance))
    .digest('hex');
}

describe('builtin appearance preset output', () => {
  it('formats hex palette references as stable rgb strings', () => {
    expect(rgbFromHex('#00e6ff')).toBe('rgb(0, 230, 255)');
    expect(rgbaFromHex('#00e6ff', 0.12)).toBe('rgba(0, 230, 255, 0.12)');
    expect(rgbaFromHex('#00e6ff', '0.12')).toBe('rgba(0, 230, 255, 0.12)');
    expect(overlayBlack(0.3)).toBe('rgba(0, 0, 0, 0.3)');
    expect(overlayWhite(0.08)).toBe('rgba(255, 255, 255, 0.08)');
  });

  it('aliases staged git colors to added colors unless an appearance overrides them', () => {
    expect(createGitColors({
      branch: '#64748b',
      branchBg: 'rgba(100, 116, 139, 0.1)',
      changes: '#f59e0b',
      added: '#22c55e',
      deleted: '#ef4444',
    })).toMatchObject({
      staged: '#22c55e',
    });

    expect(createGitColors({
      branch: '#64748b',
      branchBg: 'rgba(100, 116, 139, 0.1)',
      changes: '#f59e0b',
      added: '#22c55e',
      deleted: '#ef4444',
      staged: '#10b981',
    })).toMatchObject({
      staged: '#10b981',
    });
  });

  it('derives repeated palette families from compact authoring inputs', () => {
    expect(createAccentScale({
      base: '#60a5fa',
      hover: '#3b82f6',
    })).toEqual({
      50: 'rgba(96, 165, 250, 0.04)',
      100: 'rgba(96, 165, 250, 0.08)',
      200: 'rgba(96, 165, 250, 0.15)',
      300: 'rgba(96, 165, 250, 0.25)',
      400: 'rgba(96, 165, 250, 0.4)',
      500: '#60a5fa',
      600: '#3b82f6',
      700: 'rgba(59, 130, 246, 0.8)',
    });

    expect(createSecondaryAccentScale({
      base: '#8b5cf6',
      hover: '#7c3aed',
    })).toEqual({
      100: 'rgba(139, 92, 246, 0.08)',
      200: 'rgba(139, 92, 246, 0.15)',
      500: '#8b5cf6',
      600: '#7c3aed',
    });

    expect(createSemanticColors({
      success: '#34d399',
      warning: '#f59e0b',
      error: '#ef4444',
      info: '#a1a1aa',
    })).toMatchObject({
      successBg: 'rgba(52, 211, 153, 0.1)',
      successBorder: 'rgba(52, 211, 153, 0.3)',
      warningBg: 'rgba(245, 158, 11, 0.1)',
      errorBorder: 'rgba(239, 68, 68, 0.3)',
      infoBg: 'rgba(161, 161, 170, 0.1)',
      infoBorder: 'rgba(161, 161, 170, 0.3)',
    });
  });

  it('does not carry retired runtime-only authoring stops in builtin appearance schemas', () => {
    for (const appearance of builtinAppearancePalettes) {
      expect(appearance.colors.accent).not.toHaveProperty('800');
      expect(appearance.colors.purple).not.toHaveProperty('50');
      expect(appearance.colors.purple).not.toHaveProperty('400');
      expect(appearance.colors.purple).not.toHaveProperty('800');
      expect(appearance.colors.background).not.toHaveProperty('quaternary');
      expect(appearance.colors.background).not.toHaveProperty('tooltip');
      expect(appearance.colors.element).not.toHaveProperty('elevated');
    }
  });

  it('keeps approved near-neutral preset stops scoped to their semantic roles', () => {
    const serializedAppearances = JSON.stringify(builtinAppearancePalettes).toLowerCase();
    const lightAppearance = builtinAppearancePalettes.find(appearance => appearance.id === 'openbitfun-light');

    expect(lightAppearance?.colors.background.primary).toBe('#fdfdfd');
    expect(lightAppearance?.monaco?.colors.background).toBe('#ffffff');
    expect(lightAppearance?.monaco?.colors.lineHighlight).toBe('rgba(16, 26, 39, 0.03)');
    expect(serializedAppearances.match(/#fdfdfd/g)).toHaveLength(1);
    expect(serializedAppearances).not.toContain('#e2e6eb');
    expect(serializedAppearances).not.toContain('#f0f2f5');
  });

  it('keeps the default light appearance on the neutral, navy, and restrained semantic palette', () => {
    const lightAppearance = builtinAppearancePalettes.find(appearance => appearance.id === 'openbitfun-light');
    const tokens = getBuiltinAppearanceThemeTokens('openbitfun-light');

    expect(lightAppearance).toMatchObject({
      description: 'Light appearance - Crisp white surfaces, soft neutral grays, deep navy actions',
      version: '2.5.0',
      colors: {
        background: {
          primary: '#fdfdfd',
          secondary: '#ffffff',
          tertiary: '#f7f7f7',
          elevated: '#ffffff',
          workbench: '#f3f3f5',
          scene: '#ffffff',
          chrome: '#f8f8f9',
        },
        text: {
          primary: 'rgba(0, 0, 0, 0.80)',
          secondary: 'rgba(0, 0, 0, 0.60)',
          muted: '#6a6a6a',
          disabled: 'rgba(0, 0, 0, 0.30)',
        },
        accent: {
          50: 'rgba(16, 26, 39, 0.03)',
          100: '#f3f3f5',
          500: '#101a27',
          600: '#1c1c1f',
          700: '#000000',
        },
        semantic: {
          success: '#247344',
          successBg: '#e1fbe9',
          successBorder: '#247344',
          error: '#a74352',
          errorBg: 'rgba(167, 67, 82, 0.12)',
          info: '#555555',
          infoBg: '#f3f3f5',
          infoBorder: 'rgba(16, 26, 39, 0.15)',
        },
        border: {
          base: 'rgba(16, 26, 39, 0.15)',
        },
        element: {
          subtle: 'rgba(16, 26, 39, 0.03)',
          soft: '#f3f3f5',
        },
      },
      components: {
        button: {
          primary: {
            default: { background: '#101a27', color: '#ffffff' },
            hover: { background: '#1c1c1f', color: '#ffffff' },
            active: { background: '#000000', color: '#ffffff' },
          },
        },
      },
      monaco: {
        colors: {
          background: '#ffffff',
          lineHighlight: 'rgba(16, 26, 39, 0.03)',
        },
      },
    });
    expect(tokens).toMatchObject({
      '--openbitfun-color-surface-chrome': '#f8f8f9',
      '--openbitfun-color-selection-surface': 'rgba(0, 0, 0, 0.08)',
      '--openbitfun-component-config-page-section-background': '#f7f7f7',
      '--openbitfun-component-config-page-section-border': 'rgba(16, 26, 39, 0.08)',
      '--openbitfun-component-config-page-section-border-width': '1px',
      '--openbitfun-component-config-page-divider': 'rgba(16, 26, 39, 0.08)',
    });
  });

  it('keeps monochrome content readable while projecting inverse structural chrome', () => {
    const monochrome = builtinAppearancePalettes.find(
      appearance => appearance.id === 'openbitfun-monochrome',
    );
    const monochromePackage = getBuiltinAppearance('openbitfun-monochrome');
    const tokens = getBuiltinAppearanceThemeTokens('openbitfun-monochrome');
    const chromeTokens = monochromePackage?.renderers?.['theme-tokens']?.settings.scopes?.chrome;

    expect(monochrome).toMatchObject({
      type: 'light',
      description: 'Black-and-white contrast appearance - Deep black chrome, bright white workspace, soft neutral blocks',
      colors: {
        background: {
          primary: '#ffffff',
          scene: '#ffffff',
        },
        text: {
          primary: 'rgba(0, 0, 0, 0.80)',
          secondary: 'rgba(0, 0, 0, 0.60)',
          muted: '#6a6a6a',
        },
        border: {
          subtle: 'rgba(16, 26, 39, 0.08)',
          base: 'rgba(16, 26, 39, 0.15)',
          prominent: 'rgba(16, 26, 39, 0.48)',
        },
        element: {
          subtle: 'rgba(16, 26, 39, 0.03)',
          soft: '#f3f3f5',
          strong: 'rgba(0, 0, 0, 0.10)',
        },
        accent: {
          500: '#1c1c1f',
          600: '#000000',
        },
        chrome: {
          background: {
            primary: '#1c1c1f',
            secondary: '#262626',
          },
          text: {
            primary: '#f3f3f5',
            secondary: '#b0b0b0',
            muted: '#858585',
            disabled: '#555555',
          },
          accent: {
            500: '#f3f3f5',
            600: '#ffffff',
          },
        },
      },
      components: {
        button: {
          primary: {
            default: { background: '#1c1c1f', color: '#ffffff' },
            hover: { background: '#000000', color: '#ffffff' },
          },
        },
        configPage: {
          section: {
            background: '#f3f3f5',
            border: 'transparent',
            borderWidth: '0',
            shadow: 'none',
          },
          divider: 'rgba(16, 26, 39, 0.08)',
          rowHover: 'rgba(16, 26, 39, 0.03)',
        },
      },
    });
    expect(tokens).toMatchObject({
      '--openbitfun-color-surface-canvas': '#ffffff',
      '--openbitfun-color-content-primary': 'rgba(0, 0, 0, 0.80)',
      '--openbitfun-color-content-secondary': 'rgba(0, 0, 0, 0.60)',
      '--openbitfun-color-content-disabled': 'rgba(0, 0, 0, 0.30)',
      '--openbitfun-color-border-subtle': 'rgba(16, 26, 39, 0.08)',
      '--openbitfun-color-border-default': 'rgba(16, 26, 39, 0.15)',
      '--openbitfun-color-surface-subtle': 'rgba(16, 26, 39, 0.03)',
      '--openbitfun-color-action-quiet-hover': '#f3f3f5',
      '--openbitfun-color-scrollbar-thumb': 'rgba(16, 26, 39, 0.15)',
      '--openbitfun-component-config-page-section-background': '#f3f3f5',
      '--openbitfun-component-config-page-section-border': 'transparent',
      '--openbitfun-component-config-page-section-border-width': '0',
      '--openbitfun-component-config-page-section-shadow': 'none',
      '--openbitfun-component-config-page-divider': 'rgba(16, 26, 39, 0.08)',
    });
    expect(chromeTokens).toMatchObject({
      '--openbitfun-color-surface-canvas': '#1c1c1f',
      '--openbitfun-color-content-primary': '#f3f3f5',
      '--openbitfun-color-action-quiet-hover': 'rgba(255, 255, 255, 0.06)',
    });
  });

  it('projects builtin appearances to a compact OpenCode-compatible plugin color key set', () => {
    expect(PLUGIN_APPEARANCE_COLOR_KEYS).toEqual([
      'primary',
      'secondary',
      'accent',
      'success',
      'warning',
      'error',
      'info',
    ]);

    for (const appearance of builtinAppearancePalettes) {
      const projection = createPluginAppearanceColorProjection(appearance);

      expect(Object.keys(projection).sort()).toEqual([...PLUGIN_APPEARANCE_COLOR_KEYS].sort());
      expect(projection.primary).toBe(appearance.colors.accent[500]);
      expect(projection.secondary).toBe(appearance.colors.purple?.[500] ?? appearance.colors.accent[600]);
      expect(projection.accent).toBe(appearance.colors.accent[600]);
      expect(projection.success).toBe(appearance.colors.semantic.success);
      expect(projection.warning).toBe(appearance.colors.semantic.warning);
      expect(projection.error).toBe(appearance.colors.semantic.error);
      expect(projection.info).toBe(appearance.colors.semantic.info);
    }
  });

  it('keeps resolved preset objects stable across helper refactors', () => {
    expect(builtinAppearancePalettes.map(appearance => ({
      id: appearance.id,
      type: appearance.type,
      hash: hashAppearance(appearance),
    }))).toMatchInlineSnapshot(`
      [
        {
          "hash": "50d3928d5d563c0a24663862668fe172e24266182ead5fb8052b5d7fe272a8ec",
          "id": "openbitfun-light",
          "type": "light",
        },
        {
          "hash": "7bb69b925ebe3be3a161e74cb615763685996d1cd0a29e31bfc0e1760ae2890c",
          "id": "openbitfun-monochrome",
          "type": "light",
        },
        {
          "hash": "1e318b55bb667a5f0dd042c02189c907e367b73ef01c4733e9d2c2828b47f2c2",
          "id": "openbitfun-slate",
          "type": "dark",
        },
        {
          "hash": "a07c7a8671a46dce4210c116f64a7953f78c1ebec3c86f45f8766844db15054c",
          "id": "openbitfun-dark",
          "type": "dark",
        },
        {
          "hash": "a50b8c195173166c247ca75b97eaaea3d59eb698741e12289d72cc8493359761",
          "id": "openbitfun-midnight",
          "type": "dark",
        },
        {
          "hash": "c1e0a46d859aa191c8c4b753ccf2867a91832babecc33a49634f82fb1631f0bc",
          "id": "openbitfun-china-style",
          "type": "light",
        },
        {
          "hash": "17449840509547a810f8cfd85823ea62e162352018a1857c86d68650fd6d028e",
          "id": "openbitfun-china-night",
          "type": "dark",
        },
        {
          "hash": "9ec2b937bc9c21caa8482c4d9008b38786f74afdef00b4e6949886f50f5e0b86",
          "id": "openbitfun-cyber",
          "type": "dark",
        },
        {
          "hash": "d3a84aafd59824a6b3716e5560034e4b5901068aa9af8e07f891c6543d0dcf58",
          "id": "openbitfun-tokyo-night",
          "type": "dark",
        },
      ]
    `);
  });
});

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
    const lightAppearance = builtinAppearancePalettes.find(appearance => appearance.id === 'bitfun-light');

    expect(lightAppearance?.colors.background.primary).toBe('#fdfdfd');
    expect(lightAppearance?.monaco?.colors.background).toBe('#ffffff');
    expect(lightAppearance?.monaco?.colors.lineHighlight).toBe('rgba(16, 26, 39, 0.03)');
    expect(serializedAppearances.match(/#fdfdfd/g)).toHaveLength(1);
    expect(serializedAppearances).not.toContain('#e2e6eb');
    expect(serializedAppearances).not.toContain('#f0f2f5');
  });

  it('keeps the default light appearance on the neutral, navy, and restrained semantic palette', () => {
    const lightAppearance = builtinAppearancePalettes.find(appearance => appearance.id === 'bitfun-light');
    const tokens = getBuiltinAppearanceThemeTokens('bitfun-light');

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
          primary: '#1c1c1f',
          secondary: '#555555',
          muted: '#6a6a6a',
          disabled: '#9a9a9a',
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
      '--bf-color-surface-chrome': '#f8f8f9',
      '--bf-color-selection-surface': 'rgba(0, 0, 0, 0.08)',
      '--bf-component-config-page-section-background': '#f7f7f7',
      '--bf-component-config-page-section-border': 'rgba(16, 26, 39, 0.08)',
      '--bf-component-config-page-section-border-width': '1px',
      '--bf-component-config-page-divider': 'rgba(16, 26, 39, 0.08)',
    });
  });

  it('keeps monochrome content readable while projecting inverse structural chrome', () => {
    const monochrome = builtinAppearancePalettes.find(
      appearance => appearance.id === 'bitfun-monochrome',
    );
    const monochromePackage = getBuiltinAppearance('bitfun-monochrome');
    const tokens = getBuiltinAppearanceThemeTokens('bitfun-monochrome');
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
          primary: '#1c1c1f',
          secondary: '#555555',
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
      '--bf-color-surface-canvas': '#ffffff',
      '--bf-color-content-primary': '#1c1c1f',
      '--bf-color-content-secondary': '#555555',
      '--bf-color-border-subtle': 'rgba(16, 26, 39, 0.08)',
      '--bf-color-border-default': 'rgba(16, 26, 39, 0.15)',
      '--bf-color-surface-subtle': 'rgba(16, 26, 39, 0.03)',
      '--bf-color-action-quiet-hover': '#f3f3f5',
      '--bf-color-scrollbar-thumb': 'rgba(16, 26, 39, 0.15)',
      '--bf-component-config-page-section-background': '#f3f3f5',
      '--bf-component-config-page-section-border': 'transparent',
      '--bf-component-config-page-section-border-width': '0',
      '--bf-component-config-page-section-shadow': 'none',
      '--bf-component-config-page-divider': 'rgba(16, 26, 39, 0.08)',
    });
    expect(chromeTokens).toMatchObject({
      '--bf-color-surface-canvas': '#1c1c1f',
      '--bf-color-content-primary': '#f3f3f5',
      '--bf-color-action-quiet-hover': 'rgba(255, 255, 255, 0.06)',
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
          "hash": "cf3b1df5872d83daab7f9dd33671a6f82537c1e07cb90d6a49ac7d7a416cd045",
          "id": "bitfun-light",
          "type": "light",
        },
        {
          "hash": "21924d3ea4f17d63e89538e539ed7cdea263b0c31682e1378221e5ac46937d78",
          "id": "bitfun-monochrome",
          "type": "light",
        },
        {
          "hash": "d5d100c9f013b8827b800f2c4183f8da540af6f01118738603658145bc9f0953",
          "id": "bitfun-slate",
          "type": "dark",
        },
        {
          "hash": "12f16e11bc459ed7f48de3a59a3cd8eab0f66694023f40ce0ce529d45bbf453e",
          "id": "bitfun-dark",
          "type": "dark",
        },
        {
          "hash": "74be0da6f5a9357aacc287c3ceac869aa9057cf6235193a5363b6c06e30c996e",
          "id": "bitfun-midnight",
          "type": "dark",
        },
        {
          "hash": "7327064fbfe41d709c942a480829cf33ea1c2dcbc6552ece012a4f43ef15fdbe",
          "id": "bitfun-china-style",
          "type": "light",
        },
        {
          "hash": "ba820850c6bb14e35e128eb2c950ca999542b525577ff5f58ba5a1d7f6704dd8",
          "id": "bitfun-china-night",
          "type": "dark",
        },
        {
          "hash": "6bf3b8ccc3df8e57eeda3c6fedddc4cd5220d3c03a4733b3e4ddbfc9b6ae0e5c",
          "id": "bitfun-cyber",
          "type": "dark",
        },
        {
          "hash": "ed40c5b47a335e87db582174afc55bc28849647daf61ad809a321c9258a9f7fc",
          "id": "bitfun-tokyo-night",
          "type": "dark",
        },
      ]
    `);
  });
});

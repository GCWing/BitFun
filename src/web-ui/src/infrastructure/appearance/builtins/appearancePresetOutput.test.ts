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

  it('keeps settings row hover feedback separated from dark scene surfaces', () => {
    const darkAppearance = builtinAppearancePalettes.find(
      appearance => appearance.id === 'openbitfun-dark',
    );
    const tokens = getBuiltinAppearanceThemeTokens('openbitfun-dark');

    expect(tokens).toMatchObject({
      '--openbitfun-color-surface-scene': '#1c1c1f',
      '--openbitfun-color-surface-tertiary': '#0e0e10',
      '--openbitfun-color-action-quiet-hover': 'rgba(255, 255, 255, 0.06)',
      '--openbitfun-color-action-neutral-surface': 'rgba(255, 255, 255, 0.1)',
      '--openbitfun-component-config-page-row-hover-background': 'rgba(255, 255, 255, 0.1)',
    });

    expect(tokens['--openbitfun-component-config-page-row-hover-background'])
      .toBe(darkAppearance?.colors.element.base);
    expect(tokens['--openbitfun-component-config-page-row-hover-background'])
      .not.toBe(darkAppearance?.colors.element.soft);

    for (const appearance of builtinAppearancePalettes.filter(
      entry => entry.type === 'dark' && !entry.components?.configPage?.rowHover,
    )) {
      expect(
        getBuiltinAppearanceThemeTokens(appearance.id)[
          '--openbitfun-component-config-page-row-hover-background'
        ],
        appearance.id,
      ).toBe(appearance.colors.element.base);
    }
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
          "hash": "1fe3b2d75f26bcc2f5ba7a060798ccd4893ab0e6a243e4dbcbd1ea20b3774b05",
          "id": "openbitfun-light",
          "type": "light",
        },
        {
          "hash": "3a0a60eabad8363abaded31051a4e5a7aaeb5a22ee12fbd81f81c6f2e1ed44a9",
          "id": "openbitfun-monochrome",
          "type": "light",
        },
        {
          "hash": "af8895255a3a21481063077505cf70d60c13105fbc446d8c6860a86041bd1e47",
          "id": "openbitfun-slate",
          "type": "dark",
        },
        {
          "hash": "bc97870416ab67aa59fa362f75b4f5bf3397112ea3d7c3ed7ec340cd031d70cb",
          "id": "openbitfun-dark",
          "type": "dark",
        },
        {
          "hash": "326680e416018b02899682a3903b6a17e81099e50553386bf018f22d21c8a13e",
          "id": "openbitfun-midnight",
          "type": "dark",
        },
        {
          "hash": "8b4f1e1437d1105c0033ec2ac7a3191e318ca0329521933a2bbe7f9aad715345",
          "id": "openbitfun-china-style",
          "type": "light",
        },
        {
          "hash": "661f9bbd92c166cd3e18d931fa2072547223abbe2a39b03be81d3cdfea147b95",
          "id": "openbitfun-china-night",
          "type": "dark",
        },
        {
          "hash": "e06490cefa484ce9765e0ac199440f9d9ccedf05f3b5d35acd47ce3fd9a02257",
          "id": "openbitfun-cyber",
          "type": "dark",
        },
        {
          "hash": "e6647f3450dba47be1059bea47c06bbc6463461636079e85b73446ae9a8afcb4",
          "id": "openbitfun-tokyo-night",
          "type": "dark",
        },
      ]
    `);
  });
});

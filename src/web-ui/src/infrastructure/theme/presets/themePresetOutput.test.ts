import { createHash } from 'node:crypto';
import { describe, expect, it } from 'vitest';

import { builtinThemes } from './index';
import {
  PLUGIN_THEME_COLOR_KEYS,
  createPluginThemeColorProjection,
} from '../pluginThemeProjection';
import {
  createAccentScale,
  createGitColors,
  createSemanticColors,
  createSecondaryAccentScale,
  overlayBlack,
  overlayWhite,
  rgbFromHex,
  rgbaFromHex,
} from './shared';

function hashTheme(theme: unknown): string {
  return createHash('sha256')
    .update(JSON.stringify(theme))
    .digest('hex');
}

describe('builtin theme preset output', () => {
  it('formats hex palette references as stable rgb strings', () => {
    expect(rgbFromHex('#00e6ff')).toBe('rgb(0, 230, 255)');
    expect(rgbaFromHex('#00e6ff', 0.12)).toBe('rgba(0, 230, 255, 0.12)');
    expect(rgbaFromHex('#00e6ff', '0.12')).toBe('rgba(0, 230, 255, 0.12)');
    expect(overlayBlack(0.3)).toBe('rgba(0, 0, 0, 0.3)');
    expect(overlayWhite(0.08)).toBe('rgba(255, 255, 255, 0.08)');
  });

  it('aliases staged git colors to added colors unless a theme overrides them', () => {
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

  it('does not carry retired runtime-only authoring stops in builtin theme schemas', () => {
    for (const theme of builtinThemes) {
      expect(theme.colors.accent).not.toHaveProperty('800');
      expect(theme.colors.purple).not.toHaveProperty('50');
      expect(theme.colors.purple).not.toHaveProperty('400');
      expect(theme.colors.purple).not.toHaveProperty('800');
      expect(theme.typography.weight).not.toHaveProperty('bold');
    }
  });

  it('keeps near-neutral preset foregrounds on canonical stops', () => {
    const serializedThemes = JSON.stringify(builtinThemes).toLowerCase();

    expect(serializedThemes).not.toContain('#fafafa');
    expect(serializedThemes).not.toContain('#e2e6eb');
    expect(serializedThemes).not.toContain('#f0f2f5');
  });

  it('projects builtin themes to a compact OpenCode-compatible plugin color key set', () => {
    expect(PLUGIN_THEME_COLOR_KEYS).toEqual([
      'primary',
      'secondary',
      'accent',
      'success',
      'warning',
      'error',
      'info',
    ]);

    for (const theme of builtinThemes) {
      const projection = createPluginThemeColorProjection(theme);

      expect(Object.keys(projection).sort()).toEqual([...PLUGIN_THEME_COLOR_KEYS].sort());
      expect(projection.primary).toBe(theme.colors.accent[500]);
      expect(projection.secondary).toBe(theme.colors.purple?.[500] ?? theme.colors.accent[600]);
      expect(projection.accent).toBe(theme.colors.accent[600]);
      expect(projection.success).toBe(theme.colors.semantic.success);
      expect(projection.warning).toBe(theme.colors.semantic.warning);
      expect(projection.error).toBe(theme.colors.semantic.error);
      expect(projection.info).toBe(theme.colors.semantic.info);
    }
  });

  it('keeps resolved preset objects stable across helper refactors', () => {
    expect(builtinThemes.map(theme => ({
      id: theme.id,
      type: theme.type,
      hash: hashTheme(theme),
    }))).toMatchInlineSnapshot(`
      [
        {
          "hash": "07ac3c2391daeeb61276222879877407d2b0308daf1ae6fb992359ab4c760bfb",
          "id": "bitfun-light",
          "type": "light",
        },
        {
          "hash": "9bb6e963edc70e14da1e4b009ffaa4e8313fa58e46be104a2678425407135eef",
          "id": "bitfun-slate",
          "type": "dark",
        },
        {
          "hash": "4d9beb6796a2037b4e75d4bc77dd2bfc4e97703dc6ee445b9e795d413b05a6b7",
          "id": "bitfun-dark",
          "type": "dark",
        },
        {
          "hash": "d8b041347f3cb06996f27a011df5a661dc1d9ebb4c215ef75cbedafc0922135b",
          "id": "bitfun-midnight",
          "type": "dark",
        },
        {
          "hash": "0e453739640a89cdb0c7f78522ea8d928b3def3c2184190f61d4a0bcd0c711a1",
          "id": "bitfun-china-style",
          "type": "light",
        },
        {
          "hash": "9b2cf6146d238463e02985ed1fb5c45cb1be3ca0de4b0f2032af6d3526c6f9ac",
          "id": "bitfun-china-night",
          "type": "dark",
        },
        {
          "hash": "21cf925270c495469895638825cb8523bf66ec388048bab9be84b066779070c8",
          "id": "bitfun-cyber",
          "type": "dark",
        },
        {
          "hash": "b6d1c7a8138172ce6436dd2dae595147a8317d5cbd0697b39c8373211e02c8ac",
          "id": "bitfun-tokyo-night",
          "type": "dark",
        },
      ]
    `);
  });
});

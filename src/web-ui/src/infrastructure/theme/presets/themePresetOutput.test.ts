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
      800: 'rgba(59, 130, 246, 0.9)',
    });

    expect(createSecondaryAccentScale({
      base: '#8b5cf6',
      hover: '#7c3aed',
    })).toEqual({
      50: 'rgba(139, 92, 246, 0.04)',
      100: 'rgba(139, 92, 246, 0.08)',
      200: 'rgba(139, 92, 246, 0.15)',
      400: 'rgba(139, 92, 246, 0.4)',
      500: '#8b5cf6',
      600: '#7c3aed',
      800: 'rgba(124, 58, 237, 0.9)',
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
          "hash": "64235e8fca47462c9943d3015d024dbd9be8df90d25a5e7508f7e5b2ee789c6c",
          "id": "bitfun-light",
          "type": "light",
        },
        {
          "hash": "d6b190bdf145c29e28852d4922e0ee03cfe635ccc42081d82acd9ed75ba9e9c6",
          "id": "bitfun-slate",
          "type": "dark",
        },
        {
          "hash": "65b55d4d15503949e120e28fa1210449dfec390af69514410cea429f28bd19da",
          "id": "bitfun-dark",
          "type": "dark",
        },
        {
          "hash": "57d5efdaec49f1e4f21f3d095150dd81e810a0dedae4ae690d15b99e042228fa",
          "id": "bitfun-midnight",
          "type": "dark",
        },
        {
          "hash": "9fa44f208108fa973bdded43853ad1de449a2f235b3469350900d3afa3e8a50a",
          "id": "bitfun-china-style",
          "type": "light",
        },
        {
          "hash": "89461a6629abee37ae1d573b57bce7acf54b05101425c82b65d444573ef66722",
          "id": "bitfun-china-night",
          "type": "dark",
        },
        {
          "hash": "14f6897c94657f1e090107d8ff9d81d7540fad1fca0eaf1b068662b9fa5a518b",
          "id": "bitfun-cyber",
          "type": "dark",
        },
        {
          "hash": "80c5e00c474fd929154996d1fc8f0a230c50c5ccaaa966fbe588cf2349e2ede5",
          "id": "bitfun-tokyo-night",
          "type": "dark",
        },
      ]
    `);
  });
});

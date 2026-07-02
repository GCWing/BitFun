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
      changesBg: 'rgba(245, 158, 11, 0.1)',
      added: '#22c55e',
      addedBg: 'rgba(34, 197, 94, 0.1)',
      deleted: '#ef4444',
      deletedBg: 'rgba(239, 68, 68, 0.1)',
    })).toMatchObject({
      staged: '#22c55e',
      stagedBg: 'rgba(34, 197, 94, 0.1)',
    });

    expect(createGitColors({
      branch: '#64748b',
      branchBg: 'rgba(100, 116, 139, 0.1)',
      changes: '#f59e0b',
      changesBg: 'rgba(245, 158, 11, 0.1)',
      added: '#22c55e',
      addedBg: 'rgba(34, 197, 94, 0.1)',
      deleted: '#ef4444',
      deletedBg: 'rgba(239, 68, 68, 0.1)',
      staged: '#10b981',
      stagedBg: 'rgba(16, 185, 129, 0.1)',
    })).toMatchObject({
      staged: '#10b981',
      stagedBg: 'rgba(16, 185, 129, 0.1)',
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
          "hash": "f1c87b4cbe320d7f174a272aa08bd1df8baf02683d2275bac65c8ab4c46795bd",
          "id": "bitfun-light",
          "type": "light",
        },
        {
          "hash": "c27ab539f87e1c5e1072f6b4e8b74ad5088a461485f34965318b654156f1f728",
          "id": "bitfun-slate",
          "type": "dark",
        },
        {
          "hash": "644ebf466f1a722c329b7298b2fb2c40f7b352e338d0e6abf5ae22d1b233114f",
          "id": "bitfun-dark",
          "type": "dark",
        },
        {
          "hash": "737a002520289773d793a8af0f86d59951e79edf413fe406ae6a7f49437a9553",
          "id": "bitfun-midnight",
          "type": "dark",
        },
        {
          "hash": "701165cbf33a44d92547024f1c93c735e57a4479934ac82f913deccebc5c3c40",
          "id": "bitfun-china-style",
          "type": "light",
        },
        {
          "hash": "dd58088bd1558fcf17962e37cda63704c846a723d7408689c93142b78d12acf3",
          "id": "bitfun-china-night",
          "type": "dark",
        },
        {
          "hash": "683856e15d61742132227c99d1abb92ea08ffc29259e7b64d5e4389d4aa214ed",
          "id": "bitfun-cyber",
          "type": "dark",
        },
        {
          "hash": "d8777258e8c141a0225606d83ce931d8b35b4231bfa036155016e1766b78ed23",
          "id": "bitfun-tokyo-night",
          "type": "dark",
        },
      ]
    `);
  });
});

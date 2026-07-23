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
      expect(theme.colors.background).not.toHaveProperty('quaternary');
      expect(theme.colors.background).not.toHaveProperty('tooltip');
      expect(theme.colors.element).not.toHaveProperty('elevated');
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
          "hash": "18bde3b1b694643489ccf854d6a4ec2f92b6522925b1afae71c053c84fe018a8",
          "id": "bitfun-light",
          "type": "light",
        },
        {
          "hash": "7def888a159fe62da73f21717777cad2fd13a048853b9264cda683220e899677",
          "id": "bitfun-slate",
          "type": "dark",
        },
        {
          "hash": "c7a28e7fde81910bb796e18afabdb7b2840a5c0ae7a471b583990b43ce804921",
          "id": "bitfun-dark",
          "type": "dark",
        },
        {
          "hash": "b3447ec7218ad3f9bfe9749ca5ed567aee733f8555c86fb9dbca712294484b7c",
          "id": "bitfun-midnight",
          "type": "dark",
        },
        {
          "hash": "438f2ae26c4d1ebecbfa98e020d8e7d6559668fbf8e2c56b2dc6aa6bcadc3537",
          "id": "bitfun-china-style",
          "type": "light",
        },
        {
          "hash": "9caa3cc0deac7cf940ab550c79ea0a5d747f9496095af8ef78e4df1a64abf842",
          "id": "bitfun-china-night",
          "type": "dark",
        },
        {
          "hash": "6443493750d1b48805d6392fd17c11347f4f02af88943326522efee29330b417",
          "id": "bitfun-cyber",
          "type": "dark",
        },
        {
          "hash": "34e5b2c1ea244d28dffa9be172d3d48e65e82100b39125b21e3760b4316192d3",
          "id": "bitfun-tokyo-night",
          "type": "dark",
        },
      ]
    `);
  });
});

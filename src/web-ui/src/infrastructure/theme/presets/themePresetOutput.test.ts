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
          "hash": "63622f32216bbb5aea27e3ba0aec61f385a43842e57a6f17c814b341b8a54ac7",
          "id": "bitfun-light",
          "type": "light",
        },
        {
          "hash": "a62e5f3c6a5cbf4c7a35a2f51ed3a5f2a1eb82f6421f11e194ebb9a707517311",
          "id": "bitfun-slate",
          "type": "dark",
        },
        {
          "hash": "a527410c9bdf5ed396d82d843e19b298f8b5fe280ebc70bfe01237e753b28713",
          "id": "bitfun-dark",
          "type": "dark",
        },
        {
          "hash": "8b2f11e903371752749b5c7a44337df517dbb2ed9cf0fed29a2341ebb1890f0b",
          "id": "bitfun-midnight",
          "type": "dark",
        },
        {
          "hash": "d198bd1a20566689f5bf2660bc47b5aa2da854e5f3dcae9e655a5408b36141d1",
          "id": "bitfun-china-style",
          "type": "light",
        },
        {
          "hash": "f93427f5952e5a7fe5882cb114a6969e8fb7564292761029d6ee071233b2bdd5",
          "id": "bitfun-china-night",
          "type": "dark",
        },
        {
          "hash": "fffcb8a66e3a10745d155402be32b90df29e162f3b4a589862cccb9bafb215b7",
          "id": "bitfun-cyber",
          "type": "dark",
        },
        {
          "hash": "81ece301a307408b2149141c7b1553bf422245d20278e98442d6c36063acfc3b",
          "id": "bitfun-tokyo-night",
          "type": "dark",
        },
      ]
    `);
  });
});

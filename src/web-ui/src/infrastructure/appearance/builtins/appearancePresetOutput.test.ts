import { createHash } from 'node:crypto';
import { describe, expect, it } from 'vitest';

import { builtinAppearancePalettes } from './palettes';
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
      expect(appearance.typography.weight).not.toHaveProperty('bold');
    }
  });

  it('keeps near-neutral preset foregrounds on canonical stops', () => {
    const serializedAppearances = JSON.stringify(builtinAppearancePalettes).toLowerCase();

    expect(serializedAppearances).not.toContain('#fafafa');
    expect(serializedAppearances).not.toContain('#e2e6eb');
    expect(serializedAppearances).not.toContain('#f0f2f5');
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
          "hash": "b70ab1232690e55f3446d84c5c5d4459c508c6cd5f1e0bfe8565eba82e97ea6e",
          "id": "bitfun-light",
          "type": "light",
        },
        {
          "hash": "6c426dc8aacd8095fdf5a69ed80b4d42c0235cf72f61b20186fc14d5ebcb320b",
          "id": "bitfun-slate",
          "type": "dark",
        },
        {
          "hash": "db937836a31fb02d67f6035f1b3a036b8161f3833fdbe2634f37936ba7ef6357",
          "id": "bitfun-dark",
          "type": "dark",
        },
        {
          "hash": "e1455a17d5891da49f584ac1da6740a04db152afd6cdd571a9406f86811e6c8b",
          "id": "bitfun-midnight",
          "type": "dark",
        },
        {
          "hash": "7944cb574fd2b30cd4ed766e1b25e86eabd4c90b4a0e054489231a68de5cc424",
          "id": "bitfun-china-style",
          "type": "light",
        },
        {
          "hash": "f66e855842633612ca3aed2c30ff8df057dd0768eeb47e89e049efab8968f227",
          "id": "bitfun-china-night",
          "type": "dark",
        },
        {
          "hash": "b72278893e14c1b0cef86449facf7ba61b4f714f077c3923241ebb012a90e3c8",
          "id": "bitfun-cyber",
          "type": "dark",
        },
        {
          "hash": "abcd6c91fdc1c03ea66d7d460401351620d672130e4742cce6a8be7f04b22626",
          "id": "bitfun-tokyo-night",
          "type": "dark",
        },
      ]
    `);
  });
});

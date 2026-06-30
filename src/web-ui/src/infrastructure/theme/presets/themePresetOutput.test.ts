import { createHash } from 'node:crypto';
import { describe, expect, it } from 'vitest';

import { builtinThemes } from './index';
import { createGitColors, overlayBlack, overlayWhite, rgbFromHex, rgbaFromHex } from './shared';

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

  it('keeps near-neutral preset foregrounds on canonical stops', () => {
    const serializedThemes = JSON.stringify(builtinThemes).toLowerCase();

    expect(serializedThemes).not.toContain('#fafafa');
    expect(serializedThemes).not.toContain('#e2e6eb');
    expect(serializedThemes).not.toContain('#f0f2f5');
  });

  it('keeps resolved preset objects stable across helper refactors', () => {
    expect(builtinThemes.map(theme => ({
      id: theme.id,
      type: theme.type,
      hash: hashTheme(theme),
    }))).toMatchInlineSnapshot(`
      [
        {
          "hash": "c3b6a5647f5a098c777c5e0669578a2a6a83fe399c4ffe2b16e00b532361e0a5",
          "id": "bitfun-light",
          "type": "light",
        },
        {
          "hash": "f9732d9339162f704c6bd19dbac61e6ecf7819dd3d72a6437cce04812ab16db4",
          "id": "bitfun-slate",
          "type": "dark",
        },
        {
          "hash": "4d3b604eeed6cac6f06228c93b2771217dda6f5e6a1fd151bc48849f59c229c1",
          "id": "bitfun-dark",
          "type": "dark",
        },
        {
          "hash": "5b5429e48817fcd6d5643989f959340e6766ee5a0e3edfdff88f3984c0e343e8",
          "id": "bitfun-midnight",
          "type": "dark",
        },
        {
          "hash": "46ac5adf2b0dd0bc633f27665e1544893eb57617c123500b2d5b543690eca1f9",
          "id": "bitfun-china-style",
          "type": "light",
        },
        {
          "hash": "bc760598ce85d6e4a7f22349aace0e25ab6cb401535bdd468513a141a2069e82",
          "id": "bitfun-china-night",
          "type": "dark",
        },
        {
          "hash": "4b4597856d6c78a81c49a8f50c91d35682d8b53c27635b7097be1cb13a2c7a22",
          "id": "bitfun-cyber",
          "type": "dark",
        },
        {
          "hash": "1c391cd9207188d5edf906dabd3f23f28e07952c10f1ee9ebf30432747fc0fa0",
          "id": "bitfun-tokyo-night",
          "type": "dark",
        },
      ]
    `);
  });
});

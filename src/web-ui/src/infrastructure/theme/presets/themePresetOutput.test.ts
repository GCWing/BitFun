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
          "hash": "f1c87b4cbe320d7f174a272aa08bd1df8baf02683d2275bac65c8ab4c46795bd",
          "id": "bitfun-light",
          "type": "light",
        },
        {
          "hash": "e6310cfa9ee13263e955a4abaabbbba63713c1210ec981bc49c5b5f786c880e8",
          "id": "bitfun-slate",
          "type": "dark",
        },
        {
          "hash": "235f2c1d67c65426031fb5f6c7f47b37e9a29e102adb501194a6cb87dbbb6880",
          "id": "bitfun-dark",
          "type": "dark",
        },
        {
          "hash": "f0a96ff2f8dc2a63c37ab413e49ec0e9f7987f12bc510d36d7c52f9bfc7eb8bc",
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
          "hash": "101a1ee5c4a14ac03deca443f5496fac521b49ada31554a90eb1375738b2afe4",
          "id": "bitfun-cyber",
          "type": "dark",
        },
        {
          "hash": "c276008fcdbac289f0893cbcb3cdc4227e6dade9870b923d7b606f4eb8701873",
          "id": "bitfun-tokyo-night",
          "type": "dark",
        },
      ]
    `);
  });
});

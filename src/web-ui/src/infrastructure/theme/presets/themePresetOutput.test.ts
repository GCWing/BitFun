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
          "hash": "44a6f6daeceecfb0166c667b3252f5766f98de7811599079d58d485605ba5857",
          "id": "bitfun-light",
          "type": "light",
        },
        {
          "hash": "7bfb47bdfd3658b51c5c3a126aff9fafba00cf83a79d6ee148cfe3cecf9095d0",
          "id": "bitfun-slate",
          "type": "dark",
        },
        {
          "hash": "2033449df1da52308c856bc9fa0bb006ca9416545c7b3971ee0c1700fd3954d2",
          "id": "bitfun-dark",
          "type": "dark",
        },
        {
          "hash": "2b5f34aa379e97bbea554fa8b376d951c8927bd2d46a25e04cef39d16742ba8d",
          "id": "bitfun-midnight",
          "type": "dark",
        },
        {
          "hash": "e1b7b1ad6ef0bd0b9cb7bc5b020da2112a43367a989837d19856179a24da529d",
          "id": "bitfun-china-style",
          "type": "light",
        },
        {
          "hash": "ad731afd2ca6dd1cf75d286a14ffc82bc6cc074cdadff68e1daf382e3c445f4a",
          "id": "bitfun-china-night",
          "type": "dark",
        },
        {
          "hash": "b37c0cb6d539a703d1b6c0eca6c374d555ff74c0de0e2ec357bd56241a848fd4",
          "id": "bitfun-cyber",
          "type": "dark",
        },
        {
          "hash": "beae87b070e7b95463d50f25e715d7e8268a25aa43b8c8b6775de0ff728732c6",
          "id": "bitfun-tokyo-night",
          "type": "dark",
        },
      ]
    `);
  });
});

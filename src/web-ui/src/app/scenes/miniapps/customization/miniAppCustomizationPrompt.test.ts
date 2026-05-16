import { describe, expect, it } from 'vitest';
import { buildMiniAppCustomizationPrompt } from './miniAppCustomizationPrompt';

describe('buildMiniAppCustomizationPrompt', () => {
  it('includes draft root and active-app write warning', () => {
    const prompt = buildMiniAppCustomizationPrompt({
      appId: 'builtin-gomoku',
      appName: 'Gomoku',
      draftId: 'draft-1',
      draftRoot: 'C:/Users/me/AppData/Roaming/BitFun/miniapps/.drafts/builtin-gomoku/draft-1',
      userRequest: 'Make the board lighter',
    });

    expect(prompt).toContain('Draft root: C:/Users/me/AppData/Roaming/BitFun/miniapps/.drafts/builtin-gomoku/draft-1');
    expect(prompt).toContain('Edit only files under the draft root.');
    expect(prompt).toContain('Do not edit the active app directory.');
    expect(prompt).toContain('Make the board lighter');
  });
});

import { describe, expect, it } from 'vitest';
import {
  resolveFontSizeTokens,
  PRESET_UI_BASE_PX,
} from './index';

describe('resolveFontSizeTokens', () => {
  it('returns preset tokens for named levels', () => {
    const tokens = resolveFontSizeTokens({ level: 'default' });
    expect(tokens.base).toBe(`${PRESET_UI_BASE_PX.default}px`);
  });

  it('derives tokens for custom level', () => {
    const tokens = resolveFontSizeTokens({ level: 'custom', customPx: 16 });
    expect(tokens.base).toBe('16px');
  });

  it('falls back to 14px when custom has no customPx', () => {
    const tokens = resolveFontSizeTokens({ level: 'custom' });
    expect(tokens.base).toBe('14px');
  });
});

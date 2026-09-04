// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';
import {
  getTypographyTokenNumber,
  getTypographyTokenPx,
  getTypographyTokenValue,
  readActiveTypographyTokenPx,
} from './typographyRuntime';

describe('typography runtime adapter', () => {
  it('translates canonical design tokens for numeric renderer APIs', () => {
    expect(getTypographyTokenPx('font.size.base')).toBe(14);
    expect(getTypographyTokenNumber('font.weight.semibold')).toBe(600);
    expect(getTypographyTokenNumber('lineHeight.reading')).toBe(1.58);
    expect(getTypographyTokenValue('font.family.mono')).toContain('monospace');
  });

  it('reads active root font-size overrides before falling back to the canonical value', () => {
    document.documentElement.style.setProperty('--openbitfun-font-size-base', '17px');
    expect(readActiveTypographyTokenPx('font.size.base')).toBe(17);
    document.documentElement.style.removeProperty('--openbitfun-font-size-base');
  });
});

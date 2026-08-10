import { describe, expect, it } from 'vitest';

import {
  TEXT_STROKE_GRADIENT_COLORS,
  buildTextStrokeColorCycle,
} from './TextStrokeEffectGradient';
import { APPEARANCE_DOMAIN_TOKENS } from '@/infrastructure/appearance/appearanceDomainTokens';

const TEXT_STROKE_TOKEN_SEQUENCE = Array.from(
  { length: 5 },
  (_, index) => `var(--bf-appearance-token-domain-text-stroke-${index})`,
);

describe('TextStrokeEffect color cycles', () => {
  it('keeps gradient animation values closed over the appearance token sequence', () => {
    expect(APPEARANCE_DOMAIN_TOKENS.textStroke).toEqual(TEXT_STROKE_TOKEN_SEQUENCE);
    expect(TEXT_STROKE_GRADIENT_COLORS).toBe(APPEARANCE_DOMAIN_TOKENS.textStroke);

    const expectedCycle = [
      ...TEXT_STROKE_TOKEN_SEQUENCE.slice(2),
      ...TEXT_STROKE_TOKEN_SEQUENCE.slice(0, 2),
      TEXT_STROKE_TOKEN_SEQUENCE[2],
    ].join('; ');
    expect(buildTextStrokeColorCycle(2)).toBe(expectedCycle);
  });
});

import { describe, expect, it } from 'vitest';

import {
  TEXT_STROKE_GRADIENT_COLORS,
  buildTextStrokeColorCycle,
} from './TextStrokeEffectGradient';

describe('TextStrokeEffect color cycles', () => {
  it('keeps gradient animation values closed over the original visual color sequence', () => {
    expect(TEXT_STROKE_GRADIENT_COLORS).toEqual([
      '#eab308',
      '#ef4444',
      '#3b82f6',
      '#06b6d4',
      '#8b5cf6',
    ]);

    expect(buildTextStrokeColorCycle(2)).toBe(
      '#3b82f6; #06b6d4; #8b5cf6; #eab308; #ef4444; #3b82f6',
    );
  });
});

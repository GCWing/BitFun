import { describe, expect, it } from 'vitest';

import {
  TEXT_STROKE_GRADIENT_COLORS,
  buildTextStrokeColorCycle,
} from './TextStrokeEffectGradient';
import { UI_EXCEPTION_ACCENTS } from '@/shared/theme/uiExceptionAccents';

const MIGRATED_TEXT_STROKE_VISUAL_SEQUENCE = [
  '#eab308',
  '#ef4444',
  '#3b82f6',
  '#06b6d4',
  '#8b5cf6',
] as const;

describe('TextStrokeEffect color cycles', () => {
  it('keeps gradient animation values closed over the original visual color sequence', () => {
    expect(UI_EXCEPTION_ACCENTS.textStroke).toEqual(MIGRATED_TEXT_STROKE_VISUAL_SEQUENCE);
    expect(TEXT_STROKE_GRADIENT_COLORS).toBe(UI_EXCEPTION_ACCENTS.textStroke);

    const expectedCycle = [
      ...MIGRATED_TEXT_STROKE_VISUAL_SEQUENCE.slice(2),
      ...MIGRATED_TEXT_STROKE_VISUAL_SEQUENCE.slice(0, 2),
      MIGRATED_TEXT_STROKE_VISUAL_SEQUENCE[2],
    ].join('; ');
    expect(buildTextStrokeColorCycle(2)).toBe(expectedCycle);
  });
});

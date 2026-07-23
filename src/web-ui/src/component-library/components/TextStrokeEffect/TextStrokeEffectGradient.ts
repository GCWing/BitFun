import { UI_EXCEPTION_ACCENTS } from '@/shared/theme/uiExceptionAccents';

export const TEXT_STROKE_GRADIENT_COLORS = UI_EXCEPTION_ACCENTS.textStroke;

export const TEXT_STROKE_GRADIENT_OFFSETS = ['0%', '25%', '50%', '75%', '100%'] as const;

export function buildTextStrokeColorCycle(startIndex: number): string {
  const colors = Array.from(TEXT_STROKE_GRADIENT_COLORS);
  const cycle = [
    ...colors.slice(startIndex),
    ...colors.slice(0, startIndex),
    colors[startIndex],
  ];
  return cycle.join('; ');
}

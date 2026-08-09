import { APPEARANCE_DOMAIN_TOKENS } from '@/infrastructure/appearance/appearanceDomainTokens';

export const TEXT_STROKE_GRADIENT_COLORS = APPEARANCE_DOMAIN_TOKENS.textStroke;

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

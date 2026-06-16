export const TEXT_STROKE_GRADIENT_COLORS = [
  '#eab308',
  '#ef4444',
  '#3b82f6',
  '#06b6d4',
  '#8b5cf6',
] as const;

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

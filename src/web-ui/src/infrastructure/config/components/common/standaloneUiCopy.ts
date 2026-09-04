import type { ReactNode } from 'react';

/**
 * Settings subtitles and descriptions are standalone UI fragments rather than
 * prose paragraphs. Keep their source copy reusable while presenting them
 * without a terminal sentence period. Ellipses remain intact.
 */
export function formatStandaloneUiText(value: string): string {
  const trimmed = value.trimEnd();
  if (trimmed.endsWith('。') || trimmed.endsWith('．')) {
    return trimmed.slice(0, -1);
  }
  if (trimmed.endsWith('.') && !trimmed.endsWith('..')) {
    return trimmed.slice(0, -1);
  }
  return trimmed;
}

export function formatStandaloneUiCopy(value: ReactNode): ReactNode {
  return typeof value === 'string' ? formatStandaloneUiText(value) : value;
}

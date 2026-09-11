import type { AppearanceThemeTokenName } from '../types';

export function withLegacyIndicatorTokens(
  tokens: Partial<Record<AppearanceThemeTokenName, string>> | undefined,
): Partial<Record<AppearanceThemeTokenName, string>> {
  const result = { ...tokens };
  for (const [next, previous] of [
    ['--openbitfun-color-number-badge-background', '--openbitfun-color-action-neutral-surface'],
    ['--openbitfun-color-key-hint-content', '--openbitfun-color-content-muted'],
  ] as const) {
    if (result[next] === undefined && tokens?.[previous] !== undefined) result[next] = tokens[previous];
  }
  return result;
}

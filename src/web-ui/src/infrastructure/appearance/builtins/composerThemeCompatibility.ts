import type { AppearanceThemeTokenName } from '../types';

export function withLegacyComposerTokens(
  tokens: Partial<Record<AppearanceThemeTokenName, string>> | undefined,
): Partial<Record<AppearanceThemeTokenName, string>> {
  const result = { ...tokens };
  for (const [next, previous] of [
    ['--openbitfun-color-composer-border', '--openbitfun-color-field-border'],
    ['--openbitfun-color-composer-context-background', '--openbitfun-color-surface-subtle'],
  ] as const) {
    if (result[next] === undefined && tokens?.[previous] !== undefined) result[next] = tokens[previous];
  }
  return result;
}

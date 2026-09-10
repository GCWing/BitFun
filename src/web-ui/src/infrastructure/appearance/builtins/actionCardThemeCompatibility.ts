import type { AppearanceThemeTokenName } from '../types';

export function withLegacyActionCardToken(
  tokens: Partial<Record<AppearanceThemeTokenName, string>> | undefined,
): Partial<Record<AppearanceThemeTokenName, string>> {
  const result = { ...tokens };
  if (result['--openbitfun-color-action-card-background'] === undefined
    && tokens?.['--openbitfun-color-action-neutral-surface'] !== undefined) {
    result['--openbitfun-color-action-card-background'] = tokens['--openbitfun-color-action-neutral-surface'];
  }
  return result;
}

import type { AppearanceThemeTokenName } from '../types';

export function withLegacyCaptionToken(
  tokens: Partial<Record<AppearanceThemeTokenName, string>> | undefined,
): Partial<Record<AppearanceThemeTokenName, string>> {
  const result = { ...tokens };
  if (result['--openbitfun-color-content-caption'] === undefined
    && tokens?.['--openbitfun-color-content-muted'] !== undefined) {
    result['--openbitfun-color-content-caption'] = tokens['--openbitfun-color-content-muted'];
  }
  return result;
}

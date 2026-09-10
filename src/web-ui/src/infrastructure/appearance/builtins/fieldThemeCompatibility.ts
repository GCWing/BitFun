import type { AppearanceThemeTokenName } from '../types';

// Old packages used content.muted for hints and borderFocus for editing. Explicit field colors
// take precedence; missing values continue to come from the selected base theme.
export function withLegacyFieldTokens(
  tokens: Partial<Record<AppearanceThemeTokenName, string>> | undefined,
): Partial<Record<AppearanceThemeTokenName, string>> {
  const result = { ...tokens };
  const legacy = tokens?.['--openbitfun-color-content-muted'];
  if (result['--openbitfun-color-field-placeholder'] === undefined && legacy !== undefined) {
    result['--openbitfun-color-field-placeholder'] = legacy;
  }
  const legacyFocus = tokens?.['--openbitfun-color-field-border-focus'];
  if (result['--openbitfun-color-field-border-active'] === undefined && legacyFocus !== undefined) {
    result['--openbitfun-color-field-border-active'] = legacyFocus;
  }
  const legacyGroup = tokens?.['--openbitfun-color-surface-tertiary'];
  if (result['--openbitfun-color-field-group-background'] === undefined && legacyGroup !== undefined) {
    result['--openbitfun-color-field-group-background'] = legacyGroup;
  }
  return result;
}

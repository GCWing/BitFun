import type { AppearanceThemeTokenName } from '../types';

// Existing Appearance packages may customize Button through shared action tokens.
// Only inherit explicitly supplied values; a new component override always wins.
const legacyButtonTokens = {
  'content': 'color-action-neutral-content',
  'outline-border': 'color-action-neutral-border',
  'outline-border-interactive': 'color-action-neutral-border',
  'fill-background': 'color-action-neutral-surface',
  'fill-background-hover': 'color-action-neutral-surface-hover',
  'fill-background-pressed': 'color-action-neutral-surface-pressed',
  'primary-background': 'color-action-primary-background',
  'primary-background-hover': 'color-action-primary-hover',
  'primary-background-pressed': 'color-action-primary-pressed',
  'primary-content-disabled': 'color-action-neutral-content-disabled',
  'text-content': 'color-accent-default',
  'text-content-hover': 'color-accent-hover',
  'text-content-disabled': 'color-accent-disabled',
} as const;

export function withLegacyButtonTokens(
  tokens: Partial<Record<AppearanceThemeTokenName, string>> | undefined,
): Partial<Record<AppearanceThemeTokenName, string>> {
  const result = { ...tokens };
  for (const [suffix, legacySuffix] of Object.entries(legacyButtonTokens)) {
    const name = `--openbitfun-component-button-${suffix}` as AppearanceThemeTokenName;
    const legacyName = `--openbitfun-${legacySuffix}` as AppearanceThemeTokenName;
    if (result[name] === undefined && tokens?.[legacyName] !== undefined) {
      result[name] = tokens[legacyName];
    }
  }
  return result;
}

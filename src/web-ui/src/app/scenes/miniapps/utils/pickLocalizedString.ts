/**
 * Pick the best-matching localized string for a MiniApp meta field.
 *
 * Resolution order:
 *   1. `meta.i18n.locales[currentLanguage].<field>` (exact match)
 *   2. Chinese variants fall back to `zh-CN`
 *   3. `meta.i18n.locales['en-US'].<field>`         (universal fallback)
 *   4. The top-level `meta.<field>` value           (author default / legacy apps)
 *
 * Apps without an `i18n` block (legacy or user-authored) keep working transparently.
 */

import type { MiniAppMeta, MiniAppLocaleStrings } from '@/infrastructure/api/service-api/MiniAppAPI';
import { getLocaleFallbackChain } from '@/infrastructure/i18n/presets';

function getFallbackChain(currentLanguage: string): string[] {
  return getLocaleFallbackChain(currentLanguage);
}

type LocalizableStringField = 'name' | 'description';

export function pickLocalizedString(
  meta: Pick<MiniAppMeta, LocalizableStringField | 'i18n'>,
  currentLanguage: string,
  field: LocalizableStringField,
): string {
  const fromCurrent = meta.i18n?.locales?.[currentLanguage]?.[field];
  if (fromCurrent) return fromCurrent;

  for (const fallbackLang of getFallbackChain(currentLanguage)) {
    if (fallbackLang === currentLanguage) continue;
    const v = meta.i18n?.locales?.[fallbackLang]?.[field];
    if (v) return v;
  }

  return meta[field] ?? '';
}

export function pickLocalizedTags(
  meta: Pick<MiniAppMeta, 'tags' | 'i18n'>,
  currentLanguage: string,
): string[] {
  const fromCurrent = meta.i18n?.locales?.[currentLanguage]?.tags;
  if (fromCurrent && fromCurrent.length) return fromCurrent;

  for (const fallbackLang of getFallbackChain(currentLanguage)) {
    if (fallbackLang === currentLanguage) continue;
    const v = meta.i18n?.locales?.[fallbackLang]?.tags;
    if (v && v.length) return v;
  }

  return meta.tags ?? [];
}

/** Convenience: project all localizable fields at once. */
export function pickLocalizedMeta(
  meta: MiniAppMeta,
  currentLanguage: string,
): MiniAppLocaleStrings & { name: string; description: string; tags: string[] } {
  return {
    name: pickLocalizedString(meta, currentLanguage, 'name'),
    description: pickLocalizedString(meta, currentLanguage, 'description'),
    tags: pickLocalizedTags(meta, currentLanguage),
  };
}

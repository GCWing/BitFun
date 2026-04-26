/**
 * Single Web UI registry for selectable locales.
 *
 * To add a locale, add one metadata entry here and provide the matching
 * `src/web-ui/src/locales/<locale-id>/*.json` files. The i18n audit checks
 * that the registry and locale folders stay in sync.
 */
export const LOCALE_IDS = ['zh-CN', 'en-US', 'zh-TW'] as const;
export type LocaleId = (typeof LOCALE_IDS)[number];

export const builtinLocales = [
  {
    id: 'zh-CN',
    name: '简体中文',
    englishName: 'Simplified Chinese',
    nativeName: '简体中文',
    shortName: '中',
    rtl: false,
    dateFormat: 'YYYY年MM月DD日',
    numberFormat: {
      decimal: '.',
      thousands: ',',
    },
    aliases: ['zh', 'zh-Hans', 'zh-CN'],
    contentFallbacks: ['en-US'],
    builtin: true,
  },
  {
    id: 'en-US',
    name: 'English',
    englishName: 'English (US)',
    nativeName: 'English',
    shortName: 'EN',
    rtl: false,
    dateFormat: 'MM/DD/YYYY',
    numberFormat: {
      decimal: '.',
      thousands: ',',
    },
    aliases: ['en', 'en-US'],
    contentFallbacks: ['zh-CN'],
    builtin: true,
  },
  {
    id: 'zh-TW',
    name: '繁體中文',
    englishName: 'Traditional Chinese',
    nativeName: '繁體中文',
    shortName: '繁',
    rtl: false,
    dateFormat: 'YYYY年MM月DD日',
    numberFormat: {
      decimal: '.',
      thousands: ',',
    },
    aliases: ['zh-TW', 'zh-Hant', 'zh-HK', 'zh-MO'],
    contentFallbacks: ['zh-CN', 'en-US'],
    builtin: true,
  },
] satisfies LocaleMetadata[];

const localeAliasesByPriority = builtinLocales
  .flatMap(locale => locale.aliases.map(alias => ({ locale, alias: alias.toLowerCase() })))
  .sort((a, b) => b.alias.length - a.alias.length);

export interface LocaleMetadata {
  id: LocaleId;
  name: string;
  englishName: string;
  nativeName: string;
  shortName: string;
  rtl: boolean;
  dateFormat: string;
  numberFormat: {
    decimal: string;
    thousands: string;
  };
  aliases: readonly string[];
  contentFallbacks: readonly LocaleId[];
  builtin: boolean;
}

export function getLocaleMetadata(localeId: LocaleId): LocaleMetadata | undefined {
  return builtinLocales.find(locale => locale.id === localeId);
}

export function resolveLocaleId(value: string | null | undefined): LocaleId | null {
  const normalized = value?.trim().toLowerCase();
  if (!normalized) return null;

  const exact = builtinLocales.find(locale => locale.id.toLowerCase() === normalized);
  if (exact) return exact.id;

  // Prefer the longest alias so `zh-Hant-TW` resolves to `zh-TW` instead of
  // being captured by the broad `zh` alias. Resolve from a pre-sorted list so
  // repeated locale checks do not rebuild and sort alias metadata every time.
  return localeAliasesByPriority
    .find(({ alias }) => normalized === alias || normalized.startsWith(`${alias}-`))
    ?.locale.id ?? null;
}

export function isLocaleSupported(localeId: string): localeId is LocaleId {
  return resolveLocaleId(localeId) === localeId;
}

export function getSupportedLocaleIds(): LocaleId[] {
  return builtinLocales.map(locale => locale.id);
}

export function getLocaleFallbackChain(localeId: string, includeSelf = false): LocaleId[] {
  const resolved = resolveLocaleId(localeId);
  // Keep the chain registry-driven so adding a locale only updates metadata,
  // not every downstream caller that needs content fallback behavior.
  const chain: LocaleId[] = resolved
    ? [
      ...(includeSelf ? [resolved] : []),
      ...(getLocaleMetadata(resolved)?.contentFallbacks ?? []),
    ]
    : ['en-US', 'zh-CN'];

  return Array.from(new Set(chain));
}

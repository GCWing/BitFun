export const MOBILE_LOCALES = [
  {
    id: 'zh-CN',
    shortName: '中',
    aliases: ['zh', 'zh-Hans', 'zh-CN'],
  },
  {
    id: 'zh-TW',
    shortName: '繁',
    aliases: ['zh-TW', 'zh-Hant', 'zh-HK', 'zh-MO'],
  },
  {
    id: 'en-US',
    shortName: 'EN',
    aliases: ['en', 'en-US'],
  },
] as const;

const mobileLocaleAliasesByPriority = MOBILE_LOCALES
  .flatMap(locale => locale.aliases.map(alias => ({ locale, alias: alias.toLowerCase() })))
  .sort((a, b) => b.alias.length - a.alias.length);

export type MobileLanguage = (typeof MOBILE_LOCALES)[number]['id'];

export const DEFAULT_LANGUAGE = 'en-US' satisfies MobileLanguage;

export function isMobileLanguage(value: string | null | undefined): value is MobileLanguage {
  return MOBILE_LOCALES.some(locale => locale.id === value);
}

export function resolveMobileLanguage(value: string | null | undefined): MobileLanguage | null {
  const normalized = value?.trim().toLowerCase();
  if (!normalized) return null;

  const exact = MOBILE_LOCALES.find(locale => locale.id.toLowerCase() === normalized);
  if (exact) return exact.id;

  // Match the longest alias first so `zh-Hant-*` stays mapped to `zh-TW`.
  return mobileLocaleAliasesByPriority
    .find(({ alias }) => normalized === alias || normalized.startsWith(`${alias}-`))
    ?.locale.id ?? null;
}

export function getNextMobileLanguage(language: MobileLanguage): MobileLanguage {
  const currentIndex = MOBILE_LOCALES.findIndex(locale => locale.id === language);
  return MOBILE_LOCALES[(currentIndex + 1) % MOBILE_LOCALES.length].id;
}

export function getMobileLanguageShortName(language: MobileLanguage): string {
  return MOBILE_LOCALES.find(locale => locale.id === language)?.shortName ?? language;
}

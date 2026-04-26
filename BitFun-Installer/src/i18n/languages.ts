import en from './locales/en.json';
import zh from './locales/zh.json';
import zhTW from './locales/zh-TW.json';

export const INSTALLER_LANGUAGES = [
  {
    uiCode: 'en',
    appCode: 'en-US',
    label: 'English',
    nativeName: 'English',
    continueLabel: 'Continue',
    aliases: ['en', 'en-US'],
    resource: en,
  },
  {
    uiCode: 'zh',
    appCode: 'zh-CN',
    label: 'Chinese',
    nativeName: '简体中文',
    continueLabel: '继续',
    aliases: ['zh', 'zh-Hans', 'zh-CN'],
    resource: zh,
  },
  {
    uiCode: 'zh-TW',
    appCode: 'zh-TW',
    label: 'Traditional Chinese',
    nativeName: '繁體中文',
    continueLabel: '繼續',
    aliases: ['zh-TW', 'zh-Hant', 'zh-HK', 'zh-MO'],
    resource: zhTW,
  },
] as const;

const installerAliasesByPriority = INSTALLER_LANGUAGES
  .flatMap(language => language.aliases.map(alias => ({ language, alias: alias.toLowerCase() })))
  .sort((a, b) => b.alias.length - a.alias.length);

export type InstallerUiLanguage = (typeof INSTALLER_LANGUAGES)[number]['uiCode'];
export type AppLanguage = (typeof INSTALLER_LANGUAGES)[number]['appCode'];

export const installerResources = Object.fromEntries(
  INSTALLER_LANGUAGES.map(language => [
    language.uiCode,
    { translation: language.resource },
  ]),
);

export function isInstallerUiLanguage(value: string | null | undefined): value is InstallerUiLanguage {
  return INSTALLER_LANGUAGES.some(language => language.uiCode === value);
}

export function mapUiLanguageToAppLanguage(uiLanguage: InstallerUiLanguage): AppLanguage {
  return INSTALLER_LANGUAGES.find(language => language.uiCode === uiLanguage)?.appCode ?? 'en-US';
}

export function mapAppLanguageToUiLanguage(appLanguage: string | null | undefined): InstallerUiLanguage | null {
  return resolveInstallerUiLanguage(appLanguage);
}

export function resolveInstallerUiLanguage(value: string | null | undefined): InstallerUiLanguage | null {
  const normalized = value?.trim().toLowerCase();
  if (!normalized) return null;

  const exact = INSTALLER_LANGUAGES.find(language => language.uiCode.toLowerCase() === normalized);
  if (exact) return exact.uiCode;

  // Keep alias resolution deterministic when both broad and script-specific
  // Chinese aliases are present, and reuse the same priority list for browser
  // detection and app-language canonicalization.
  return installerAliasesByPriority
    .find(({ alias }) => normalized === alias || normalized.startsWith(`${alias}-`))
    ?.language.uiCode ?? null;
}

export function detectInstallerUiLanguage(appLanguage?: string | null): InstallerUiLanguage {
  return mapAppLanguageToUiLanguage(appLanguage)
    ?? resolveInstallerUiLanguage(typeof navigator !== 'undefined' ? navigator.language : null)
    ?? 'en';
}

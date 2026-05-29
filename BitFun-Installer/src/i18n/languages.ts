import {
  DEFAULT_INSTALLER_UI_LANGUAGE,
  INSTALLER_LANGUAGE_DEFINITIONS,
  SHARED_TERMS_BY_APP_LANGUAGE,
  type AppLanguage,
  type InstallerUiLanguage,
} from './generatedLocaleContract';
import en from './locales/en.json';
import zh from './locales/zh.json';
import zhTW from './locales/zh-TW.json';

const installerResourceByUiCode = {
  en,
  zh,
  'zh-TW': zhTW,
} satisfies Record<InstallerUiLanguage, Record<string, unknown>>;

export const INSTALLER_LANGUAGES = INSTALLER_LANGUAGE_DEFINITIONS.map(language => ({
  ...language,
  resource: {
    ...installerResourceByUiCode[language.uiCode],
    shared: SHARED_TERMS_BY_APP_LANGUAGE[language.appCode],
  },
}));

const installerAliasesByPriority = INSTALLER_LANGUAGES
  .flatMap(language => language.aliases.map(alias => ({ language, alias: alias.toLowerCase() })))
  .sort((a, b) => b.alias.length - a.alias.length);

export type { AppLanguage, InstallerUiLanguage };
export { DEFAULT_INSTALLER_UI_LANGUAGE };

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
  const normalized = appLanguage?.trim();
  if (!normalized) return null;

  const exact = INSTALLER_LANGUAGES.find(language => language.appCode === normalized);
  return exact?.uiCode ?? resolveInstallerUiLanguage(normalized);
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

export function getInstallerUiFallbackChain(
  uiLanguage: string | null | undefined,
  includeSelf = false,
): InstallerUiLanguage[] {
  const resolvedUiLanguage = resolveInstallerUiLanguage(uiLanguage);
  const language = INSTALLER_LANGUAGES.find(item => item.uiCode === resolvedUiLanguage);
  if (!language) {
    return [DEFAULT_INSTALLER_UI_LANGUAGE];
  }

  const chain: InstallerUiLanguage[] = [];
  if (includeSelf) {
    chain.push(language.uiCode);
  }
  for (const appLanguage of language.contentFallbacks) {
    const uiLanguage = mapAppLanguageToUiLanguage(appLanguage);
    if (uiLanguage) {
      chain.push(uiLanguage);
    }
  }

  return Array.from(new Set(chain));
}

export function detectInstallerUiLanguage(appLanguage?: string | null): InstallerUiLanguage {
  return mapAppLanguageToUiLanguage(appLanguage)
    ?? resolveInstallerUiLanguage(typeof navigator !== 'undefined' ? navigator.language : null)
    ?? DEFAULT_INSTALLER_UI_LANGUAGE;
}

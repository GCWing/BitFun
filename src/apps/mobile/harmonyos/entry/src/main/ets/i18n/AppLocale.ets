export const APP_LOCALE_ZH_CN: string = 'zh-CN';
export const APP_LOCALE_EN_US: string = 'en-US';

export interface AppLocaleDescriptor {
  readonly id: string;
  readonly nativeNameKey: string;
  readonly aliases: string[];
  readonly contentFallbacks: string[];
}

const LOCALES: AppLocaleDescriptor[] = [
  {
    id: APP_LOCALE_ZH_CN,
    nativeNameKey: 'settings.language.zhCN',
    aliases: ['zh', 'zh-hans', 'zh-cn', 'zh_cn', 'zh-hans-cn'],
    contentFallbacks: [APP_LOCALE_EN_US]
  },
  {
    id: APP_LOCALE_EN_US,
    nativeNameKey: 'settings.language.enUS',
    aliases: ['en', 'en-us', 'en_us', 'en-gb', 'en-au', 'en-ca'],
    contentFallbacks: [APP_LOCALE_ZH_CN]
  }
];

export class AppLocale {
  static readonly DEFAULT: string = APP_LOCALE_ZH_CN;

  static supported(): string[] {
    return LOCALES.map((locale: AppLocaleDescriptor): string => locale.id);
  }

  static descriptors(): AppLocaleDescriptor[] {
    return LOCALES.slice();
  }

  static isSupported(value: string | undefined | null): boolean {
    return AppLocale.resolve(value).length > 0;
  }

  static initial(stored: string | undefined | null, systemLanguage: string | undefined | null): string {
    return AppLocale.resolve(stored) || AppLocale.resolve(systemLanguage) || AppLocale.DEFAULT;
  }

  static resolve(value: string | undefined | null): string {
    const normalized = (value || '').trim().toLowerCase().split('_').join('-');
    if (normalized.length === 0) {
      return '';
    }
    for (let index = 0; index < LOCALES.length; index += 1) {
      const locale = LOCALES[index];
      if (locale.id.toLowerCase() === normalized || locale.aliases.indexOf(normalized) >= 0) {
        return locale.id;
      }
    }
    const language = normalized.split('-')[0];
    for (let index = 0; index < LOCALES.length; index += 1) {
      const locale = LOCALES[index];
      if (locale.aliases.indexOf(language) >= 0) {
        return locale.id;
      }
    }
    return '';
  }

  static fallbackChain(language: string, includeSelf: boolean = true): string[] {
    const resolved = AppLocale.resolve(language) || AppLocale.DEFAULT;
    const locale = LOCALES.find((item: AppLocaleDescriptor): boolean => item.id === resolved);
    const chain: string[] = [];
    if (includeSelf) {
      chain.push(resolved);
    }
    if (locale) {
      for (let index = 0; index < locale.contentFallbacks.length; index += 1) {
        const fallback = locale.contentFallbacks[index];
        if (chain.indexOf(fallback) < 0) {
          chain.push(fallback);
        }
      }
    }
    if (chain.indexOf(AppLocale.DEFAULT) < 0) {
      chain.push(AppLocale.DEFAULT);
    }
    return chain;
  }

  static next(language: string): string {
    const supported = AppLocale.supported();
    const currentIndex = supported.indexOf(AppLocale.resolve(language) || AppLocale.DEFAULT);
    return supported[(currentIndex + 1) % supported.length];
  }

  static nativeNameKey(language: string): string {
    const resolved = AppLocale.resolve(language) || AppLocale.DEFAULT;
    const locale = LOCALES.find((item: AppLocaleDescriptor): boolean => item.id === resolved);
    return locale ? locale.nativeNameKey : 'settings.language.zhCN';
  }

  static speechLanguage(language: string): string {
    return AppLocale.resolve(language) || AppLocale.DEFAULT;
  }

  static speechLocate(language: string): string {
    return (AppLocale.resolve(language) || AppLocale.DEFAULT) === APP_LOCALE_EN_US ? 'US' : 'CN';
  }

  static systemLanguageTag(language: string): string {
    return (AppLocale.resolve(language) || AppLocale.DEFAULT) === APP_LOCALE_EN_US ? 'en-US' : 'zh-Hans';
  }
}

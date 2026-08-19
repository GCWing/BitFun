import { AppLocale, APP_LOCALE_EN_US, APP_LOCALE_ZH_CN } from './AppLocale';
import { EN_US_MESSAGES } from './EnUsMessages';
import { ZH_CN_MESSAGES } from './ZhCnMessages';

@ObservedV2
export class AppLocaleState {
  @Trace language: string = AppLocale.DEFAULT;
  @Trace revision: number = 0;
}

export class RemoteI18n {
  static readonly state: AppLocaleState = new AppLocaleState();
  private static readonly catalogs: Map<string, Map<string, string>> = RemoteI18n.buildCatalogs();

  static language(): string {
    return RemoteI18n.state.language;
  }

  static revision(): number {
    return RemoteI18n.state.revision;
  }

  static setLanguage(language: string): string {
    const resolved = AppLocale.resolve(language) || AppLocale.DEFAULT;
    if (RemoteI18n.state.language !== resolved) {
      RemoteI18n.state.language = resolved;
      RemoteI18n.state.revision += 1;
    }
    return resolved;
  }

  static t(key: string): string {
    return RemoteI18n.lookup(RemoteI18n.state.language, key);
  }

  static f(key: string, value: string): string {
    return RemoteI18n.t(key).replace('{0}', value);
  }

  static f2(key: string, first: string, second: string): string {
    return RemoteI18n.t(key).replace('{0}', first).replace('{1}', second);
  }

  static translate(language: string, key: string): string {
    return RemoteI18n.lookup(language, key);
  }

  static textEquals(key: string, message: string): boolean {
    const supported = AppLocale.supported();
    for (let index = 0; index < supported.length; index += 1) {
      if (RemoteI18n.lookup(supported[index], key) === message) {
        return true;
      }
    }
    return false;
  }

  static textContains(key: string, message: string): boolean {
    const supported = AppLocale.supported();
    for (let index = 0; index < supported.length; index += 1) {
      const value = RemoteI18n.lookup(supported[index], key);
      if (value.length > 0 && message.indexOf(value) >= 0) {
        return true;
      }
    }
    return false;
  }

  static textHasTemplatePrefix(key: string, message: string): boolean {
    const supported = AppLocale.supported();
    for (let index = 0; index < supported.length; index += 1) {
      const prefix = RemoteI18n.lookup(supported[index], key).split('{0}')[0];
      if (prefix.length > 0 && message.indexOf(prefix) >= 0) {
        return true;
      }
    }
    return false;
  }

  static keys(language: string = AppLocale.DEFAULT): string[] {
    const catalog = RemoteI18n.catalogs.get(AppLocale.resolve(language) || AppLocale.DEFAULT);
    if (!catalog) {
      return [];
    }
    const keys: string[] = [];
    catalog.forEach((_value: string, key: string) => {
      keys.push(key);
    });
    return keys;
  }

  static formatAbsoluteDate(timestamp: number): string {
    const date = new Date(timestamp);
    const year = `${date.getFullYear()}`;
    const month = `${date.getMonth() + 1}`.padStart(2, '0');
    const day = `${date.getDate()}`.padStart(2, '0');
    if (RemoteI18n.language() === APP_LOCALE_EN_US) {
      return `${month}/${day}/${year}`;
    }
    return `${year}年${month}月${day}日`;
  }

  static speechLanguage(): string {
    return AppLocale.speechLanguage(RemoteI18n.language());
  }

  static speechLocate(): string {
    return AppLocale.speechLocate(RemoteI18n.language());
  }

  static nativeName(language: string = RemoteI18n.language()): string {
    return RemoteI18n.t(AppLocale.nativeNameKey(language));
  }

  static retranslateKnown(message: string, keys: string[]): string {
    for (let index = 0; index < keys.length; index += 1) {
      if (RemoteI18n.textEquals(keys[index], message)) {
        return RemoteI18n.t(keys[index]);
      }
    }
    return RemoteI18n.retranslate(message);
  }

  static retranslate(message: string): string {
    if (message.length === 0) {
      return message;
    }
    const keys = RemoteI18n.keys(AppLocale.DEFAULT);
    for (let index = 0; index < keys.length; index += 1) {
      const key = keys[index];
      const matchedLocale = RemoteI18n.uniqueLocaleForValue(key, message);
      if (matchedLocale.length > 0) {
        return RemoteI18n.t(key);
      }
      if (RemoteI18n.hasUniqueTemplate(key)) {
        const templated = RemoteI18n.retranslateTemplate(key, message);
        if (templated.length > 0) {
          return templated;
        }
      }
    }
    return message;
  }

  private static uniqueLocaleForValue(key: string, message: string): string {
    const supported = AppLocale.supported();
    for (let index = 0; index < supported.length; index += 1) {
      const locale = supported[index];
      if (RemoteI18n.lookup(locale, key) === message && RemoteI18n.isUniqueValue(locale, key)) {
        return locale;
      }
    }
    return '';
  }

  private static hasUniqueTemplate(key: string): boolean {
    const supported = AppLocale.supported();
    for (let index = 0; index < supported.length; index += 1) {
      if (!RemoteI18n.isUniqueValue(supported[index], key)) {
        return false;
      }
    }
    return true;
  }

  private static isUniqueValue(language: string, key: string): boolean {
    const catalog = RemoteI18n.catalogs.get(AppLocale.resolve(language) || language);
    const value = catalog?.get(key);
    if (!catalog || value === undefined) {
      return false;
    }
    let count = 0;
    catalog.forEach((item: string) => {
      if (item === value) {
        count += 1;
      }
    });
    return count === 1;
  }

  private static retranslateTemplate(key: string, message: string): string {
    const supported = AppLocale.supported();
    for (let index = 0; index < supported.length; index += 1) {
      const template = RemoteI18n.lookup(supported[index], key);
      const firstSlot = template.indexOf('{0}');
      if (firstSlot < 0) {
        continue;
      }
      const prefix = template.slice(0, firstSlot);
      const afterFirst = template.slice(firstSlot + 3);
      const secondSlot = afterFirst.indexOf('{1}');
      if (secondSlot < 0) {
        const suffix = afterFirst;
        if (!message.startsWith(prefix) || !message.endsWith(suffix)) {
          continue;
        }
        const value = message.slice(prefix.length, message.length - suffix.length);
        return RemoteI18n.f(key, value);
      }
      const middle = afterFirst.slice(0, secondSlot);
      const suffix = afterFirst.slice(secondSlot + 3);
      if (!message.startsWith(prefix) || !message.endsWith(suffix)) {
        continue;
      }
      const body = message.slice(prefix.length, message.length - suffix.length);
      const middleAt = body.indexOf(middle);
      if (middleAt < 0) {
        continue;
      }
      return RemoteI18n.f2(key, body.slice(0, middleAt), body.slice(middleAt + middle.length));
    }
    return '';
  }

  private static lookup(language: string, key: string): string {
    const chain = AppLocale.fallbackChain(language, true);
    for (let index = 0; index < chain.length; index += 1) {
      const value = RemoteI18n.catalogs.get(chain[index])?.get(key);
      if (value !== undefined) {
        return value;
      }
    }
    return key;
  }

  private static buildCatalogs(): Map<string, Map<string, string>> {
    const catalogs = new Map<string, Map<string, string>>();
    catalogs.set(APP_LOCALE_ZH_CN, new Map<string, string>(ZH_CN_MESSAGES));
    catalogs.set(APP_LOCALE_EN_US, new Map<string, string>(EN_US_MESSAGES));
    return catalogs;
  }
}

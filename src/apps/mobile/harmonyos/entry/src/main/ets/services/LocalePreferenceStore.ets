import { preferences } from '@kit.ArkData';
import { i18n } from '@kit.LocalizationKit';
import { AppLocale } from '../i18n/AppLocale';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { HarmonyUpgradeIdentityContract } from './HarmonyUpgradeIdentityContract';
import { OhosError } from './OhosError';
import { RemoteLogger } from './RemoteLogger';

const STORE_NAME = HarmonyUpgradeIdentityContract.APP_LOCALE_STORE;
const LANGUAGE_KEY = 'language';

export interface LocaleSystemHost {
  readSystemLanguage(): string;
  applyPreferredLanguage(language: string): void;
}

export class DefaultLocaleSystemHost implements LocaleSystemHost {
  readSystemLanguage(): string {
    try {
      const preferred = i18n.System.getAppPreferredLanguage();
      if (preferred && preferred.length > 0 && preferred !== 'default') {
        return preferred;
      }
    } catch (_err) {
      // Fall through to the system language.
    }
    try {
      return i18n.System.getSystemLanguage() || '';
    } catch (_err) {
      return '';
    }
  }

  applyPreferredLanguage(language: string): void {
    try {
      i18n.System.setAppPreferredLanguage(AppLocale.systemLanguageTag(language));
    } catch (err) {
      RemoteLogger.warn(`app preferred language update failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }
}

export class LocalePreferenceStore {
  private store?: preferences.Preferences;
  private readonly systemHost: LocaleSystemHost;

  constructor(systemHost: LocaleSystemHost = new DefaultLocaleSystemHost()) {
    this.systemHost = systemHost;
  }

  async init(context: Context): Promise<string> {
    try {
      this.store = await preferences.getPreferences(context, STORE_NAME);
    } catch (err) {
      throw OhosError.wrap('locale preference init', err);
    }
    const stored = await this.getString(LANGUAGE_KEY);
    const detected = AppLocale.initial(stored, this.systemHost.readSystemLanguage());
    return this.apply(detected, AppLocale.resolve(stored).length === 0);
  }

  async save(language: string): Promise<string> {
    return this.apply(AppLocale.resolve(language) || AppLocale.DEFAULT, true);
  }

  current(): string {
    return RemoteI18n.language();
  }

  private async apply(language: string, persist: boolean): Promise<string> {
    const resolved = RemoteI18n.setLanguage(language);
    this.systemHost.applyPreferredLanguage(resolved);
    if (persist) {
      await this.putString(LANGUAGE_KEY, resolved);
    }
    return resolved;
  }

  private async getString(key: string): Promise<string> {
    const store = this.store;
    if (!store) {
      return '';
    }
    try {
      const value = await store.get(key, '');
      return typeof value === 'string' ? value : '';
    } catch (err) {
      throw OhosError.wrap('locale preference read', err);
    }
  }

  private async putString(key: string, value: string): Promise<void> {
    const store = this.store;
    if (!store) {
      throw new Error('Locale preference store is not initialized.');
    }
    try {
      await store.put(key, value);
      await store.flush();
    } catch (err) {
      throw OhosError.wrap('locale preference write', err);
    }
  }
}

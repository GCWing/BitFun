import { AppLocale } from '../../i18n/AppLocale';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { LocalePreferenceStore } from '../../services/LocalePreferenceStore';
import { RemoteLogger } from '../../services/RemoteLogger';

export interface LocaleControllerHooks {
  readonly onLanguageChanged: (language: string) => void;
}

export class LocaleController {
  private readonly store: LocalePreferenceStore;
  private readonly hooks: LocaleControllerHooks;

  constructor(store: LocalePreferenceStore, hooks: LocaleControllerHooks) {
    this.store = store;
    this.hooks = hooks;
  }

  async initialize(context: Context): Promise<string> {
    try {
      const language = await this.store.init(context);
      this.hooks.onLanguageChanged(language);
      RemoteLogger.info(`app locale ready language=${language}`);
      return language;
    } catch (err) {
      const fallback = RemoteI18n.setLanguage(AppLocale.DEFAULT);
      RemoteLogger.warn(`locale init failed, using ${fallback}: ${err instanceof Error ? err.message : String(err)}`);
      this.hooks.onLanguageChanged(fallback);
      return fallback;
    }
  }

  async setLanguage(language: string): Promise<string> {
    const resolved = await this.store.save(language);
    this.hooks.onLanguageChanged(resolved);
    RemoteLogger.info(`app locale changed language=${resolved}`);
    return resolved;
  }

  current(): string {
    return this.store.current();
  }
}

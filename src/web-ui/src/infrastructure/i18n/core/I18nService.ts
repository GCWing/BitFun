 

import i18next, {
  i18n as I18nInstance,
  Resource,
  TFunction,
  type BackendModule,
} from 'i18next';
import { initReactI18next } from 'react-i18next';

import type {
  LocaleId,
  LocaleMetadata,
  I18nNamespace,
  I18nEventType,
  I18nEvent,
  I18nEventListener,
  I18nHooks,
} from '../types';
import {
  builtinLocales,
  DEFAULT_LOCALE,
  DEFAULT_FALLBACK_LOCALE,
  DEFAULT_NAMESPACE,
  WEB_UI_BOOTSTRAP_NAMESPACES,
  getLocaleFallbackChain,
  isLocaleSupported,
  SHARED_TERMS_BY_LOCALE,
} from '../presets';
import { useI18nStore } from '../store/i18nStore';
import { i18nAPI } from '@/infrastructure/api/service-api/I18nAPI';

import { createLogger } from '@/shared/utils/logger';
import { logDuration, measureSync, nowMs, elapsedMs } from '@/shared/utils/timing';

const log = createLogger('I18nService');

const lazyLocaleModules = import.meta.glob('../../../locales/**/*.json', {
  import: 'default',
}) as Record<string, () => Promise<Record<string, unknown>>>;

// Keep the bootstrap set explicit because these namespaces are used by
// synchronous i18nService.t(...) call sites during module initialization.
const bootstrapLocaleModules = import.meta.glob([
  '../../../locales/*/common.json',
  '../../../locales/*/components.json',
  '../../../locales/*/errors.json',
  '../../../locales/*/flow-chat.json',
  '../../../locales/*/panels/files.json',
  '../../../locales/*/panels/git.json',
  '../../../locales/*/settings/ai-model.json',
  '../../../locales/*/settings/lsp.json',
  '../../../locales/*/tools.json',
], {
  eager: true,
  import: 'default',
}) as Record<string, Record<string, unknown>>;

function parseLocaleModulePath(modulePath: string): { locale: string; namespace: string } | null {
  const match = modulePath.match(/locales\/([^/]+)\/(.+)\.json$/);
  if (!match) return null;

  const [, locale, namespace] = match;
  return { locale, namespace };
}

function addNamespaceResource(
  resources: Resource,
  locale: string,
  namespace: string,
  messages: Record<string, unknown>,
): void {
  resources[locale] = {
    ...(resources[locale] ?? {}),
    [namespace]: messages,
  };
}

function buildResources(): Resource {
  const resources = Object.entries(bootstrapLocaleModules).reduce<Resource>((acc, [modulePath, messages]) => {
    const parsed = parseLocaleModulePath(modulePath);
    if (!parsed) return acc;

    addNamespaceResource(acc, parsed.locale, parsed.namespace, messages);
    return acc;
  }, {});

  for (const [locale, sharedTerms] of Object.entries(SHARED_TERMS_BY_LOCALE)) {
    addNamespaceResource(resources, locale, 'shared', sharedTerms);
  }

  return resources;
}

async function loadLocaleNamespace(locale: string, namespace: string): Promise<Record<string, unknown>> {
  if (namespace === 'shared') {
    return SHARED_TERMS_BY_LOCALE[locale as LocaleId] ?? {};
  }

  const resourceModule = lazyLocaleModules[`../../../locales/${locale}/${namespace}.json`];
  if (!resourceModule) {
    return {};
  }

  return resourceModule();
}

const lazyNamespaceBackend: BackendModule = {
  type: 'backend',
  init() {
    // Required by the i18next backend interface; this backend has no setup state.
  },
  read(language, namespace, callback) {
    loadLocaleNamespace(language, namespace)
      .then((messages) => callback(null, messages))
      .catch((error) => callback(error, false));
  },
};

const resourcesResult = measureSync(() => buildResources());
const resources = resourcesResult.value;
logDuration(log, 'I18n resources prepared', resourcesResult.durationMs, {
  data: {
    localeCount: Object.keys(resources).length,
    bootstrapModuleCount: Object.keys(bootstrapLocaleModules).length,
    lazyModuleCount: Object.keys(lazyLocaleModules).length,
  },
});

 
export class I18nService {
  private i18nInstance: I18nInstance;
  private currentLocaleId: LocaleId = DEFAULT_LOCALE;
  private listeners: Map<I18nEventType, Set<I18nEventListener>> = new Map();
  private hooks: I18nHooks = {};
  private initialized: boolean = false;
  // Monotonic counter to detect mid-flight locale changes and avoid racey overrides.
  private localeChangeSeq: number = 0;

  constructor() {
    this.i18nInstance = i18next.createInstance();

    this.i18nInstance
      .use(lazyNamespaceBackend)
      .use(initReactI18next)
      .init({
        resources,
        partialBundledLanguages: true,
        lng: DEFAULT_LOCALE,
        fallbackLng: (code) => getLocaleFallbackChain(code ?? DEFAULT_FALLBACK_LOCALE),
        defaultNS: DEFAULT_NAMESPACE,
        // Shared terms are an explicit namespace, not a global fallback. Product
        // surfaces should opt in with local-first fallback keys when needed.
        fallbackNS: false,
        ns: [...WEB_UI_BOOTSTRAP_NAMESPACES],
        // Bootstrap namespaces must remain available to module-level
        // i18nService.t(...) calls immediately after service construction.
        initImmediate: false,
        interpolation: {
          escapeValue: false,
        },
        react: {
          useSuspense: false,
        },
      });
  }

  

   
  async initialize(): Promise<void> {
    if (this.initialized) {
      log.debug('Already initialized, skipping');
      return;
    }

    const startedAt = nowMs();
    try {
      let localeToUse: LocaleId = DEFAULT_LOCALE;
      const preInjectedLocale = document.documentElement.getAttribute('lang');
      if (preInjectedLocale && isLocaleSupported(preInjectedLocale)) {
        log.debug('Using pre-injected locale', { locale: preInjectedLocale });
        localeToUse = preInjectedLocale as LocaleId;
      }

      if (localeToUse !== this.currentLocaleId) {
        await this.loadNamespacesForLocale(WEB_UI_BOOTSTRAP_NAMESPACES, localeToUse);
        await this.i18nInstance.changeLanguage(localeToUse);
        this.currentLocaleId = localeToUse;
      }
      
      
      const store = useI18nStore.getState();
      WEB_UI_BOOTSTRAP_NAMESPACES.forEach((namespace) => store.addLoadedNamespace(namespace));
      store.setCurrentLanguage(this.currentLocaleId);
      store.setInitialized(true);
      
      
      this.updateHtmlLang(this.currentLocaleId);

      this.initialized = true;
      log.info('Initialization completed', { locale: this.currentLocaleId });
      logDuration(log, 'I18n initialization timing', elapsedMs(startedAt), {
        level: 'debug',
        data: {
        locale: this.currentLocaleId,
        },
      });

      const seqAtInitEnd = this.localeChangeSeq;
      const localeAtInitEnd = this.currentLocaleId;
      this.loadAndApplyBackendLocale(seqAtInitEnd, localeAtInitEnd);
    } catch (error) {
      log.error('Initialization failed', {
        error,
        durationMs: elapsedMs(startedAt),
      });
      
      this.initialized = true;
      const store = useI18nStore.getState();
      store.setInitialized(true);
    }
  }

  private async loadAndApplyBackendLocale(seqAtInitEnd: number, localeAtInitEnd: LocaleId): Promise<void> {
    const startedAt = nowMs();
    try {
      const savedLocale = await this.loadCurrentLocale();
      if (!savedLocale || savedLocale === this.currentLocaleId) {
        return;
      }

      // If the user changed language after initialization, do not override it with a stale backend value.
      if (this.localeChangeSeq !== seqAtInitEnd || this.currentLocaleId !== localeAtInitEnd) {
        return;
      }

      await this.changeLanguage(savedLocale);
      logDuration(log, 'Backend locale applied after initialization', elapsedMs(startedAt), {
        data: {
        locale: savedLocale,
        },
      });
    } catch (error) {
      log.debug('Failed to load backend locale (ignored)', error);
    }
  }

   
  private async loadCurrentLocale(): Promise<LocaleId | null> {
    const startedAt = nowMs();
    try {
      
      const timeoutPromise = new Promise<null>((resolve) => {
        setTimeout(() => resolve(null), 2000); 
      });

      const locale = await Promise.race([
        i18nAPI.getCurrentLanguage(),
        timeoutPromise,
      ]);

      const resolvedLocale = locale && isLocaleSupported(locale) ? locale : null;
      logDuration(log, 'Backend locale load completed', elapsedMs(startedAt), {
        data: {
        locale: resolvedLocale ?? locale ?? 'timeout',
        },
      });
      return resolvedLocale;
    } catch (error) {
      log.debug('Failed to load locale config (ignored)', {
        error,
        durationMs: elapsedMs(startedAt),
      });
      return null;
    }
  }

   
  private async saveCurrentLocale(locale: LocaleId): Promise<void> {
    try {
      await i18nAPI.setLanguage(locale);
    } catch (error) {
      log.warn('Failed to save locale config', error);
    }
  }

  

   
  getI18nInstance(): I18nInstance {
    return this.i18nInstance;
  }

   
  getT(): TFunction {
    return this.i18nInstance.t.bind(this.i18nInstance);
  }

   
  getCurrentLocale(): LocaleId {
    return this.currentLocaleId;
  }

   
  getCurrentLocaleMetadata(): LocaleMetadata | undefined {
    return builtinLocales.find(locale => locale.id === this.currentLocaleId);
  }

   
  getSupportedLocales(): LocaleMetadata[] {
    return builtinLocales;
  }

   
  async changeLanguage(locale: LocaleId): Promise<void> {
    if (!isLocaleSupported(locale)) {
      log.error('Unsupported locale', { locale });
      throw new Error(`Unsupported locale: ${locale}`);
    }

    if (locale === this.currentLocaleId) {
      log.debug('Locale unchanged, skipping', { locale });
      return;
    }

    const oldLocale = this.currentLocaleId;
    const store = useI18nStore.getState();

    try {
      this.localeChangeSeq += 1;
      store.setChanging(true);

      
      if (this.hooks.beforeChange) {
        await this.hooks.beforeChange(locale, oldLocale);
      }
      this.emitEvent('i18n:before-change', locale, oldLocale);

      const loadedNamespaces = new Set<I18nNamespace>([
        ...WEB_UI_BOOTSTRAP_NAMESPACES,
        ...store.loadedNamespaces,
      ]);
      await this.loadNamespacesForLocale([...loadedNamespaces], locale);

      await this.i18nInstance.changeLanguage(locale);
      this.currentLocaleId = locale;

      
      this.updateHtmlLang(locale);

      
      store.setCurrentLanguage(locale);

      
      await this.saveCurrentLocale(locale);

      
      if (this.hooks.afterChange) {
        await this.hooks.afterChange(locale, oldLocale);
      }
      this.emitEvent('i18n:after-change', locale, oldLocale);

      log.info('Language changed', { locale, previousLocale: oldLocale });
    } catch (error) {
      log.error('Failed to change language', { locale, error });
      this.emitEvent('i18n:error', locale, oldLocale, undefined, error as Error);
      throw error;
    } finally {
      store.setChanging(false);
    }
  }

   
  private updateHtmlLang(locale: LocaleId): void {
    document.documentElement.setAttribute('lang', locale);
    
    
    const metadata = builtinLocales.find(l => l.id === locale);
    if (metadata?.rtl) {
      document.documentElement.setAttribute('dir', 'rtl');
    } else {
      document.documentElement.setAttribute('dir', 'ltr');
    }
  }

  

   
  async loadNamespace(namespace: I18nNamespace): Promise<void> {
    const store = useI18nStore.getState();

    if (this.hasNamespaceResources(namespace, this.currentLocaleId)) {
      store.addLoadedNamespace(namespace);
      return;
    }

    try {
      await this.loadNamespacesForLocale([namespace], this.currentLocaleId);
      store.addLoadedNamespace(namespace);
      this.emitEvent('i18n:namespace-loaded', this.currentLocaleId, undefined, namespace);
    } catch (error) {
      log.error('Failed to load namespace', { namespace, error });
      throw error;
    }
  }

   
  isNamespaceLoaded(namespace: I18nNamespace): boolean {
    const store = useI18nStore.getState();
    return store.loadedNamespaces.includes(namespace) && this.hasNamespaceResources(namespace, this.currentLocaleId);
  }

  private hasNamespaceResources(namespace: I18nNamespace, locale: LocaleId): boolean {
    return getLocaleFallbackChain(locale, true)
      .every((localeId) => this.i18nInstance.hasResourceBundle(localeId, namespace));
  }

  private async loadNamespacesForLocale(
    namespaces: readonly I18nNamespace[],
    locale: LocaleId,
  ): Promise<void> {
    const localeChain = getLocaleFallbackChain(locale, true);
    await Promise.all(
      localeChain.flatMap((localeId) =>
        namespaces.map(async (namespace) => {
          if (this.i18nInstance.hasResourceBundle(localeId, namespace)) {
            return;
          }

          const messages = await loadLocaleNamespace(localeId, namespace);
          if (Object.keys(messages).length > 0) {
            this.i18nInstance.addResourceBundle(localeId, namespace, messages, true, true);
          }
        }),
      ),
    );
  }

  

   
  t(key: string, options?: Record<string, unknown>): string {
    return this.i18nInstance.t(key, { ...(options as any), returnObjects: false }) as string;
  }

   
  exists(key: string): boolean {
    return this.i18nInstance.exists(key);
  }

  

   
  formatDate(date: Date | number, options?: Intl.DateTimeFormatOptions): string {
    return new Intl.DateTimeFormat(this.currentLocaleId, options).format(date);
  }

   
  formatNumber(number: number, options?: Intl.NumberFormatOptions): string {
    return new Intl.NumberFormat(this.currentLocaleId, options).format(number);
  }

   
  formatCurrency(amount: number, currency: string = 'CNY'): string {
    return this.formatNumber(amount, {
      style: 'currency',
      currency,
    });
  }

   
  formatRelativeTime(date: Date | number, unit?: Intl.RelativeTimeFormatUnit): string {
    const rtf = new Intl.RelativeTimeFormat(this.currentLocaleId, { numeric: 'auto' });
    
    const now = Date.now();
    const target = typeof date === 'number' ? date : date.getTime();
    const diff = target - now;
    
    
    const seconds = Math.round(diff / 1000);
    const minutes = Math.round(diff / 60000);
    const hours = Math.round(diff / 3600000);
    const days = Math.round(diff / 86400000);
    
    if (unit) {
      return rtf.format(Math.round(diff / this.getUnitMilliseconds(unit)), unit);
    }
    
    if (Math.abs(seconds) < 60) {
      return rtf.format(seconds, 'second');
    } else if (Math.abs(minutes) < 60) {
      return rtf.format(minutes, 'minute');
    } else if (Math.abs(hours) < 24) {
      return rtf.format(hours, 'hour');
    } else {
      return rtf.format(days, 'day');
    }
  }

  private getUnitMilliseconds(unit: Intl.RelativeTimeFormatUnit): number {
    switch (unit) {
      case 'second': return 1000;
      case 'minute': return 60000;
      case 'hour': return 3600000;
      case 'day': return 86400000;
      case 'week': return 604800000;
      case 'month': return 2592000000;
      case 'year': return 31536000000;
      default: return 1000;
    }
  }

  

   
  on(eventType: I18nEventType, listener: I18nEventListener): () => void {
    if (!this.listeners.has(eventType)) {
      this.listeners.set(eventType, new Set());
    }

    this.listeners.get(eventType)!.add(listener);

    
    return () => {
      this.listeners.get(eventType)?.delete(listener);
    };
  }

   
  private emitEvent(
    type: I18nEventType,
    locale: LocaleId,
    previousLocale?: LocaleId,
    namespace?: I18nNamespace,
    error?: Error
  ): void {
    const event: I18nEvent = {
      type,
      locale,
      previousLocale,
      namespace,
      error,
      timestamp: Date.now(),
    };

    const listeners = this.listeners.get(type);
    if (listeners) {
      listeners.forEach(listener => {
        try {
          listener(event);
        } catch (err) {
          log.error('Event listener execution failed', { eventType: type, error: err });
        }
      });
    }
  }

  

   
  registerHooks(hooks: I18nHooks): void {
    this.hooks = { ...this.hooks, ...hooks };
  }

  

   
  isInitialized(): boolean {
    return this.initialized;
  }

   
  isRTL(): boolean {
    const metadata = this.getCurrentLocaleMetadata();
    return metadata?.rtl ?? false;
  }
}


export const i18nService = new I18nService();

 

import i18next, { i18n as I18nInstance, TFunction } from 'i18next';
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
  isLocaleSupported,
} from '../presets';
import { useI18nStore } from '../store/i18nStore';
import { i18nAPI } from '@/infrastructure/api';


import zhCNCommon from '../../../locales/zh-CN/common.json';
import zhCNFlowChat from '../../../locales/zh-CN/flow-chat.json';
import zhCNTools from '../../../locales/zh-CN/tools.json';
import zhCNSettings from '../../../locales/zh-CN/settings.json';
import zhCNErrors from '../../../locales/zh-CN/errors.json';
import zhCNNotifications from '../../../locales/zh-CN/notifications.json';
import zhCNComponents from '../../../locales/zh-CN/components.json';

import zhCNPanelsSessions from '../../../locales/zh-CN/panels/sessions.json';
import zhCNPanelsWorkflows from '../../../locales/zh-CN/panels/workflows.json';
import zhCNPanelsFiles from '../../../locales/zh-CN/panels/files.json';
import zhCNPanelsGit from '../../../locales/zh-CN/panels/git.json';
import zhCNPanelsProjectContext from '../../../locales/zh-CN/panels/project-context.json';
import zhCNPanelsTerminal from '../../../locales/zh-CN/panels/terminal.json';

import zhCNSettingsAiModel from '../../../locales/zh-CN/settings/ai-model.json';
import zhCNSettingsSuperAgent from '../../../locales/zh-CN/settings/super-agent.json';
import zhCNSettingsModes from '../../../locales/zh-CN/settings/modes.json';
import zhCNSettingsAgenticTools from '../../../locales/zh-CN/settings/agentic-tools.json';
import zhCNSettingsMcp from '../../../locales/zh-CN/settings/mcp.json';
import zhCNSettingsTheme from '../../../locales/zh-CN/settings/theme.json';
import zhCNSettingsAiFeatures from '../../../locales/zh-CN/settings/ai-features.json';
import zhCNSettingsLsp from '../../../locales/zh-CN/settings/lsp.json';
import zhCNSettingsTerminal from '../../../locales/zh-CN/settings/terminal.json';
import zhCNSettingsDebug from '../../../locales/zh-CN/settings/debug.json';
import zhCNSettingsLogging from '../../../locales/zh-CN/settings/logging.json';
import zhCNSettingsEditor from '../../../locales/zh-CN/settings/editor.json';
import zhCNSettingsPromptTemplates from '../../../locales/zh-CN/settings/prompt-templates.json';
import zhCNSettingsSkills from '../../../locales/zh-CN/settings/skills.json';
import zhCNSettingsAiRules from '../../../locales/zh-CN/settings/ai-rules.json';
import zhCNSettingsAiMemory from '../../../locales/zh-CN/settings/ai-memory.json';
import zhCNSettingsAgents from '../../../locales/zh-CN/settings/agents.json';
import zhCNSettingsDefaultModel from '../../../locales/zh-CN/settings/default-model.json';

import zhCNMermaidEditor from '../../../locales/zh-CN/mermaid-editor.json';
import zhCNOnboarding from '../../../locales/zh-CN/onboarding.json';

import enUSCommon from '../../../locales/en-US/common.json';
import enUSFlowChat from '../../../locales/en-US/flow-chat.json';
import enUSTools from '../../../locales/en-US/tools.json';
import enUSSettings from '../../../locales/en-US/settings.json';
import enUSErrors from '../../../locales/en-US/errors.json';
import enUSNotifications from '../../../locales/en-US/notifications.json';
import enUSComponents from '../../../locales/en-US/components.json';

import enUSPanelsSessions from '../../../locales/en-US/panels/sessions.json';
import enUSPanelsWorkflows from '../../../locales/en-US/panels/workflows.json';
import enUSPanelsFiles from '../../../locales/en-US/panels/files.json';
import enUSPanelsGit from '../../../locales/en-US/panels/git.json';
import enUSPanelsProjectContext from '../../../locales/en-US/panels/project-context.json';
import enUSPanelsTerminal from '../../../locales/en-US/panels/terminal.json';

import enUSSettingsAiModel from '../../../locales/en-US/settings/ai-model.json';
import enUSSettingsSuperAgent from '../../../locales/en-US/settings/super-agent.json';
import enUSSettingsModes from '../../../locales/en-US/settings/modes.json';
import enUSSettingsAgenticTools from '../../../locales/en-US/settings/agentic-tools.json';
import enUSSettingsMcp from '../../../locales/en-US/settings/mcp.json';
import enUSSettingsTheme from '../../../locales/en-US/settings/theme.json';
import enUSSettingsAiFeatures from '../../../locales/en-US/settings/ai-features.json';
import enUSSettingsLsp from '../../../locales/en-US/settings/lsp.json';
import enUSSettingsTerminal from '../../../locales/en-US/settings/terminal.json';
import enUSSettingsDebug from '../../../locales/en-US/settings/debug.json';
import enUSSettingsLogging from '../../../locales/en-US/settings/logging.json';
import enUSSettingsEditor from '../../../locales/en-US/settings/editor.json';
import enUSSettingsPromptTemplates from '../../../locales/en-US/settings/prompt-templates.json';
import enUSSettingsSkills from '../../../locales/en-US/settings/skills.json';
import enUSSettingsAiRules from '../../../locales/en-US/settings/ai-rules.json';
import enUSSettingsAiMemory from '../../../locales/en-US/settings/ai-memory.json';
import enUSSettingsAgents from '../../../locales/en-US/settings/agents.json';
import enUSSettingsDefaultModel from '../../../locales/en-US/settings/default-model.json';

import enUSMermaidEditor from '../../../locales/en-US/mermaid-editor.json';
import enUSOnboarding from '../../../locales/en-US/onboarding.json';

import { createLogger } from '@/shared/utils/logger';

const log = createLogger('I18nService');

 
const resources = {
  'zh-CN': {
    common: zhCNCommon,
    'flow-chat': zhCNFlowChat,
    tools: zhCNTools,
    settings: zhCNSettings,
    errors: zhCNErrors,
    notifications: zhCNNotifications,
    components: zhCNComponents,
    
    'panels/sessions': zhCNPanelsSessions,
    'panels/workflows': zhCNPanelsWorkflows,
    'panels/files': zhCNPanelsFiles,
    'panels/git': zhCNPanelsGit,
    'panels/project-context': zhCNPanelsProjectContext,
    'panels/terminal': zhCNPanelsTerminal,
    
    'settings/ai-model': zhCNSettingsAiModel,
    'settings/super-agent': zhCNSettingsSuperAgent,
    'settings/modes': zhCNSettingsModes,
    'settings/agentic-tools': zhCNSettingsAgenticTools,
    'settings/mcp': zhCNSettingsMcp,
    'settings/theme': zhCNSettingsTheme,
    'settings/ai-features': zhCNSettingsAiFeatures,
    'settings/lsp': zhCNSettingsLsp,
    'settings/terminal': zhCNSettingsTerminal,
    'settings/debug': zhCNSettingsDebug,
    'settings/logging': zhCNSettingsLogging,
    'settings/editor': zhCNSettingsEditor,
    'settings/prompt-templates': zhCNSettingsPromptTemplates,
    'settings/skills': zhCNSettingsSkills,
    'settings/ai-rules': zhCNSettingsAiRules,
    'settings/ai-memory': zhCNSettingsAiMemory,
    'settings/agents': zhCNSettingsAgents,
    'settings/default-model': zhCNSettingsDefaultModel,
    
    'mermaid-editor': zhCNMermaidEditor,
    'onboarding': zhCNOnboarding,
  },
  'en-US': {
    common: enUSCommon,
    'flow-chat': enUSFlowChat,
    tools: enUSTools,
    settings: enUSSettings,
    errors: enUSErrors,
    notifications: enUSNotifications,
    components: enUSComponents,
    
    'panels/sessions': enUSPanelsSessions,
    'panels/workflows': enUSPanelsWorkflows,
    'panels/files': enUSPanelsFiles,
    'panels/git': enUSPanelsGit,
    'panels/project-context': enUSPanelsProjectContext,
    'panels/terminal': enUSPanelsTerminal,
    
    'settings/ai-model': enUSSettingsAiModel,
    'settings/super-agent': enUSSettingsSuperAgent,
    'settings/modes': enUSSettingsModes,
    'settings/agentic-tools': enUSSettingsAgenticTools,
    'settings/mcp': enUSSettingsMcp,
    'settings/theme': enUSSettingsTheme,
    'settings/ai-features': enUSSettingsAiFeatures,
    'settings/lsp': enUSSettingsLsp,
    'settings/terminal': enUSSettingsTerminal,
    'settings/debug': enUSSettingsDebug,
    'settings/logging': enUSSettingsLogging,
    'settings/editor': enUSSettingsEditor,
    'settings/prompt-templates': enUSSettingsPromptTemplates,
    'settings/skills': enUSSettingsSkills,
    'settings/ai-rules': enUSSettingsAiRules,
    'settings/ai-memory': enUSSettingsAiMemory,
    'settings/agents': enUSSettingsAgents,
    'settings/default-model': enUSSettingsDefaultModel,
    
    'mermaid-editor': enUSMermaidEditor,
    'onboarding': enUSOnboarding,
  },
};

 
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
      .use(initReactI18next)
      .init({
        resources,
        lng: DEFAULT_LOCALE,
        fallbackLng: DEFAULT_FALLBACK_LOCALE,
        defaultNS: DEFAULT_NAMESPACE,
        ns: [
          'common', 
          'flow-chat', 
          'tools', 
          'settings', 
          'errors', 
          'notifications', 
          'components',
          
          'panels/sessions',
          'panels/workflows',
          'panels/files',
          'panels/git',
          'panels/project-context',
          'panels/terminal',
          
          'settings/ai-model',
          'settings/super-agent',
          'settings/modes',
          'settings/agentic-tools',
          'settings/mcp',
          'settings/theme',
          'settings/ai-features',
          'settings/lsp',
          'settings/terminal',
          'settings/debug',
          'settings/logging',
          'settings/editor',
          'settings/prompt-templates',
          'settings/skills',
          'settings/ai-rules',
          'settings/ai-memory',
          'settings/agents',
          'settings/default-model',
          
          'mermaid-editor',
          'onboarding',
        ],
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

    try {
      let localeToUse: LocaleId = DEFAULT_LOCALE;
      const preInjectedLocale = document.documentElement.getAttribute('lang');
      if (preInjectedLocale && isLocaleSupported(preInjectedLocale)) {
        log.debug('Using pre-injected locale', { locale: preInjectedLocale });
        localeToUse = preInjectedLocale as LocaleId;
      }

      if (localeToUse !== this.currentLocaleId) {
        await this.i18nInstance.changeLanguage(localeToUse);
        this.currentLocaleId = localeToUse;
      }
      
      
      const store = useI18nStore.getState();
      store.setCurrentLanguage(this.currentLocaleId);
      store.setInitialized(true);
      
      
      this.updateHtmlLang(this.currentLocaleId);

      this.initialized = true;
      log.info('Initialization completed', { locale: this.currentLocaleId });

      const seqAtInitEnd = this.localeChangeSeq;
      const localeAtInitEnd = this.currentLocaleId;
      this.loadAndApplyBackendLocale(seqAtInitEnd, localeAtInitEnd);
    } catch (error) {
      log.error('Initialization failed', error);
      
      this.initialized = true;
      const store = useI18nStore.getState();
      store.setInitialized(true);
    }
  }

  private async loadAndApplyBackendLocale(seqAtInitEnd: number, localeAtInitEnd: LocaleId): Promise<void> {
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
    } catch (error) {
      log.debug('Failed to load backend locale (ignored)', error);
    }
  }

   
  private async loadCurrentLocale(): Promise<LocaleId | null> {
    try {
      
      const timeoutPromise = new Promise<null>((resolve) => {
        setTimeout(() => resolve(null), 2000); 
      });

      const locale = await Promise.race([
        i18nAPI.getCurrentLanguage(),
        timeoutPromise,
      ]);

      if (locale && isLocaleSupported(locale)) {
        return locale;
      }
      return null;
    } catch (error) {
      log.debug('Failed to load locale config (ignored)', error);
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
    
    if (store.loadedNamespaces.includes(namespace)) {
      return;
    }

    try {
      await this.i18nInstance.loadNamespaces(namespace);
      store.addLoadedNamespace(namespace);
      this.emitEvent('i18n:namespace-loaded', this.currentLocaleId, undefined, namespace);
    } catch (error) {
      log.error('Failed to load namespace', { namespace, error });
      throw error;
    }
  }

   
  isNamespaceLoaded(namespace: I18nNamespace): boolean {
    const store = useI18nStore.getState();
    return store.loadedNamespaces.includes(namespace);
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

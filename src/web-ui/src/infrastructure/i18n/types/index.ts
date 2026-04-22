 

import type { ALL_NAMESPACES } from '../presets/namespaceRegistry';
import type { LocaleId, LocaleMetadata } from '../presets/localeRegistry';

export type { LocaleId, LocaleMetadata };

export type I18nNamespace = (typeof ALL_NAMESPACES)[number];

 
export interface I18nConfig {
   
  currentLanguage: LocaleId;
   
  fallbackLanguage: LocaleId;
   
  autoDetect: boolean;
   
  loadedNamespaces: I18nNamespace[];
}

 
export type I18nEventType = 
  | 'i18n:before-change'
  | 'i18n:after-change'
  | 'i18n:namespace-loaded'
  | 'i18n:error';

 
export interface I18nEvent {
  type: I18nEventType;
  locale: LocaleId;
  previousLocale?: LocaleId;
  namespace?: I18nNamespace;
  error?: Error;
  timestamp: number;
}

 
export type I18nEventListener = (event: I18nEvent) => void;

 
export interface I18nHooks {
   
  beforeChange?: (newLocale: LocaleId, oldLocale: LocaleId) => Promise<void> | void;
   
  afterChange?: (newLocale: LocaleId, oldLocale: LocaleId) => Promise<void> | void;
}

 
export interface I18nState {
   
  currentLanguage: LocaleId;
   
  fallbackLanguage: LocaleId;
   
  loadedNamespaces: I18nNamespace[];
   
  isInitialized: boolean;
   
  isChanging: boolean;
   
  autoDetect: boolean;
}

/**
 * I18n Store Actions
 */
export interface I18nActions {
   
  setCurrentLanguage: (locale: LocaleId) => void;
   
  setFallbackLanguage: (locale: LocaleId) => void;
   
  addLoadedNamespace: (namespace: I18nNamespace) => void;
   
  setInitialized: (initialized: boolean) => void;
   
  setChanging: (changing: boolean) => void;
   
  setAutoDetect: (autoDetect: boolean) => void;
   
  reset: () => void;
}

 
export interface TranslationParams {
  [key: string]: string | number | boolean | Date | undefined;
}

 
export interface PluralOptions {
  count: number;
  [key: string]: string | number;
}

 
export interface DateFormatOptions {
  format?: 'short' | 'medium' | 'long' | 'full';
  dateStyle?: 'short' | 'medium' | 'long' | 'full';
  timeStyle?: 'short' | 'medium' | 'long' | 'full';
}

 
export interface NumberFormatOptions {
  style?: 'decimal' | 'currency' | 'percent';
  currency?: string;
  minimumFractionDigits?: number;
  maximumFractionDigits?: number;
}

 

 
export type LocaleId = 'zh-CN' | 'en-US';

 
export interface LocaleMetadata {
   
  id: LocaleId;
   
  name: string;
   
  englishName: string;
   
  nativeName: string;
   
  rtl: boolean;
   
  dateFormat: string;
   
  numberFormat: {
    decimal: string;
    thousands: string;
  };
   
  builtin: boolean;
}

 
export type I18nNamespace = 
  | 'common'           
  | 'flow-chat'        
  | 'tools'            
  | 'settings'         
  | 'errors'           
  | 'notifications'    
  | 'components'
  | 'panels/git'
  | 'panels/terminal'
  | 'scenes/capabilities'
  | 'scenes/apps'
  | 'scenes/subagents'
  | 'scenes/profile'
  | 'scenes/skills'
  | 'scenes/tools';      

 
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
